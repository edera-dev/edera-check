use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use std::{fs, path::Path};

const GROUP_IDENTIFIER: &str = "guesttype";
const NAME: &str = "Guest Support Checks";

pub struct GuestTypeChecks {
    host_executor: HostNamespaceExecutor,
}

impl GuestTypeChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        GuestTypeChecks { host_executor }
    }

    /// Run all the checkers asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.check_guest_support().boxed()]).await;

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

    async fn check_guest_support(&self) -> CheckResult {
        let name = String::from("Guest Type Support");
        match self
            .host_executor
            .spawn_in_host_ns(async {
                let xen = Path::new("/sys/hypervisor/guest_type");
                xen.exists() && fs::read_to_string(xen).unwrap_or_default().trim() == "PVH"
            })
            .await
        {
            Ok(true) => CheckResult::new(&name, Passed),
            Ok(false) => CheckResult::new(&name, Failed("PVH guests not supported".into())),
            Err(e) => CheckResult::new(&name, Errored(format!("Error: {}", e))),
        }
    }
}

#[async_trait]
impl CheckGroup for GuestTypeChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Supported guest type checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Optional("PVH guest support not available on this system".into())
    }
}
