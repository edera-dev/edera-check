use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

use async_trait::async_trait;
use bytesize::ByteSize;
use log::debug;
use sysinfo::System;

const GROUP_IDENTIFIER: &str = "memory";
const NAME: &str = "Memory Checks";
const MINIMUM_MEMORY: u64 = 4 * 1024 * 1024 * 1024; // 4GB

pub struct MemoryChecks {
    host_executor: HostNamespaceExecutor,
}

impl MemoryChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        MemoryChecks { host_executor }
    }

    /// Checks that total system RAM is at least 4 GB.
    ///
    /// Manual equivalent:
    /// ```sh
    /// awk '/MemTotal/ { if ($2 >= 4194304) print "OK"; else print "FAIL" }' /proc/meminfo
    /// ```
    async fn enough_memory(&self) -> CheckResult {
        let name = String::from("Enough Memory");

        let total_mem = match self
            .host_executor
            .spawn_in_host_ns(async {
                let mut sys = System::new_all()?;
                sys.refresh_all();

                Ok::<_, sysinfo::Error>(sys.total_memory())
            })
            .await
        {
            Ok(Ok(mem)) => mem,
            Ok(Err(e)) => {
                return CheckResult::new(&name, Errored(e.to_string()));
            }
            Err(e) => {
                return CheckResult::new(&name, Errored(e.to_string()));
            }
        };

        debug!("total memory = {total_mem}");

        let mut result = Passed;
        if total_mem < MINIMUM_MEMORY {
            let reason = format!(
                "total memory is {} but need at least {}",
                ByteSize(total_mem),
                ByteSize(MINIMUM_MEMORY)
            );
            result = Failed(reason);
        }
        CheckResult::new(&name, result)
    }
}

#[async_trait]
impl CheckGroup for MemoryChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Memory requirement checks"
    }

    async fn run(&self) -> CheckGroupResult {
        let check = self.enough_memory().await;
        let result = match &check.result {
            Failed(_) => Failed("".into()),
            Errored(_) => Errored("".into()),
            _ => Passed,
        };

        CheckGroupResult {
            name: NAME.to_string(),
            result,
            results: vec![check],
        }
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Required
    }
}
