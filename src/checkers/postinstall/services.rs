use anyhow::{Result, bail};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;
use std::process::Command;

use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

const GROUP_IDENTIFIER: &str = "services";
const NAME: &str = "Service Status Checks";

#[derive(Debug, Clone, Copy, PartialEq)]
enum InitSystem {
    Systemd,
    OpenRC,
    Unknown,
}

pub struct ServiceChecks {
    host_executor: HostNamespaceExecutor,
}

impl ServiceChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        ServiceChecks { host_executor }
    }

    /// Run all the recorders asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([
            self.check_daemon().boxed(),
            self.check_storage().boxed(),
            self.check_network().boxed(),
        ])
        .await;

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

    async fn check_daemon(&self) -> CheckResult {
        let name = "Protect daemon status";
        let sname = "protect-daemon";
        let init = self.detect_init_system().await;
        debug!("detected init system: {:?}\n", init);

        match self.is_running(sname.into(), init).await {
            Ok(true) => CheckResult::new(name, Passed),
            Ok(false) => CheckResult::new(name, Failed(format!("{} not running", &sname))),
            Err(e) => CheckResult::new(
                name,
                Errored(format!("failed to check service {sname}: {e}")),
            ),
        }
    }

    async fn check_storage(&self) -> CheckResult {
        let name = "Protect storage daemon status";
        let sname = "protect-storage";
        let init = self.detect_init_system().await;
        debug!("detected init system: {:?}\n", init);

        match self.is_running(sname.into(), init).await {
            Ok(true) => CheckResult::new(name, Passed),
            Ok(false) => CheckResult::new(name, Failed(format!("{} not running", &sname))),
            Err(e) => CheckResult::new(
                name,
                Errored(format!("failed to check service {sname}: {e}")),
            ),
        }
    }

    async fn check_network(&self) -> CheckResult {
        let name = "Protect network daemon status";
        let sname = "protect-network";
        let init = self.detect_init_system().await;
        debug!("detected init system: {:?}\n", init);

        match self.is_running(sname.into(), init).await {
            Ok(true) => CheckResult::new(name, Passed),
            Ok(false) => CheckResult::new(name, Failed(format!("{} not running", &sname))),
            Err(e) => CheckResult::new(
                name,
                Errored(format!("failed to check service {sname}: {e}")),
            ),
        }
    }

    async fn detect_init_system(&self) -> InitSystem {
        self.host_executor
            .spawn_in_host_ns(async move {
                // Check if systemd is PID 1
                if let Ok(output) = Command::new("ps").args(["-p", "1", "-o", "comm="]).output() {
                    let comm = String::from_utf8_lossy(&output.stdout);
                    if comm.trim() == "systemd" {
                        return InitSystem::Systemd;
                    }
                }

                // otherwise, check if rc-service exists (OpenRC)
                if Command::new("which")
                    .arg("rc-service")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false)
                {
                    return InitSystem::OpenRC;
                }
                InitSystem::Unknown
            })
            .await
            .unwrap_or(InitSystem::Unknown)
    }

    async fn is_running(&self, service: String, init: InitSystem) -> Result<bool> {
        self.host_executor
            .spawn_in_host_ns(async move {
                match init {
                    InitSystem::Systemd => {
                        let status = Command::new("systemctl")
                            .args(["is-active", "--quiet", &service])
                            .status()?;
                        Ok(status.success())
                    }
                    InitSystem::OpenRC => {
                        let status = Command::new("rc-service")
                            .args([&service, "status"])
                            .status()?;
                        Ok(status.success())
                    }
                    InitSystem::Unknown => bail!("unknown init system"),
                }
            })
            .await?
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
