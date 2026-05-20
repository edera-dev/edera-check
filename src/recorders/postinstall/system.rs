use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use std::path::PathBuf;

use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed, Skipped},
    host_executor::HostNamespaceExecutor,
};
use crate::recorders::common::CommonSystemRecorder;

const GROUP_IDENTIFIER: &str = "sysinfo";
const NAME: &str = "Postinstall System Info Recorder";

pub struct SystemRecorder {
    common: CommonSystemRecorder,
}

impl SystemRecorder {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        SystemRecorder {
            common: CommonSystemRecorder::new(host_executor),
        }
    }

    /// Run all the recorders asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([
            self.common.record_lspci().boxed(),
            self.common.record_dmidecode().boxed(),
            self.common.record_cpuinfo().boxed(),
            self.common.record_cmdline().boxed(),
            self.common.record_grub_cfg().boxed(),
            self.common.record_kernel_cfg().boxed(),
            self.common.record_loaded_modules().boxed(),
            self.common.record_meminfo().boxed(),
            self.common.record_vmstat().boxed(),
            self.common.record_slabinfo().boxed(),
            self.common.record_mounts().boxed(),
            self.common.record_mountinfo().boxed(),
            self.record_hv_console().boxed(),
            self.record_hv_debug_info().boxed(),
            self.record_daemon_logs().boxed(),
            self.record_cri_logs().boxed(),
            self.record_storage_logs().boxed(),
            self.record_kubelet_logs().boxed(),
            self.record_network_logs().boxed(),
            self.record_containerd_logs().boxed(),
            self.record_oxenstored_logs().boxed(),
            self.record_xen_capabilities().boxed(),
            self.record_boot_log().boxed(),
            self.record_orchestrator_logs().boxed(),
            self.record_preinit_logs().boxed(),
            self.record_daemon_toml().boxed(),
        ])
        .await;

        let mut group_result = Passed;
        for res in results.iter() {
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

    /// Records the Xen hypervisor console log via the protect-ctl tool.
    ///
    /// Manual equivalent:
    /// ```sh
    /// protect-ctl host hv-console
    /// ```
    pub async fn record_hv_console(&self) -> CheckResult {
        self.common.run_tool("protect-ctl host hv-console").await
    }

    /// Records the Xen hypervisor debug state via the protect-ctl tool.
    ///
    /// Manual equivalent:
    /// ```sh
    /// protect-ctl host hv-debug-info
    /// ```
    pub async fn record_hv_debug_info(&self) -> CheckResult {
        self.common.run_tool("protect-ctl host hv-debug-info").await
    }

    /// Records the `protect-daemon` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u protect-daemon -o export
    /// ```
    pub async fn record_daemon_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u protect-daemon -o export")
            .await
    }

    /// Records the `protect-cri` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u protect-cri -o export
    /// ```
    pub async fn record_cri_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u protect-cri -o export")
            .await
    }

    /// Records the `protect-storage` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u protect-storage -o export
    /// ```
    pub async fn record_storage_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u protect-storage -o export")
            .await
    }

    /// Records the `protect-network` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u protect-network -o export
    /// ```
    pub async fn record_network_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u protect-network -o export")
            .await
    }

    /// Records the `containerd` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u containerd -o export
    /// ```
    pub async fn record_containerd_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u containerd -o export")
            .await
    }

    /// Records the `oxenstored` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u oxenstored -o export
    /// ```
    pub async fn record_oxenstored_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u oxenstored -o export")
            .await
    }

    /// Records the `kubelet` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u kubelet -o export
    /// ```
    pub async fn record_kubelet_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u kubelet -o export")
            .await
    }

    /// Records the current boot kernel journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -b -o export
    /// ```
    pub async fn record_boot_log(&self) -> CheckResult {
        self.common.run_tool("journalctl -b -o export").await
    }

    /// Records the Xen hypervisor capability string.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /sys/hypervisor/properties/capabilities
    /// ```
    pub async fn record_xen_capabilities(&self) -> CheckResult {
        self.common
            .record_file(PathBuf::from("/sys/hypervisor/properties/capabilities").as_ref())
            .await
            .expect("/sys/hypervisor/properties/capabilities not found")
    }

    /// Records the `protect-orchestrator` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u protect-orchestrator -o export
    /// ```
    pub async fn record_orchestrator_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u protect-orchestrator -o export")
            .await
    }

    /// Records the `protect-preinit` journalctl log.
    ///
    /// Manual equivalent:
    /// ```sh
    /// journalctl -u protect-preinit -o export
    /// ```
    pub async fn record_preinit_logs(&self) -> CheckResult {
        self.common
            .run_tool("journalctl -u protect-preinit -o export")
            .await
    }

    /// Records the Edera Protect daemon configuration.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /var/lib/edera/protect/daemon.toml
    /// ```
    pub async fn record_daemon_toml(&self) -> CheckResult {
        let file = "/var/lib/edera/protect/daemon.toml";
        match self.common.record_file(&PathBuf::from(file)).await {
            Some(result) => result,
            None => CheckResult::new(
                &format!("Captured {file}"),
                Skipped("file not found".to_string()),
            ),
        }
    }
}

#[async_trait]
impl CheckGroup for SystemRecorder {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Record postinstall system information"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Advisory
    }
}
