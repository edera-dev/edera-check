use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;
use sysinfo::{Disks, System};

const GROUP_IDENTIFIER: &str = "system";
const NAME: &str = "System Checks";
const MINIMUM_MEMORY: u64 = 4 * 1024 * 1024 * 1024; // 4GB
const MINIMUM_DISK: u64 = 20 * 1024 * 1024 * 1024; // 20GB

pub struct SystemChecks {
    host_executor: HostNamespaceExecutor,
}

impl SystemChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        SystemChecks { host_executor }
    }
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.enough_memory().boxed(), self.enough_disk().boxed()]).await;

        let mut group_result = Failed("".into());
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

    async fn enough_memory(&self) -> CheckResult {
        let name = String::from("Enough Memory");

        let total_mem = match self
            .host_executor
            .spawn_in_host_ns(async {
                let mut sys = System::new_all();
                sys.refresh_all();

                sys.total_memory()
            })
            .await
        {
            Ok(mem) => mem,
            Err(e) => {
                return CheckResult::new(&name, Errored(e.to_string()));
            }
        };

        debug!("total memory = {total_mem}");

        let mut result = Passed;
        if total_mem < MINIMUM_MEMORY {
            let reason = format!("total memory is less than {}", MINIMUM_MEMORY);
            result = Failed(reason);
        }
        CheckResult::new(&name, result)
    }

    async fn enough_disk(&self) -> CheckResult {
        let name = String::from("Enough Disk");

        let result = match self
            .host_executor
            .spawn_in_host_ns(async {
                let mut result = Failed(String::from("Not enough disk space on any disk"));
                let disks = Disks::new_with_refreshed_list();
                for disk in &disks {
                    if disk.available_space() < MINIMUM_DISK {
                        debug!(
                            "Not enough space on disk mounted at {} - {}",
                            disk.mount_point().display(),
                            disk.available_space()
                        );
                    } else {
                        debug!(
                            "Enough space on disk mounted at {} - {}",
                            disk.mount_point().display(),
                            disk.available_space()
                        );
                        result = Passed;
                    }
                }
                result
            })
            .await
        {
            Ok(result) => result,
            Err(e) => {
                return CheckResult::new(&name, Errored(e.to_string()));
            }
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
        "System requirement checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Required
    }
}
