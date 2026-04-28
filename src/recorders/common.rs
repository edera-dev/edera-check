use anyhow::{Result, bail};
use flate2::read::GzDecoder;
use procfs::Current;
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use crate::helpers::{
    CheckResult,
    CheckResultValue::{Errored, Passed, Skipped},
    host_executor::HostNamespaceExecutor,
};

pub struct CommonSystemRecorder {
    host_executor: HostNamespaceExecutor,
}

impl CommonSystemRecorder {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        CommonSystemRecorder { host_executor }
    }

    /// Runs the given command + args in host namespaces and captures the results.
    pub async fn run_tool(&self, tool: &str) -> CheckResult {
        let name = format!("Captured {tool}");
        let mut tool_args: Vec<String> = tool.split(" ").map(|s| s.to_string()).collect();
        let cmd = tool_args.remove(0);

        let output = match self
            .host_executor
            .spawn_in_host_ns(async move { Command::new(cmd).args(tool_args).output() })
            .await
        {
            Ok(output) => output,
            Err(e) => return CheckResult::new(&name, Skipped(e.to_string())),
        };

        let output = match output {
            Ok(output) => output,
            Err(e) => return CheckResult::new(&name, Skipped(e.to_string())),
        };

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            return CheckResult::new(&name, Skipped(error_message.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        CheckResult::new_with_output(&name, Passed, Some(stdout))
    }

    /// Captures the content of a given file on the host.
    pub async fn record_file(&self, file: &Path) -> Option<CheckResult> {
        let local_file = file.to_path_buf();

        self.host_executor
            .spawn_in_host_ns(async move {
                if !local_file.exists() {
                    return None;
                }
                let name = format!("Captured {}", local_file.display());

                let bytes = match tokio::fs::read(&local_file).await {
                    Ok(b) => b,
                    Err(e) => {
                        return Some(CheckResult::new(
                            &name,
                            Errored(format!("failed to read {}: {e}", local_file.display())),
                        ));
                    }
                };

                let content = if bytes.starts_with(&[0x1f, 0x8b]) {
                    // Gzip magic bytes detected
                    let mut decoder = GzDecoder::new(&bytes[..]);
                    let mut s = String::new();
                    decoder.read_to_string(&mut s).map(|_| s)
                } else {
                    String::from_utf8(bytes)
                        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
                };

                match content {
                    Ok(c) => Some(CheckResult::new_with_output(&name, Passed, Some(c))),
                    Err(e) => Some(CheckResult::new(
                        &name,
                        Errored(format!("failed to decode {}: {e}", local_file.display())),
                    )),
                }
            })
            .await
            .unwrap_or_else(|_| panic!("could not record {}", file.display()))
    }

    /// Records verbose PCI device listing.
    ///
    /// Manual equivalent:
    /// ```sh
    /// lspci -vvv
    /// ```
    pub async fn record_lspci(&self) -> CheckResult {
        self.run_tool("lspci -vvv").await
    }

    /// Records DMI/SMBIOS hardware information (BIOS, board, chassis, CPU, memory).
    ///
    /// Manual equivalent:
    /// ```sh
    /// dmidecode
    /// ```
    pub async fn record_dmidecode(&self) -> CheckResult {
        const NAME: &str = "Captured dmidecode";

        self.host_executor
            .spawn_in_host_ns(async {
                let ep_bytes = match std::fs::read("/sys/firmware/dmi/tables/smbios_entry_point") {
                    Ok(b) => b,
                    Err(e) => {
                        return CheckResult::new(
                            NAME,
                            Skipped(format!("could not read smbios_entry_point: {e}")),
                        );
                    }
                };

                let entry_point = match dmidecode::EntryPoint::search(&ep_bytes) {
                    Ok(ep) => ep,
                    Err(e) => {
                        return CheckResult::new(
                            NAME,
                            Skipped(format!("could not parse DMI entry point: {e:?}")),
                        );
                    }
                };

                let table_bytes = match std::fs::read("/sys/firmware/dmi/tables/DMI") {
                    Ok(b) => b,
                    Err(e) => {
                        return CheckResult::new(
                            NAME,
                            Skipped(format!("could not read DMI table: {e}")),
                        );
                    }
                };

                let mut output = String::new();
                for structure in entry_point.structures(&table_bytes) {
                    match structure {
                        Ok(s) => output.push_str(&format!("{s:#?}\n")),
                        Err(e) => output.push_str(&format!("Error: {e:?}\n")),
                    }
                }

                CheckResult::new_with_output(NAME, Passed, Some(output.trim().to_string()))
            })
            .await
            .unwrap_or_else(|e| CheckResult::new(NAME, Skipped(e.to_string())))
    }

    /// Records CPU hardware details and feature flags.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/cpuinfo
    /// ```
    pub async fn record_cpuinfo(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/cpuinfo").as_ref())
            .await
            .expect("/proc/cpuinfo not found")
    }

    /// Records the kernel command line used to boot the running kernel.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/cmdline
    /// ```
    pub async fn record_cmdline(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/cmdline").as_ref())
            .await
            .expect("/proc/cmdline not found")
    }

    /// Records the GRUB bootloader configuration. Checks `/boot/grub2/grub.cfg` first,
    /// falling back to `/boot/grub/grub.cfg`.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /boot/grub2/grub.cfg || cat /boot/grub/grub.cfg
    /// ```
    pub async fn record_grub_cfg(&self) -> CheckResult {
        // prefer grub2 path, since if both are present for any reason,
        // that is likely to be the "correct" one.
        let files = ["/boot/grub2/grub.cfg", "/boot/grub/grub.cfg"];

        for file in files {
            if let Some(result) = self.record_file(&PathBuf::from(file)).await {
                return result;
            }
        }
        CheckResult::new(
            "Record grub config",
            Skipped(format!("no grub config found in {:?}", files)),
        )
    }

    /// Records the kernel build configuration. Checks `/proc/config.gz` first (decompressing
    /// if needed), falling back to `/boot/config-$(uname -r)`.
    ///
    /// Manual equivalent:
    /// ```sh
    /// zcat /proc/config.gz || cat /boot/config-$(uname -r)
    /// ```
    pub async fn record_kernel_cfg(&self) -> CheckResult {
        let name = "Record kernel config";
        // Get kernel version
        //
        let Ok(kver) = self.current_kernel_version().await else {
            return CheckResult::new(name, Errored("failed to find kernel version".to_string()));
        };

        let files = ["/proc/config.gz", &format!("boot/config-{kver}")];

        for file in files {
            if let Some(result) = self.record_file(&PathBuf::from(file)).await {
                return result;
            }
        }
        CheckResult::new(
            name,
            Skipped(format!("no kernel config found in {:?}", files)),
        )
    }

    /// Records the list of currently loaded kernel modules.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cut -d' ' -f1 /proc/modules
    /// ```
    pub async fn record_loaded_modules(&self) -> CheckResult {
        let name = "Record current host kernel loaded modules";
        match self
            .host_executor
            .spawn_in_host_ns(async move { procfs::KernelModules::current() })
            .await
        {
            Ok(Ok(list)) => CheckResult::new_with_output(
                name,
                Passed,
                Some(list.0.into_keys().collect::<Vec<_>>().join("\n")),
            ),
            Ok(Err(e)) => CheckResult::new(name, Errored(e.to_string())),
            Err(e) => CheckResult::new(name, Skipped(e.to_string())),
        }
    }

    /// Records system memory usage statistics.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/meminfo
    /// ```
    pub async fn record_meminfo(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/meminfo").as_ref())
            .await
            .expect("/proc/meminfo not found")
    }

    /// Records virtual memory statistics.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/vmstat
    /// ```
    pub async fn record_vmstat(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/vmstat").as_ref())
            .await
            .expect("/proc/vmstat not found")
    }

    /// Records kernel slab allocator statistics.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/slabinfo
    /// ```
    pub async fn record_slabinfo(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/slabinfo").as_ref())
            .await
            .expect("/proc/slabinfo not found")
    }

    /// Records currently mounted filesystems.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/mounts
    /// ```
    pub async fn record_mounts(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/mounts").as_ref())
            .await
            .expect("/proc/mounts not found")
    }

    /// Records detailed mount point information for the current process.
    ///
    /// Manual equivalent:
    /// ```sh
    /// cat /proc/self/mountinfo
    /// ```
    pub async fn record_mountinfo(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/self/mountinfo").as_ref())
            .await
            .expect("/proc/self/mountinfo not found")
    }

    async fn current_kernel_version(&self) -> Result<String> {
        self.host_executor
            .spawn_in_host_ns(async {
                let output = Command::new("uname")
                    .arg("-r")
                    .output()
                    .expect("Failed to execute command");

                if !output.status.success() {
                    let error_message = String::from_utf8_lossy(&output.stderr);
                    bail!("{}", error_message);
                }

                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            })
            .await?
    }
}
