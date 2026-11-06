use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;

use crate::helpers::{
    BaseHypervisor, CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
    services as svchelpers,
};

const GROUP_IDENTIFIER: &str = "services";
const NAME: &str = "Service Status Checks";

pub struct ServiceChecks {
    host_executor: HostNamespaceExecutor,
    hypervisor: BaseHypervisor,
}

impl ServiceChecks {
    pub fn new(host_executor: HostNamespaceExecutor, hypervisor: BaseHypervisor) -> Self {
        ServiceChecks {
            host_executor,
            hypervisor,
        }
    }

    /// Run all the recorders asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let mut checks = vec![self.check_daemon().boxed()];
        // protect-storage serves 9p for Xen zones; KVM zones use virtiofsd instead.
        if self.hypervisor == BaseHypervisor::Xen {
            checks.push(self.check_storage().boxed());
        }
        checks.push(self.check_network().boxed());
        let results = join_all(checks).await;

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

    /// Checks that the `protect-daemon` service is active.
    ///
    /// Manual equivalent:
    /// ```sh
    /// systemctl is-active protect-daemon
    /// # or on OpenRC: rc-service protect-daemon status
    /// ```
    async fn check_daemon(&self) -> CheckResult {
        let name = "Protect daemon status";
        let sname = "protect-daemon";
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

    /// Checks that the `protect-storage` service is active.
    ///
    /// Manual equivalent:
    /// ```sh
    /// systemctl is-active protect-storage
    /// # or on OpenRC: rc-service protect-storage status
    /// ```
    async fn check_storage(&self) -> CheckResult {
        let name = "Protect storage daemon status";
        let sname = "protect-storage";
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

    /// Checks that the `protect-network` service is active.
    ///
    /// Manual equivalent:
    /// ```sh
    /// systemctl is-active protect-network
    /// # or on OpenRC: rc-service protect-network status
    /// ```
    async fn check_network(&self) -> CheckResult {
        let name = "Protect network daemon status";
        let sname = "protect-network";
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
impl CheckGroup for ServiceChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Check status of required host services"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Required
    }
}
