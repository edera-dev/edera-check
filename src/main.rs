mod checkers;
mod helpers;
mod recorders;

use checkers::{kernel::KernelChecks, script::ScriptChecks, system::SystemChecks};
use helpers::{
    CheckGroup, CheckGroupResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use recorders::system::SystemRecorder;

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use flate2::{Compression, write::GzEncoder};
use log::info;
use std::{
    env, fs,
    fs::File,
    path::{Path, PathBuf},
};
use tokio::task::JoinHandle;

// Skip certain groups. List is separated by ;
fn skip_groups() -> Vec<String> {
    let skips = env::var("EDERA_PREFLIGHT_SKIP_GROUPS").unwrap_or_default();
    skips.split(";").map(|s| s.to_string()).collect()
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
    tar.append_dir_all(".", base_path)
        .context("failed to append to tar {}")?;
    tar.into_inner().context("failed to finish tar")?;
    let container_tarfile = archive_path.to_string_lossy().to_string();

    let targz_content = std::fs::read(&container_tarfile).expect("could not read tar");

    info!("Read {} bytes of tar", targz_content.len());

    let copy_to_host: JoinHandle<()> = host_executor.spawn_in_host_ns(async move {
        // Write tar.gz to host
        tokio::fs::write(&container_tarfile, targz_content)
            .await
            .expect("could not write tar to host");

        info!("Wrote to: {}", container_tarfile);
    });

    Ok(copy_to_host.await?)
}

fn create_base_path() -> Result<PathBuf> {
    let now = Utc::now();

    let base = env::var("EDERA_PREFLIGHT_REPORT_DIR")
        .map(PathBuf::from)
        .unwrap_or(env::temp_dir());

    let base_path = base.join(format!(
        "protect-preflight-bundle-{}",
        now.format("%Y%m%d-%H%M%S")
    ));
    fs::create_dir_all(&base_path)
        .with_context(|| format!("could not create {}", base_path.display()))?;
    info!("Writing all files to {}", base_path.to_string_lossy());
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

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    env_logger::init();

    let host_executor = HostNamespaceExecutor::new();

    let groups: Vec<Box<dyn CheckGroup>> = vec![
        Box::new(SystemChecks),
        Box::new(ScriptChecks),
        Box::new(KernelChecks),
        Box::new(SystemRecorder::new(host_executor.clone())),
    ];

    let mut final_result = Passed;
    let skip_groups = skip_groups();

    let base_path =
        create_base_path().map_err(|e| anyhow!("failed to create bundle base path: {e}"))?;

    // Run each check group
    for group in groups {
        // Check if we need to explicity skip this group
        if skip_groups.iter().any(|skip| group.id() == skip) {
            continue;
        }

        info!("Running Group [{}] - {}", group.name(), group.description());

        let check_group_result = group.run().await;

        check_group_result.log_group();

        if env::var("EDERA_PREFLIGHT_VERBOSE").unwrap_or_default() == "true" {
            check_group_result.log_individual_checks();
        }

        // Set final result to Failed if we failed and aren't already in an Errored state
        if !matches!(final_result, Errored(_)) && matches!(check_group_result.result, Failed(_)) {
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
