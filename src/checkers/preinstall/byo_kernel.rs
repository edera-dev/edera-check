use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
    kernel as khelper,
};

use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use procfs::sys::kernel;

const GROUP_IDENTIFIER: &str = "byokernel";
const NAME: &str = "Bring-Your-Own Kernel Checks";
// Modules that the currently running kernel must have as loaded/builtin/loadable
// in order for it to be usable as a BYO kernel
const REQUIRED_MODULES: &[&str] = &[
    "nf_tables",
    "xen_evtchn",
    "xen-privcmd",
    "xen-netback",
    "xen-pciback",
    "xen-blkback",
    "xen-gntdev",
    "xen-gntalloc",
];

const KVER_FLOOR_PATCH: u16 = 0;
const KVER_FLOOR_MINOR: u8 = 15;
const KVER_FLOOR_MAJOR: u8 = 5;

pub struct BYOKernelChecks {
    host_executor: HostNamespaceExecutor,
}

impl BYOKernelChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        BYOKernelChecks { host_executor }
    }

    /// Run all the checkers asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.has_modules().boxed(), self.version_is_good().boxed()]).await;

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

    /// Checks that the running kernel is at least version 5.15.0.
    ///
    /// Manual equivalent:
    /// ```sh
    /// uname -r  # must be >= 5.15.0
    /// ```
    async fn version_is_good(&self) -> CheckResult {
        let name = String::from("Host Kernel Version Is Good");
        let floor = kernel::Version::new(KVER_FLOOR_MAJOR, KVER_FLOOR_MINOR, KVER_FLOOR_PATCH);

        let passed = khelper::host_kver_above_floor(&self.host_executor, floor).await;

        match passed {
            Err(e) => CheckResult::new(&name, Errored(e.to_string())),
            Ok(true) => CheckResult::new(&name, Passed),
            Ok(false) => CheckResult::new(
                &name,
                Failed(String::from("current kernel version is unsupported")),
            ),
        }
    }

    /// Checks that all required Xen and networking modules are available as built-in,
    /// currently loaded, or loadable (present in `modules.dep`) for the running kernel.
    ///
    /// Manual equivalent:
    /// ```sh
    /// KV=$(uname -r)
    /// for mod in nf_tables xen_evtchn xen-privcmd xen-netback xen-pciback xen-blkback xen-gntdev xen-gntalloc; do
    ///   grep -q "$mod" /lib/modules/$KV/modules.builtin \
    ///     || grep -q "^${mod} " /proc/modules \
    ///     || grep -q "$mod" /lib/modules/$KV/modules.dep \
    ///     && echo "$mod: OK" || echo "$mod: MISSING"
    /// done
    /// ```
    async fn has_modules(&self) -> CheckResult {
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

        // Search loadable modules
        let remaining = match khelper::find_loadable(&self.host_executor, &remaining).await {
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
impl CheckGroup for BYOKernelChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Bring Your Own Kernel requirement checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Optional(
            "Active kernel not sufficient for Bring-Your-Own-Kernel support".into(),
        )
    }
}
