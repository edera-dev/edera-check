use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

use anyhow::{Result, bail};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::debug;
use procfs::{Current, sys::kernel};
use std::{fs, path::PathBuf, process::Command};

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

    async fn version_is_good(&self) -> CheckResult {
        let name = String::from("Host Kernel Version Is Good");
        let mut result = Passed;

        // Get host kernel version
        let current = self
            .host_executor
            .spawn_in_host_ns(async { kernel::Version::current() })
            .await
            .expect("error spawning in host");

        if let Err(e) = current {
            return CheckResult::new(&name, Errored(e.to_string()));
        }
        let current = current.unwrap();
        let lowest = kernel::Version::new(KVER_FLOOR_MAJOR, KVER_FLOOR_MINOR, KVER_FLOOR_PATCH);

        if current < lowest {
            result = Failed(String::from("current kernel version is unsupported"));
        }
        CheckResult::new(&name, result)
    }

    async fn has_modules(&self) -> CheckResult {
        let name = String::from("Host Has Necessary Modules");
        let mut result = Passed;

        let required_modules: Vec<String> =
            REQUIRED_MODULES.iter().map(|s| s.to_string()).collect();

        // Search builtin modules
        let remaining = match self.find_builtins(&required_modules).await {
            Ok(r) => r,
            Err(e) => {
                return CheckResult::new(&name, Errored(format!("getting kernel builtins {e}")));
            }
        };

        // Search loaded modules
        let remaining = match self.find_loaded(&remaining).await {
            Ok(r) => r,
            Err(e) => {
                return CheckResult::new(&name, Errored(format!("getting kernel modules {e}")));
            }
        };

        // Search loadable modules
        let remaining = match self.find_loadable(&remaining).await {
            Ok(r) => r,
            Err(e) => {
                return CheckResult::new(&name, Errored(format!("getting kernel modules {e}")));
            }
        };
        if !remaining.is_empty() {
            result = Failed(format!("missing {:?}", remaining))
        }

        CheckResult::new(&name, result)
    }

    /// Looks at builtins for kernel_version and compares that to the list of
    /// required modules.
    /// Returns a vec of everything from required_modules that WAS NOT found in builtins.
    async fn find_builtins(&self, required_modules: &[String]) -> Result<Vec<String>> {
        let mut modules_to_find: Vec<String> = required_modules.to_owned();

        // read host builtins
        let builtins = self
            .host_executor
            .spawn_in_host_ns(async move {
                // Get kernel version
                let output = Command::new("uname").arg("-r").output()?;

                if !output.status.success() {
                    let error_message = String::from_utf8_lossy(&output.stderr);
                    bail!("{}", error_message);
                }
                let kernel_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let path = PathBuf::from(format!("/lib/modules/{kernel_version}/modules.builtin"));
                fs::read_to_string(path).map_err(|e| anyhow::anyhow!(e))
            })
            .await??;

        for builtin in builtins.lines() {
            let found = modules_to_find
                .iter()
                .position(|required| builtin.contains(required));

            if let Some(index) = found {
                debug!("builtin {}", modules_to_find[index]);
                modules_to_find.remove(index);
            }
        }

        Ok(modules_to_find)
    }

    /// Looks at loaded modules for the current host kernel and compares that to the list of
    /// required modules.
    /// Returns a vec of everything from required_modules that WAS NOT loaded.
    async fn find_loaded(&self, required_modules: &[String]) -> Result<Vec<String>> {
        let mut modules_to_find: Vec<String> = required_modules.to_owned();

        let modules = self
            .host_executor
            .spawn_in_host_ns(async move { procfs::KernelModules::current() })
            .await?;

        let modules = modules.unwrap();

        for (name, _) in modules.0.iter() {
            let found = modules_to_find.iter().position(|required| required == name);

            if let Some(index) = found {
                debug!("module {}", modules_to_find[index]);
                modules_to_find.remove(index);
            }
        }

        Ok(modules_to_find)
    }

    /// Looks at not-loaded-but-loadable modules for the current host kernel and compares
    /// that to the list of required modules.
    /// Returns a vec of everything from required_modules that is available to load (exists in
    /// modules.dep) but is NOT currently loaded or builtin.
    async fn find_loadable(&self, required_modules: &[String]) -> Result<Vec<String>> {
        let mut modules_to_find: Vec<String> = required_modules.to_owned();
        let dep_file = self
            .host_executor
            .spawn_in_host_ns(async move {
                let output = Command::new("uname").arg("-r").output()?;
                if !output.status.success() {
                    let error_message = String::from_utf8_lossy(&output.stderr);
                    bail!("{}", error_message);
                }
                let kernel_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let path = PathBuf::from(format!("/lib/modules/{kernel_version}/modules.dep"));
                fs::read_to_string(path).map_err(|e| anyhow::anyhow!(e))
            })
            .await??;

        for line in dep_file.lines() {
            let module_path = line.split(':').next().unwrap_or("");
            let found = modules_to_find
                .iter()
                .position(|required| module_path.contains(required.as_str()));
            if let Some(index) = found {
                debug!("available {}", modules_to_find[index]);
                modules_to_find.remove(index);
            }
        }
        Ok(modules_to_find)
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
