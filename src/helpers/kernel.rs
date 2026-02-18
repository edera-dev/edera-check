use crate::helpers::host_executor::HostNamespaceExecutor;

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
