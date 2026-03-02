use edera_check::checkers::preinstall::{
    byo_kernel::BYOKernelChecks, iommu::IOMMUChecks, kernel::KernelChecks, numa::NUMAChecks,
    pvh::PVHChecks, system::SystemChecks,
};

use crate::{booted_under_edera, create_base_path, create_gzip_from, write_group_report};
use anyhow::Result;
use console::style;

use edera_check::helpers::{
    CheckGroup, CheckGroupCategory,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

use edera_check::recorders::preinstall::system::SystemRecorder as prerecorder;

use anyhow::{anyhow, bail};
use std::{collections::HashSet, env, path::PathBuf, process};

pub async fn do_preinstall(
    list_only: bool,
    byo_kernel: bool,
    record_hostinfo: bool,
    only_checks: Vec<String>,
    report_dir: Option<String>,
) -> Result<()> {
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
        Box::new(NUMAChecks::new(host_executor.clone())),
    ];

    if byo_kernel {
        groups.push(Box::new(BYOKernelChecks::new(host_executor.clone())));
    }

    if record_hostinfo {
        groups.push(Box::new(prerecorder::new(host_executor.clone())));
    }

    if list_only {
        let id_w = groups
            .iter()
            .map(|g| g.id().len())
            .max()
            .unwrap_or(0)
            .max("ID".len());
        let cat_w = groups
            .iter()
            .map(|g| g.category().to_string().len())
            .max()
            .unwrap_or(0)
            .max("Category".len());
        println!(
            "Available preinstall check groups (selectively run checks with '--only-checks <ID>'):\n"
        );
        println!("  {:<id_w$}  {:<cat_w$}  Description", "ID", "Category");
        println!(
            "  {}",
            "-".repeat(id_w + 2 + cat_w + 2 + "Description".len())
        );
        for group in &groups {
            let cat = group.category().to_string();
            println!(
                "  {}{}  {}{}  {}",
                style(group.id()).cyan().bold(),
                " ".repeat(id_w - group.id().len()),
                style(&cat).white().bold(),
                " ".repeat(cat_w - cat.len()),
                group.description()
            );
        }
        return Ok(());
    }

    // See if we are already booted under Edera. If so, error out and suggest `postinstall`
    // as the command to run.
    match booted_under_edera(&host_executor).await {
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

    if record_hostinfo {
        println!(
            "Collecting information about the current host as part of locally-generated preinstall report."
        );
        println!("The information collected will remain on this host.");
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
