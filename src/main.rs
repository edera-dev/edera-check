mod checkers;
mod helpers;
mod recorders;

use checkers::preinstall::{
    byo_kernel::BYOKernelChecks, iommu::IOMMUChecks, kernel::KernelChecks, numa::NUMAChecks,
    pvh::PVHChecks, system::SystemChecks,
};

use checkers::postinstall::{
    guest_type::GuestTypeChecks, kernel::PostinstallKernelChecks, kube::KubeChecks,
    services::ServiceChecks,
};

use clap::{Parser, Subcommand};
use console::{Emoji, style};
use helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use recorders::postinstall::system::SystemRecorder as postrecorder;
use recorders::preinstall::system::SystemRecorder as prerecorder;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use flate2::{Compression, write::GzEncoder};
use log::debug;
use std::{
    collections::HashSet,
    env, fs,
    fs::File,
    path::{Path, PathBuf},
    process,
};
use tokio::task::JoinHandle;

static SPARKLE: Emoji = Emoji("✨", "[*]");

#[derive(Parser)]
#[command(name = "edera-check")]
#[command(about = "CLI to run checks before installing or using Edera", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run before installing Edera to validate hardware/host installation readiness.
    Preinstall {
        /// Validate running kernel for bring-your-own kernel support (default false)
        #[arg(short, long, default_value_t = false)]
        byo_kernel: bool,

        /// Collect information and configuration snapshot of current system (default true)
        #[arg(short, long, default_value_t = true)]
        record_hostinfo: bool,

        /// Run only selected checks, instead of default behavior of running all.
        /// Will override all other check enablement flags.
        #[arg(short, long, value_delimiter = ',')]
        only_checks: Vec<String>,

        /// Directory path to write report to. Will be created if it doesn't exist. Defaults to `/tmp`
        #[arg(short = 'd', long)]
        report_dir: Option<String>,
    },
    /// Run after installing Edera to validate workload readiness.
    Postinstall {
        /// Collect information and configuration snapshot of current system (default true)
        #[arg(short, long, default_value_t = true)]
        record_hostinfo: bool,

        /// Run only selected checks, instead of default behavior of running all.
        /// Will override all other check enablement flags.
        #[arg(short, long, value_delimiter = ',')]
        only_checks: Vec<String>,

        /// Directory path to write report to. Will be created if it doesn't exist. Defaults to `/tmp`
        #[arg(short = 'd', long)]
        report_dir: Option<String>,
    },
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Preinstall {
            byo_kernel,
            record_hostinfo,
            only_checks,
            report_dir,
        } => {
            // If we are in a privileged container running in the host pid namespace,
            // this creates a tokio thread pool that runs stuff outside of the container context,
            // directly on the host.
            // If we are in a regular old `sudo`'d binary running naked on the host,
            // this is effectively a silent no-op.
            let host_executor = HostNamespaceExecutor::new();

            // See if we are already booted under Edera. If so, error out and suggest `postinstall`
            // as the command to run.
            match host_executor
                .spawn_in_host_ns(async {
                    if !Path::new("/var/lib/edera/protect/.install-completed").exists() {
                        return false;
                    }
                    let xen = Path::new("/sys/hypervisor/type");
                    xen.exists() && fs::read_to_string(xen).unwrap_or_default().trim() == "xen"
                })
                .await
            {
                // TODO(bml) later we may add a `postinstall` command,
                // but for now all we have is `preinstall` and running it under an active Edera boot
                // is not supported or useful.
                Ok(true) => {
                    println!(
                        "{}",
                        style("Edera is already installed. Run `edera-check postinstall` instead.")
                            .red()
                            .bold()
                    );
                    process::exit(1);
                }
                Ok(false) => (),
                Err(e) => {
                    bail!("Error: {}", e);
                }
            };

            let mut groups: Vec<Box<dyn CheckGroup>> = vec![
                Box::new(SystemChecks::new(host_executor.clone())),
                Box::new(PVHChecks::new(host_executor.clone())),
                Box::new(KernelChecks::new(host_executor.clone())),
                Box::new(IOMMUChecks::new(host_executor.clone())),
                Box::new(NUMAChecks::new(host_executor.clone())),
            ];

            if record_hostinfo {
                println!(
                    "Collecting information about the current host as part of locally-generated preinstall report."
                );
                println!("The information collected will remain on this host.");
                groups.push(Box::new(prerecorder::new(host_executor.clone())));
            }

            if byo_kernel {
                groups.push(Box::new(BYOKernelChecks::new(host_executor.clone())));
            }

            // If only-checks is specified, only include checks that match the provided ID.
            if !only_checks.is_empty() {
                let valid_ids: HashSet<_> = groups.iter().map(|g| g.id().to_string()).collect();
                only_checks.iter().for_each(|id| {
                    if !valid_ids.contains(id) {
                        println!("{} '{}'", style("Unknown Check:").yellow(), style(id).red());
                    }
                });
                groups.retain(|group| only_checks.contains(&group.id().to_string()));
            }

            groups.sort_by_key(|g| g.category());

            let mut required_groups_result = Passed;
            let mut all_groups_result = Passed;

            let hostname = host_executor
                .spawn_in_host_ns(async { std::fs::read_to_string("/etc/hostname").unwrap() })
                .await?;

            let base_dir = if let Some(dir) = report_dir {
                PathBuf::from(dir)
            } else {
                env::temp_dir()
            };

            let base_path = create_base_path(base_dir, hostname.trim(), "preinstall")
                .map_err(|e| anyhow!("failed to create bundle base path: {e}"))?;
            // Run each check group
            for group in groups {
                println!(
                    "{} {} [{}] - {}",
                    style("Running Group").cyan(),
                    style(group.name()).cyan().bold(),
                    style(group.category()).white().bold(),
                    group.description()
                );

                let check_group_result = group.run().await;

                check_group_result.log_individual_checks();

                check_group_result.log_group(group.category());

                // Set final result to Failed if we failed and aren't already in an Errored state
                // However, do not allow Optional groups to count towards Errored or Failed state.
                if matches!(check_group_result.result, Failed(_)) {
                    if matches!(group.category(), CheckGroupCategory::Required)
                        && !matches!(required_groups_result, Errored(_))
                    {
                        required_groups_result = Failed(String::from("group failed"));
                    } else if !matches!(all_groups_result, Errored(_)) {
                        all_groups_result = Failed(String::from("group failed"));
                    }
                }

                if matches!(check_group_result.result, Errored(_)) {
                    if matches!(group.category(), CheckGroupCategory::Required) {
                        required_groups_result = Errored(String::from("group errored"));
                    } else {
                        all_groups_result = Errored(String::from("group errored"));
                    }
                }

                write_group_report(group, &check_group_result, &base_path)?;
            }

            create_gzip_from(base_path, host_executor.clone()).await?;

            match required_groups_result {
                Errored(_) | Failed(_) => bail!("Required preinstall checks did not pass"),
                _ => Ok(()),
            }
        }
        Commands::Postinstall {
            record_hostinfo,
            only_checks,
            report_dir,
        } => {
            // If we are in a privileged container running in the host pid namespace,
            // this creates a tokio thread pool that runs stuff outside of the container context,
            // directly on the host.
            // If we are in a regular old `sudo`'d binary running naked on the host,
            // this is effectively a silent no-op.
            let host_executor = HostNamespaceExecutor::new();

            // See if we are already booted under Edera. If so, error out and suggest `postinstall`
            // as the command to run.
            match host_executor
                .spawn_in_host_ns(async {
                    if !Path::new("/var/lib/edera/protect/.install-completed").exists() {
                        return false;
                    }
                    let xen = Path::new("/sys/hypervisor/type");
                    xen.exists() && fs::read_to_string(xen).unwrap_or_default().trim() == "xen"
                })
                .await
            {
                // TODO(bml) later we may add a `postinstall` command,
                // but for now all we have is `preinstall` and running it under an active Edera boot
                // is not supported or useful.
                Ok(true) => {}
                Ok(false) => {
                    println!(
                        "{}",
                        style("Edera not installed. Run `edera-check preinstall` instead.")
                            .red()
                            .bold()
                    );
                    process::exit(1);
                }
                Err(e) => {
                    bail!("Error: {}", e);
                }
            };

            let mut groups: Vec<Box<dyn CheckGroup>> = vec![
                Box::new(GuestTypeChecks::new(host_executor.clone())),
                Box::new(PostinstallKernelChecks::new(host_executor.clone())),
                Box::new(ServiceChecks::new(host_executor.clone())),
                Box::new(KubeChecks::new(host_executor.clone())),
            ];

            if record_hostinfo {
                println!(
                    "Collecting information about the current host as part of locally-generated preinstall report."
                );
                println!("The information collected will remain on this host.");
                groups.push(Box::new(postrecorder::new(host_executor.clone())));
            }

            // If only-checks is specified, only include checks that match the provided ID.
            if !only_checks.is_empty() {
                let valid_ids: HashSet<_> = groups.iter().map(|g| g.id().to_string()).collect();
                only_checks.iter().for_each(|id| {
                    if !valid_ids.contains(id) {
                        println!("{} '{}'", style("Unknown Check:").yellow(), style(id).red());
                    }
                });
                groups.retain(|group| only_checks.contains(&group.id().to_string()));
            }

            groups.sort_by_key(|g| g.category());

            let mut required_groups_result = Passed;
            let mut all_groups_result = Passed;

            let hostname = host_executor
                .spawn_in_host_ns(async { std::fs::read_to_string("/etc/hostname").unwrap() })
                .await?;

            let base_dir = if let Some(dir) = report_dir {
                PathBuf::from(dir)
            } else {
                env::temp_dir()
            };

            let base_path = create_base_path(base_dir, hostname.trim(), "postinstall")
                .map_err(|e| anyhow!("failed to create bundle base path: {e}"))?;
            // Run each check group
            for group in groups {
                println!(
                    "{} {} [{}] - {}",
                    style("Running Group").cyan(),
                    style(group.name()).cyan().bold(),
                    style(group.category()).white().bold(),
                    group.description()
                );

                let check_group_result = group.run().await;

                check_group_result.log_individual_checks();

                check_group_result.log_group(group.category());

                // Set final result to Failed if we failed and aren't already in an Errored state
                // However, do not allow Optional groups to count towards Errored or Failed state.
                if matches!(check_group_result.result, Failed(_)) {
                    if matches!(group.category(), CheckGroupCategory::Required)
                        && !matches!(required_groups_result, Errored(_))
                    {
                        required_groups_result = Failed(String::from("group failed"));
                    } else if !matches!(all_groups_result, Errored(_)) {
                        all_groups_result = Failed(String::from("group failed"));
                    }
                }

                if matches!(check_group_result.result, Errored(_)) {
                    if matches!(group.category(), CheckGroupCategory::Required) {
                        required_groups_result = Errored(String::from("group errored"));
                    } else {
                        all_groups_result = Errored(String::from("group errored"));
                    }
                }

                write_group_report(group, &check_group_result, &base_path)?;
            }

            create_gzip_from(base_path, host_executor.clone()).await?;

            match required_groups_result {
                Errored(_) | Failed(_) => bail!("Required preinstall checks did not pass"),
                _ => Ok(()),
            }
        }
    }
}

