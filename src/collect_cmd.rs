use crate::{create_base_path, create_gzip_from, write_group_report};
use edera_check::recorders::postinstall::system::SystemRecorder as postrecorder;

use anyhow::{Result, anyhow};
use console::style;
use edera_check::helpers::{CheckGroup, host_executor::HostNamespaceExecutor};
use std::{env, path::PathBuf};

pub async fn do_collect(report_dir: Option<String>) -> Result<()> {
    let host_executor = HostNamespaceExecutor::new();

    let groups: Vec<Box<dyn CheckGroup>> = vec![Box::new(postrecorder::new(host_executor.clone()))];

    let hostname = host_executor
        .spawn_in_host_ns(async { std::fs::read_to_string("/proc/sys/kernel/hostname").unwrap() })
        .await?;

    let base_dir = if let Some(dir) = report_dir {
        PathBuf::from(dir)
    } else {
        env::temp_dir()
    };

    let base_path = create_base_path(base_dir, hostname.trim(), "collect")
        .map_err(|e| anyhow!("failed to create bundle base path: {e}"))?;

    for group in groups {
        println!(
            "{} {} [{}] - {}",
            style("Running Group").cyan(),
            style(group.name()).cyan().bold(),
            style(group.category()).white().bold(),
            group.description()
        );
        let result = group.run().await;
        result.log_individual_checks();
        write_group_report(group, &result, &base_path)?;
    }

    create_gzip_from(base_path, host_executor).await
}
