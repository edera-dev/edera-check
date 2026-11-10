mod collect_cmd;
mod postinstall_cmd;
mod preinstall_cmd;

use clap::{ArgAction, Args, Parser, Subcommand};
use console::{Emoji, style};
use edera_check::helpers::{
    BaseHypervisor, CheckGroup, CheckGroupResult, RecordedOutput,
    host_executor::HostNamespaceExecutor, report_file_name,
};

use anyhow::{Context, Result};
use chrono::Utc;
use flate2::{Compression, write::GzEncoder};
use log::debug;
use nix::unistd::Uid;
use std::{
    fs,
    path::{Path, PathBuf},
    process,
};

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
    #[arg(short, long, default_value_t = true, action = ArgAction::Set, num_args = 0..=1,
          require_equals = true, default_missing_value = "true")]
    record_hostinfo: bool,

    /// Which hypervisor to run checks against
    #[arg(long, default_value = "xen")]
    hypervisor: BaseHypervisor,

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
    #[arg(short, long, default_value_t = true, action = ArgAction::Set, num_args = 0..=1,
          require_equals = true, default_missing_value = "true")]
    record_hostinfo: bool,

    /// Run only selected checks, instead of default behavior of running all.
    /// Will override all other check enablement flags.
    #[arg(short, long, value_delimiter = ',')]
    only_checks: Vec<String>,

    /// Directory path to write report to. Will be created if it doesn't exist. Defaults to `/tmp`
    #[arg(short = 'd', long)]
    report_dir: Option<String>,

    #[command(flatten)]
    journal: JournalArgs,
}

#[derive(Args)]
struct CollectArgs {
    /// Directory path to write report to. Will be created if it doesn't exist. Defaults to `/tmp`
    #[arg(short = 'd', long)]
    report_dir: Option<String>,

    #[command(flatten)]
    journal: JournalArgs,
}

#[derive(Args)]
struct JournalArgs {
    /// Only record journal entries since this time, in any `journalctl --since` format
    #[arg(long, default_value = "-24h", allow_hyphen_values = true)]
    journal_since: String,
}

#[derive(Subcommand)]
enum Commands {
    #[command(
        about = "Run before installing Edera to validate hardware/host installation readiness.\nAlso collects a system information snapshot."
    )]
    Preinstall(PreinstallArgs),
    #[command(
        about = "Run after installing Edera to validate workload readiness.\nAlso collects a system information snapshot."
    )]
    Postinstall(PostinstallArgs),
    #[command(about = "Run after installing Edera to collect a system information snapshot.")]
    Collect(CollectArgs),
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
                args.hypervisor,
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
                args.journal.journal_since,
            )
            .await
        }
        Commands::Collect(args) => {
            collect_cmd::do_collect(args.report_dir, args.journal.journal_since).await
        }
    }
}

async fn create_gzip_from(base_path: PathBuf, host_executor: HostNamespaceExecutor) -> Result<()> {
    let mut archive_path = base_path.clone();
    archive_path.set_extension("tar.gz");

    let collected = base_path.clone();
    let targz_content = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        {
            let enc = GzEncoder::new(&mut buf, Compression::default());
            let mut tar = tar::Builder::new(enc);
            tar.append_dir_all(".", &base_path)
                .context("failed to append to tar")?;
            tar.into_inner()
                .context("failed to finish tar")?
                .finish()
                .context("failed to finish gzip encoder")?;
        }
        Ok(buf)
    })
    .await??;

    debug!("Read {} bytes of tar", targz_content.len());

    let host_archive = archive_path.clone();
    host_executor
        .spawn_in_host_ns(async move {
            // The report directory was created in this process's mount namespace, which in
            // a container is not the host's.
            if let Some(parent) = host_archive.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .with_context(|| format!("could not create {} on host", parent.display()))?;
            }
            tokio::fs::write(&host_archive, targz_content)
                .await
                .with_context(|| format!("could not write {} to host", host_archive.display()))
        })
        .await??;

    fs::remove_dir_all(&collected)
        .with_context(|| format!("failed to remove results directory {}", collected.display()))?;

    println!(
        "{} {} Report saved: {}",
        SPARKLE,
        style("All Done!").green(),
        style(archive_path.display()).cyan()
    );
    Ok(())
}

fn report_base_path(base_dir: PathBuf, hostname: &str, stage: &str) -> PathBuf {
    let now = Utc::now();

    base_dir.join(format!(
        "edera-{}-report-{}-{}",
        stage,
        hostname,
        now.format("%Y%m%d-%H%M%S")
    ))
}

fn create_base_path(base_path: &Path) -> Result<()> {
    fs::create_dir_all(base_path)
        .with_context(|| format!("could not create {}", base_path.display()))?;
    debug!("Writing all files to {}", base_path.to_string_lossy());
    Ok(())
}

fn write_group_report(
    group: Box<dyn CheckGroup>,
    result: &CheckGroupResult,
    path: &Path,
) -> Result<()> {
    let path = path.join(group.id());
    fs::create_dir_all(&path).with_context(|| format!("could not create {}", path.display()))?;

    for check in result.results.iter() {
        let path = path.join(report_file_name(&check.name));
        match check.output_to_record.as_ref() {
            Some(RecordedOutput::Text(text)) => fs::write(&path, text),
            Some(RecordedOutput::Streamed) => Ok(()),
            None => fs::write(&path, format!("{}", check.result)),
        }
        .with_context(|| format!("failed to write to {}", path.display()))?;
    }
    Ok(())
}

/// Returns the hypervisor Edera runs on, picked the way protect-daemon picks its runtime: Xen
/// when booted under Xen, otherwise KVM only if the KVM runtime was installed.
async fn booted_under_edera(
    host_executor: &HostNamespaceExecutor,
) -> Result<Option<BaseHypervisor>> {
    host_executor
        .spawn_in_host_ns(async {
            if !Path::new("/var/lib/edera/protect/.install-completed").exists() {
                return None;
            }
            let xen = Path::new("/sys/hypervisor/type");
            if xen.exists() && fs::read_to_string(xen).unwrap_or_default().trim() == "xen" {
                return Some(BaseHypervisor::Xen);
            }
            [
                "/usr/libexec/edera-protect/cloud-hypervisor",
                "/usr/libexec/edera-protect/virtiofsd",
            ]
            .iter()
            .all(|p| Path::new(p).exists())
            .then_some(BaseHypervisor::Kvm)
        })
        .await
        .context("failed to check Edera boot status")
}
