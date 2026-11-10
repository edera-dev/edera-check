use crate::helpers::{
    CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

use anyhow::{Result, bail};
use log::debug;
use procfs::{Current, sys::kernel};
use std::{fs, path::PathBuf, process::Command};

/// Get host kernel version
pub async fn host_kver_above_floor(
    host_executor: &HostNamespaceExecutor,
    floor: kernel::Version,
) -> Result<bool> {
    let current = host_executor
        .spawn_in_host_ns(async { kernel::Version::current() })
        .await
        .expect("error spawning in host")?;

    if current < floor {
        return Ok(false);
    }

    Ok(true)
}

/// The kernel treats `-` and `_` in a module name as the same character, and `/proc/modules`
/// always prints `_`, while module files are often named with `-`.
fn normalize_module_name(name: &str) -> String {
    name.replace('-', "_")
}

/// Module name for one `modules.builtin` or `modules.dep` line, such as
/// `kernel/drivers/xen/xen-evtchn.ko.xz: kernel/...`.
fn module_name_from_line(line: &str) -> String {
    let path = line.split(':').next().unwrap_or("");
    let file = path.rsplit('/').next().unwrap_or("");
    let stem = file.split(".ko").next().unwrap_or("");
    normalize_module_name(stem)
}

/// Looks at builtins for kernel_version and compares that to the list of
/// required modules.
/// Returns a vec of everything from required_modules that WAS NOT found in builtins.
pub async fn find_builtins(
    host_executor: &HostNamespaceExecutor,
    required_modules: &[String],
) -> Result<Vec<String>> {
    let mut modules_to_find: Vec<String> = required_modules.to_owned();

    // read host builtins
    let builtins = host_executor
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
        let builtin = module_name_from_line(builtin);
        let found = modules_to_find
            .iter()
            .position(|required| normalize_module_name(required) == builtin);

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
pub async fn find_loaded(
    host_executor: &HostNamespaceExecutor,
    required_modules: &[String],
) -> Result<Vec<String>> {
    let mut modules_to_find: Vec<String> = required_modules.to_owned();

    let modules = host_executor
        .spawn_in_host_ns(async move { procfs::KernelModules::current() })
        .await?;

    let modules = modules.unwrap();

    for (name, _) in modules.0.iter() {
        let found = modules_to_find
            .iter()
            .position(|required| normalize_module_name(required) == *name);

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
pub async fn find_loadable(
    host_executor: &HostNamespaceExecutor,
    required_modules: &[String],
) -> Result<Vec<String>> {
    let mut modules_to_find: Vec<String> = required_modules.to_owned();
    let dep_file = host_executor
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
        let available = module_name_from_line(line);
        let found = modules_to_find
            .iter()
            .position(|required| normalize_module_name(required) == available);
        if let Some(index) = found {
            debug!("available {}", modules_to_find[index]);
            modules_to_find.remove(index);
        }
    }
    Ok(modules_to_find)
}

/// Try to check if a group of modules exist on a system through checking builtins,
/// loaded, then loadable modules. This function is gonna be called by a check identified
/// by parent_check_name
pub async fn check_modules(
    parent_check_name: String,
    host_executor: &HostNamespaceExecutor,
    required_modules: &[String],
) -> CheckResult {
    // Search builtin modules
    let remaining = match find_builtins(host_executor, required_modules).await {
        Ok(r) => r,
        Err(e) => {
            return CheckResult::new(
                &parent_check_name,
                Errored(format!("getting kernel builtins {e}")),
            );
        }
    };

    // Search loaded modules
    let remaining = match find_loaded(host_executor, &remaining).await {
        Ok(r) => r,
        Err(e) => {
            return CheckResult::new(
                &parent_check_name,
                Errored(format!("getting kernel modules {e}")),
            );
        }
    };

    // another namespace yet those checks should run in the host namespace
    // Search loadable modules (present in modules.dep but not yet loaded)
    let remaining = match find_loadable(host_executor, &remaining).await {
        Ok(r) => r,
        Err(e) => {
            return CheckResult::new(
                &parent_check_name,
                Errored(format!("getting kernel modules {e}")),
            );
        }
    };
    if !remaining.is_empty() {
        return CheckResult::new(
            &parent_check_name,
            Failed(format!("missing {:?}", remaining)),
        );
    }

    CheckResult::new(&parent_check_name, Passed)
}

#[cfg(test)]
mod tests {
    use super::{module_name_from_line, normalize_module_name};

    #[test]
    fn module_file_names_match_required_names_across_separators() {
        let dep = "kernel/drivers/xen/xen-evtchn.ko.xz: kernel/drivers/xen/xen-privcmd.ko.xz";
        assert_eq!(module_name_from_line(dep), "xen_evtchn");
        assert_eq!(
            module_name_from_line("kernel/arch/x86/kvm/kvm-intel.ko.zst:"),
            normalize_module_name("kvm_intel")
        );
        assert_eq!(
            module_name_from_line("kernel/drivers/xen/xen-privcmd.ko"),
            normalize_module_name("xen-privcmd")
        );
    }

    #[test]
    fn a_module_does_not_match_another_that_contains_its_name() {
        for line in [
            "kernel/drivers/scsi/arcmsr/arcmsr.ko.xz:",
            "kernel/drivers/powercap/intel_rapl_msr.ko.xz: kernel/arch/x86/kernel/msr.ko.xz",
        ] {
            assert_ne!(module_name_from_line(line), "msr");
        }
        assert_eq!(
            module_name_from_line("kernel/arch/x86/kernel/msr.ko"),
            "msr"
        );
    }
}
