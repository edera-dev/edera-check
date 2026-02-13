mod checkers;
mod helpers;
mod recorders;

use checkers::{iommu::IOMMUChecks, kernel::KernelChecks, pvh::PVHChecks, system::SystemChecks};
use clap::{Parser, Subcommand};
use console::{Emoji, style};
use helpers::{
    CheckGroup, CheckGroupResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use recorders::system::SystemRecorder;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use flate2::{Compression, write::GzEncoder};
use log::debug;
use std::{
    collections::HashSet,
    env, fs,
    fs::File,
    path::{Path, PathBuf},
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

        /// Run only selected checks, instead of default behavior of running all
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
            byo_kernel: _,
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

            let mut groups: Vec<Box<dyn CheckGroup>> = vec![
                Box::new(SystemChecks::new(host_executor.clone())),
                Box::new(PVHChecks::new(host_executor.clone())),
                Box::new(KernelChecks::new(host_executor.clone())),
                Box::new(IOMMUChecks::new(host_executor.clone())),
            ];

            if record_hostinfo {
                groups.push(Box::new(SystemRecorder::new(host_executor.clone())));
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

            let mut final_result = Passed;

            let hostname = host_executor
                .spawn_in_host_ns(async { std::fs::read_to_string("/etc/hostname").unwrap() })
                .await?;

            let base_dir = if let Some(dir) = report_dir {
                PathBuf::from(dir)
            } else {
                env::temp_dir()
            };

            let base_path = create_base_path(base_dir, hostname.trim())
                .map_err(|e| anyhow!("failed to create bundle base path: {e}"))?;
            // Run each check group
            for group in groups {
                println!(
                    "{} {} - {}",
                    style("Running Group").cyan(),
                    style(group.name()).cyan().bold(),
                    group.description()
                );

                let check_group_result = group.run().await;

                check_group_result.log_group();

                // if env::var("EDERA_PREFLIGHT_VERBOSE").unwrap_or_default() == "true" {
                check_group_result.log_individual_checks();
                // }

                // Set final result to Failed if we failed and aren't already in an Errored state
                if !matches!(final_result, Errored(_))
                    && matches!(check_group_result.result, Failed(_))
                {
                    final_result = Failed(String::from("group failed"));
                }

                if matches!(check_group_result.result, Errored(_)) {
                    final_result = Errored(String::from("group errored"));
                }

                write_group_report(group, &check_group_result, &base_path)?;
            }

            create_gzip_from(base_path, host_executor.clone()).await?;

            match final_result {
                Errored(_) | Failed(_) => bail!("Preflight checks did not pass"),
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

fn create_base_path(base_dir: PathBuf, hostname: &str) -> Result<PathBuf> {
    let now = Utc::now();

    let base_path = base_dir.join(format!(
        "edera-preinstall-report-{}-{}",
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
