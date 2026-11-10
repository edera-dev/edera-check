use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
    iommu::hardware_iommu_problem,
};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;

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

    /// Checks for an IOMMU that Xen can enable, from the Intel VT-d (DMAR) or AMD-Vi (IVRS)
    /// ACPI tables and the CPU flags.
    ///
    /// Manual equivalent:
    /// ```sh
    /// ls /sys/firmware/acpi/tables/DMAR 2>/dev/null && echo "Intel VT-d" \
    ///   || ls /sys/firmware/acpi/tables/IVRS 2>/dev/null && echo "AMD-Vi" \
    ///   || echo "no IOMMU"
    /// ```
    async fn check_iommu(&self) -> CheckResult {
        let name = String::from("IOMMU Support");

        match hardware_iommu_problem(&self.host_executor).await {
            Ok(None) => CheckResult::new(&name, Passed),
            Ok(Some(problem)) => CheckResult::new(&name, Failed(problem)),
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
        use crate::helpers::CheckResultValue::Skipped;
        CheckGroupResult {
            name: NAME.to_string(),
            result: Skipped("not supported on this arch".into()),
            results: vec![],
        }
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

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Optional(
            "PVH and some host device support not available on this system".into(),
        )
    }
}