/// This writes the gzip to the container namespace /tmp, and then copies it out to
/// the same path on the host at the end.
async fn create_gzip_from(base_path: PathBuf, host_executor: HostNamespaceExecutor) -> Result<()> {
    let mut archive_path = base_path.clone();
    archive_path.set_extension("tar.gz");
    let tar_gz = File::create(&archive_path)
        .with_context(|| format!("failed to create {}", archive_path.display()))?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(".", &base_path)
        .context("failed to append to tar {}")?;
    tar.into_inner().context("failed to finish tar")?;
    let container_tarfile = archive_path.to_string_lossy().to_string();

    let targz_content = std::fs::read(&container_tarfile).expect("could not read tar");

    debug!("Read {} bytes of tar", targz_content.len());
    // Remove the source directory after tar creation
    std::fs::remove_dir_all(&base_path)
        .with_context(|| format!("failed to remove results directory {}", base_path.display()))?;

    let copy_to_host: JoinHandle<()> = host_executor.spawn_in_host_ns(async move {
        // Write tar.gz to host
        tokio::fs::write(&container_tarfile, targz_content)
            .await
            .expect("could not write tar to host");

        println!(
            "{} {} Report saved: {}",
            SPARKLE,
            style("All Done!").green(),
            style(container_tarfile).cyan()
        );
    });

    Ok(copy_to_host.await?)
}

