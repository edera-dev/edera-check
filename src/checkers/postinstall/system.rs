use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    disk::available_space,
    host_executor::HostNamespaceExecutor,
};

use async_trait::async_trait;
use bytesize::ByteSize;
use futures::{FutureExt, future::join_all};
use std::path::Path;

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
                match available_space(Path::new(&mount_path)) {
                    Ok(avail) if avail >= free_thresh => Passed,
                    Ok(avail) => Failed(format!(
                        "{} has {} free, at least {} is recommended for ",
                        mount_path,
                        ByteSize(avail),
                        ByteSize(free_thresh),
                    )),
                    Err(e) => Errored(format!(
                        "could not read free space for {}: {}",
                        mount_path, e
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
