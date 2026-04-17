use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

use async_trait::async_trait;
use bytesize::ByteSize;
use futures::{FutureExt, future::join_all};
use log::debug;
use sysinfo::Disks;

const GROUP_IDENTIFIER: &str = "system";
const NAME: &str = "System Checks";
const RECOMMENDED_DISK_VAR: u64 = 20 * 1024 * 1024 * 1024; // 5GB

pub struct SystemChecks {
    host_executor: HostNamespaceExecutor,
}

impl SystemChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        SystemChecks { host_executor }
    }
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self
            .enough_disk("/var/lib".into(), RECOMMENDED_DISK_VAR)
            .boxed()])
        .await;

        let mut group_result = Passed;
        for res in results.iter() {
            // Set group result to Failed if we failed and aren't already in an Errored state
            if !matches!(group_result, Errored(_)) && matches!(res.result, Failed(_)) {
                group_result = Failed("".into());
            }

            if matches!(res.result, Errored(_)) {
                group_result = Errored("".into());
            }
        }

        CheckGroupResult {
            name: NAME.to_string(),
            result: group_result,
            results,
        }
    }

    async fn enough_disk(&self, mount_path: String, free_thresh: u64) -> CheckResult {
        let name = format!("Enough Disk Space for {}", &mount_path);
        let result = match self
            .host_executor
            .spawn_in_host_ns(async move {
                let disks = Disks::new_with_refreshed_list();
                match Self::available_space_for_path(&disks, &mount_path) {
                    Some(avail) if avail >= free_thresh => Passed,
                    Some(avail) => Failed(format!(
                        "{} has {} free, at least {} is recommended for ",
                        mount_path,
                        ByteSize(avail),
                        ByteSize(free_thresh),
                    )),
                    None => Failed(format!(
                        "no mounted filesystem found covering {}",
                        mount_path
                    )),
                }
            })
            .await
        {
            Ok(result) => result,
            Err(e) => Errored(e.to_string()),
        };
        CheckResult::new(&name, result)
    }

    /// Finds available bytes for the filesystem that best covers `path`
    fn available_space_for_path(disks: &Disks, path: &str) -> Option<u64> {
        let target = std::path::Path::new(path);
        disks
            .iter()
            .filter(|d| target.starts_with(d.mount_point()))
            .max_by_key(|d| d.mount_point().as_os_str().len())
            .map(|d| {
                debug!(
                    "Best mount for {}: {} ({} bytes available)",
                    path,
                    d.mount_point().display(),
                    d.available_space()
                );
                d.available_space()
            })
    }
}

#[async_trait]
impl CheckGroup for SystemChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "System checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Advisory
    }
}