fn create_base_path(base_dir: PathBuf, hostname: &str, stage: &str) -> Result<PathBuf> {
    let now = Utc::now();

    let base_path = base_dir.join(format!(
        "edera-{}-report-{}-{}",
        stage,
        hostname,
        now.format("%Y%m%d-%H%M%S")
    ));
    fs::create_dir_all(&base_path)
        .with_context(|| format!("could not create {}", base_path.display()))?;
    debug!("Writing all files to {}", base_path.to_string_lossy());
    Ok(base_path)
}

fn write_group_report(
    group: Box<dyn CheckGroup>,
    result: &CheckGroupResult,
    path: &Path,
) -> Result<()> {
    let path = path.join(group.id());
    fs::create_dir_all(&path).with_context(|| format!("could not create {}", path.display()))?;

    for check in result.results.iter() {
        // Sanitize the name of the check into a flat file. Script Checks default to the path of
        // the script as the name so we need to sanitize.
        let name = check
            .name
            .replace(" ", "_")
            .replace("/", "_")
            .replace(".", "");

        let path = path.join(name);
        match check.output_to_record.as_ref() {
            Some(text) => fs::write(&path, text),
            None => fs::write(&path, format!("{}", check.result)),
        }
        .with_context(|| format!("failed to write to {}", path.display()))?;
    }
    Ok(())
}
