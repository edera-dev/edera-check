use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
    kernel as khelper,
};

use async_trait::async_trait;
use futures::{FutureExt, future::join_all};

const GROUP_IDENTIFIER: &str = "kernel";
const NAME: &str = "Kernel Checks";
// TODO (bml) assemble actual list
const REQUIRED_MODULES: &[&str] = &["nf_tables", "msr"];

pub struct KernelChecks {
    host_executor: HostNamespaceExecutor,
}

impl KernelChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        KernelChecks { host_executor }
    }

    /// Run all the checkers asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.has_modules().boxed()]).await;

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

    /// Checks that `nf_tables` and `msr` are either built into or loaded by the running kernel.
    ///
    /// Manual equivalent:
    /// ```sh
    /// grep -E 'nf_tables|msr' /lib/modules/$(uname -r)/modules.builtin
    /// grep -E '^nf_tables |^msr ' /proc/modules
    /// ```
    pub async fn has_modules(&self) -> CheckResult {
        let name = String::from("Host Has Necessary Modules");

        let required_modules: Vec<String> =
            REQUIRED_MODULES.iter().map(|s| s.to_string()).collect();

        // Search builtin modules
        let remaining = match khelper::find_builtins(&self.host_executor, &required_modules).await {
            Ok(r) => r,
            Err(e) => {
                return CheckResult::new(&name, Errored(format!("getting kernel builtins {e}")));
            }
        };

        // Search loaded modules
        let remaining = match khelper::find_loaded(&self.host_executor, &remaining).await {
            Ok(r) => r,
            Err(e) => {
                return CheckResult::new(&name, Errored(format!("getting kernel modules {e}")));
            }
        };
        if !remaining.is_empty() {
            return CheckResult::new(&name, Failed(format!("missing {:?}", remaining)));
        }

        CheckResult::new(&name, Passed)
    }
}

#[async_trait]
impl CheckGroup for KernelChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Kernel requirement checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Required
    }
}
