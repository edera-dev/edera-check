use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;

const GROUP_IDENTIFIER: &str = "numa";
const NAME: &str = "NUMA Checks";

pub struct NUMAChecks {
    host_executor: HostNamespaceExecutor,
}

#[cfg(target_arch = "x86_64")]
impl NUMAChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        NUMAChecks { host_executor }
    }

    /// Run all the checkers asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.check_numa().boxed()]).await;

        let mut group_result = Passed;
        for res in results.iter() {
            // Set group result to Failed if we failed and aren't already in an Errored state
            if !matches!(group_result, Errored(_)) && matches!(res.result, Failed(_)) {
                group_result = Failed(String::from("group errored"));
            }

            if matches!(res.result, Errored(_)) {
                group_result = Errored(String::from("group errored"));
            }
        }

        CheckGroupResult {
            name: NAME.to_string(),
            result: group_result,
            results,
        }
    }

    async fn check_numa(&self) -> CheckResult {
        let name = String::from("IOMMU Support");

        match self
            .host_executor
            .spawn_in_host_ns(async {
                std::fs::read_dir("/sys/devices/system/node")
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .filter(|e| {
                        let name = e.file_name();
                        let s = name.to_string_lossy();
                        s.strip_prefix("node")
                            .map(|rest| rest.chars().all(|c| c.is_ascii_digit()))
                            .unwrap_or(false)
                    })
                    .count()
            })
            .await
        {
            Ok(count) => {
                if count <= 1 {
                    CheckResult::new(&name, Passed)
                } else {
                    CheckResult::new(&name, Failed(format!("{} NUMA nodes detected", count)))
                }
            }
            Err(e) => {
                debug!("Error: {}", e);
                CheckResult::new(&name, Errored(e.to_string()))
            }
        }
    }
}

// No-op for other archs
// TODO(bml) arm64 NUMA??
#[cfg(not(target_arch = "x86_64"))]
impl NUMAChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        NUMAChecks { host_executor }
    }

    pub async fn run_all(&self) -> CheckGroupResult {
        CheckGroupResult::new(NAME)
    }
}

#[async_trait]
impl CheckGroup for NUMAChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "NUMA capability checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Advisory
    }
}
