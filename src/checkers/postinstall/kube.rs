use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;

use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
    services as svchelpers,
};

const GROUP_IDENTIFIER: &str = "kubernetes";
const NAME: &str = "Kubernetes Capability Checks";

pub struct KubeChecks {
    host_executor: HostNamespaceExecutor,
}

impl KubeChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        KubeChecks { host_executor }
    }

    /// Run all the recorders asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.check_cri().boxed(), self.check_kubelet().boxed()]).await;

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

    /// Checks that the `protect-cri` container runtime interface service is active.
    ///
    /// Manual equivalent:
    /// ```sh
    /// systemctl is-active protect-cri
    /// # or on OpenRC: rc-service protect-cri status
    /// ```
    async fn check_cri(&self) -> CheckResult {
        let name = "Protect CRI daemon status";
        let sname = "protect-cri";
        let init = svchelpers::detect_init_system(&self.host_executor).await;
        debug!("detected init system: {:?}\n", init);

        match svchelpers::is_running(&self.host_executor, sname.into(), init).await {
            Ok(true) => CheckResult::new(name, Passed),
            Ok(false) => CheckResult::new(name, Failed(format!("{} not running", &sname))),
            Err(e) => CheckResult::new(
                name,
                Errored(format!("failed to check service {sname}: {e}")),
            ),
        }
    }

    /// Checks that the `kubelet` service is active.
    ///
    /// Manual equivalent:
    /// ```sh
    /// systemctl is-active kubelet
    /// # or on OpenRC: rc-service kubelet status
    /// ```
    async fn check_kubelet(&self) -> CheckResult {
        let name = "kubelet status";
        let sname = "kubelet";
        let init = svchelpers::detect_init_system(&self.host_executor).await;
        debug!("detected init system: {:?}\n", init);

        match svchelpers::is_running(&self.host_executor, sname.into(), init).await {
            Ok(true) => CheckResult::new(name, Passed),
            Ok(false) => CheckResult::new(name, Failed(format!("{} not running", &sname))),
            Err(e) => CheckResult::new(
                name,
                Errored(format!("failed to check service {sname}: {e}")),
            ),
        }
    }
}

#[async_trait]
impl CheckGroup for KubeChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Check status of Kubernetes integration"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Optional("Kubernetes feature not available on this system".into())
    }
}
