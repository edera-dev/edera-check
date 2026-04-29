use async_trait::async_trait;
use futures::{FutureExt, future::join_all};

use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};
use crate::recorders::common::CommonSystemRecorder;

const GROUP_IDENTIFIER: &str = "sysinfo";
const NAME: &str = "System Info Recorder";

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
            self.common.record_nftables_ruleset().boxed(),
            self.common.record_links().boxed(),
            self.common.record_routes().boxed(),
            self.common.record_neighbours().boxed(),
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
        "Record system information for reporting purposes"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Advisory
    }
}
