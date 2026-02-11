use crate::helpers::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use anyhow::{Result, bail};
use async_trait::async_trait;
use log::debug;
use procfs::{Current, sys::kernel};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const GROUP_IDENTIFIER: &str = "KernelChecks";
const NAME: &str = "Kernel Checks";

pub struct KernelChecks;

impl KernelChecks {
    pub fn run_all(&self) -> CheckGroupResult {
        let results = vec![self.has_modules(), self.version_is_good()];

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

    fn current_kernel_version(&self) -> Result<String> {
        let output = Command::new("uname")
            .arg("-r")
            .output()
            .expect("Failed to execute command");

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            bail!("{}", error_message.to_string());
        }

        let kernel_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(kernel_version)
    }

    fn version_is_good(&self) -> CheckResult {
        let name = String::from("Host Kernel Version Is Good");
        let mut result = Passed;

        // Get kernel version
        let current = kernel::Version::current();
        if let Err(e) = current {
            return CheckResult::new(&name, Errored(e.to_string()));
        }
        let current = current.unwrap();
        let lowest = kernel::Version::new(5, 15, 0);

        if current < lowest {
            result = Failed(String::from("current kernel version is unsupported"));
        }
        CheckResult::new(&name, result)
    }

    fn has_modules(&self) -> CheckResult {
        let name = String::from("Host Has Necessary Modules");
        let mut result = Passed;

        let mut required_modules = vec![
            "xen_evtchn",
            "xen-privcmd",
            "xen-netback",
            "xen-pciback",
            "xen-blkback",
            "xen-gntdev",
            "xen-gntalloc",
        ];

        // Get kernel version
        let kernel_version = self.current_kernel_version();
        if let Err(e) = kernel_version {
            return CheckResult::new(&name, Errored(e.to_string()));
        }
        let kernel_version = kernel_version.unwrap();

        // Search builtin modules
        let path = PathBuf::from(format!("/lib/modules/{kernel_version}/modules.builtin"));
        let builtins = fs::read_to_string(path);
        if let Err(e) = builtins {
            return CheckResult::new(&name, Errored(e.to_string()));
        }
        let builtins = builtins.unwrap();

        for builtin in builtins.lines() {
            let found = required_modules
                .iter()
                .position(|required| builtin.contains(required));

            if let Some(index) = found {
                debug!("builtin {}", required_modules[index]);
                required_modules.remove(index);
            }
        }

        // Search loaded modules
        let modules = procfs::KernelModules::current();
        if let Err(e) = modules {
            return CheckResult::new(&name, Errored(format!("getting kernel modules {e}")));
        }
        let modules = modules.unwrap();

        for (name, _) in modules.0.iter() {
            let found = required_modules
                .iter()
                .position(|required| required == name);

            if let Some(index) = found {
                debug!("module {}", required_modules[index]);
                required_modules.remove(index);
            }
        }
        if !required_modules.is_empty() {
            result = Failed(format!("missing {:?}", required_modules))
        }

        CheckResult::new(&name, result)
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
        self.run_all()
    }
}
