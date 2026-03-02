mod checkers;
mod helpers;
mod postinstall_cmd;
mod preinstall_cmd;
mod recorders;

use clap::{Args, Parser, Subcommand};
use console::{Emoji, style};
use helpers::{CheckGroup, CheckGroupResult, host_executor::HostNamespaceExecutor};

use anyhow::{Context, Result};
use chrono::Utc;
use flate2::{Compression, write::GzEncoder};
use log::debug;
use nix::unistd::Uid;
use std::{
    fs,
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
enum PreinstallAction {
    /// List available check groups and their IDs.
    ListChecks,
}

#[derive(Subcommand)]
enum PostinstallAction {
    /// List available check groups and their IDs.
    ListChecks,
}

#[derive(Args)]
struct PreinstallArgs {
    #[command(subcommand)]
    action: Option<PreinstallAction>,

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
}

#[derive(Args)]
struct PostinstallArgs {
    #[command(subcommand)]
    action: Option<PostinstallAction>,

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
}

#[derive(Subcommand)]
enum Commands {
    /// Run before installing Edera to validate hardware/host installation readiness.
    Preinstall(PreinstallArgs),
    /// Run after installing Edera to validate workload readiness.
    Postinstall(PostinstallArgs),
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    env_logger::init();

    if !Uid::effective().is_root() {
        println!("{}", style("This tool must be run as root").red().bold());
        process::exit(1);
    }

    let cli = Cli::parse();
    match cli.command {
        Commands::Preinstall(args) => {
            preinstall_cmd::do_preinstall(
                matches!(args.action, Some(PreinstallAction::ListChecks)),
                args.byo_kernel,
                args.record_hostinfo,
                args.only_checks,
                args.report_dir,
            )
            .await
        }
        Commands::Postinstall(args) => {
            postinstall_cmd::do_postinstall(
                matches!(args.action, Some(PostinstallAction::ListChecks)),
                args.record_hostinfo,
                args.only_checks,
                args.report_dir,
            )
            .await
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

async fn booted_under_edera(host_executor: &HostNamespaceExecutor) -> Result<bool> {
    host_executor
        .spawn_in_host_ns(async {
            if !Path::new("/var/lib/edera/protect/.install-completed").exists() {
                return false;
            }
            let xen = Path::new("/sys/hypervisor/type");
            xen.exists() && fs::read_to_string(xen).unwrap_or_default().trim() == "xen"
        })
        .await
        .context("failed to check Edera boot status")
}
