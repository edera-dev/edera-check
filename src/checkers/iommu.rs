use crate::helpers::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;
use std::path::Path;

const GROUP_IDENTIFIER: &str = "iommu";
const NAME: &str = "IOMMU Checks";

pub struct IOMMUChecks {
    host_executor: HostNamespaceExecutor,
}

#[cfg(target_arch = "x86_64")]
impl IOMMUChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        IOMMUChecks { host_executor }
    }

    /// Run all the checkers asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.check_iommu().boxed()]).await;

        let mut group_result = Passed;
        for res in results.iter() {
            // Set group result to Failed if we failed and aren't already in an Errored state
            if !matches!(group_result, Errored(_)) && matches!(res.result, Failed(_)) {
                group_result = Failed(String::from("group failed"));
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

    async fn check_iommu(&self) -> CheckResult {
        let name = String::from("IOMMU Support");

        match self
            .host_executor
            .spawn_in_host_ns(async {
                if Path::new("/sys/firmware/acpi/tables/DMAR").exists() {
                    debug!("Found Intel IOMMU");
                    true
                } else if Path::new("/sys/firmware/acpi/tables/IVRS").exists() {
                    debug!("Found AMD IOMMU");
                    true
                } else {
                    false
                }
            })
            .await
        {
            Ok(true) => CheckResult::new(&name, Passed),
            Ok(false) => CheckResult::new(&name, Failed("no IOMMU detected".to_string())),
            Err(e) => {
                debug!("Error: {}", e);
                CheckResult::new(&name, Errored(e.to_string()))
            }
        }
    }
}

// No-op for other archs
// TODO(bml) arm64 IOMMU??
#[cfg(not(target_arch = "x86_64"))]
impl IOMMUChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        IOMMUChecks { host_executor }
    }

    pub async fn run_all(&self) -> CheckGroupResult {
        CheckGroupResult::new(NAME)
    }
}

#[async_trait]
impl CheckGroup for IOMMUChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "IOMMU capability checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }
}
