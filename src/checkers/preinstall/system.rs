use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    disk::available_space,
    host_executor::HostNamespaceExecutor,
};

use async_trait::async_trait;
use bytesize::ByteSize;
use futures::{FutureExt, future::join_all};
use std::path::Path;

const GROUP_IDENTIFIER: &str = "system";
const NAME: &str = "System Checks";
const MINIMUM_DISK_GENERAL: u64 = 1024 * 1024 * 1024; // 1GB
const MINIMUM_DISK_VAR: u64 = 5 * 1024 * 1024 * 1024; // 5GB
const MINIMUM_DISK_BOOT: u64 = 200 * 1024 * 1024; // 200MB

pub struct SystemChecks {
    host_executor: HostNamespaceExecutor,
    changes_boot: bool,
}

impl SystemChecks {
    /// `changes_boot` adds the checks for an install that changes what the host boots.
    pub fn new(host_executor: HostNamespaceExecutor, changes_boot: bool) -> Self {
        SystemChecks {
            host_executor,
            changes_boot,
        }
    }
    pub async fn run_all(&self) -> CheckGroupResult {
        let mut checks = vec![
            self.not_snap_docker().boxed(),
            self.enough_disk("/usr".into(), MINIMUM_DISK_GENERAL)
                .boxed(),
            self.enough_disk("/var/lib".into(), MINIMUM_DISK_VAR)
                .boxed(),
            self.has_nft_bin().boxed(),
            self.has_package_manager_bin().boxed(),
            self.has_service_manager_bin().boxed(),
            self.has_linux_util_bins(&["tar", "grep"]).boxed(),
        ];
        if self.changes_boot {
            checks.push(self.enough_disk("/boot".into(), MINIMUM_DISK_BOOT).boxed());
            checks.push(self.has_grub_mkconfig_bin().boxed());
        }
        let results = join_all(checks).await;

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

    async fn not_snap_docker(&self) -> CheckResult {
        let name = String::from("Supported Docker Runtime");
        let is_snap = std::fs::read_to_string("/proc/self/mountinfo")
            .map(|c| c.contains("snap/docker"))
            .unwrap_or(false);

        if is_snap {
            CheckResult::new(
                &name,
                Errored(
                    "Docker is running as a snap: hosts with snap-managed Docker runtimes are not supported installation targets".into(),
                ),
            )
        } else {
            CheckResult::new(&name, Passed)
        }
    }

    /// Checks that the `nft` binary, typically from the `nftables` package,
    /// is in PATH. Currently, the installer and `protect-network` rely on this
    /// in releases before 1.13. Releases 1.13 and after bundle it in `protect-tools`.
    async fn has_nft_bin(&self) -> CheckResult {
        let name = String::from("'nft' Binary Present");
        let found = self
            .host_executor
            .spawn_in_host_ns(async {
                std::process::Command::new("nft")
                    .arg("--version")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .status()
                    .is_ok()
            })
            .await
            .unwrap_or(false);

        if found {
            CheckResult::new(&name, Passed)
        } else {
            CheckResult::new(
                &name,
                Errored("'nft' binary is required but not present, install `nftables`".into()),
            )
        }
    }

    /// Checks that the given util-linux/coreutils/etc binaries are present in PATH.
    /// Only the launch has to succeed, because BusyBox `grep` rejects `--version`.
    /// On all supported systems this should be a given.
    /// Currently, the installer relies explicitly on these being present on the host.
    async fn has_linux_util_bins(&self, bins: &[&str]) -> CheckResult {
        let name = String::from("Basic Linux Utility Binaries Present");
        let owned_bins: Vec<String> = bins.iter().map(|s| s.to_string()).collect();

        let missing = self
            .host_executor
            .spawn_in_host_ns(async move {
                let mut missing = Vec::new();
                for bin in &owned_bins {
                    let found = std::process::Command::new(bin)
                        .arg("--version")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .is_ok();
                    if !found {
                        missing.push(bin.clone());
                    }
                }
                missing
            })
            .await
            .unwrap_or_default();

        if missing.is_empty() {
            CheckResult::new(&name, Passed)
        } else {
            let msg = format!(
                "required basic Linux utility binaries not found in PATH: {}",
                missing.join(", ")
            );
            CheckResult::new(&name, Errored(msg))
        }
    }

    /// Checks that either `grub-mkconfig` or `grub2-mkconfig` is present in PATH,
    /// as the installer requires one or the other to change what the host boots.
    async fn has_grub_mkconfig_bin(&self) -> CheckResult {
        let name = String::from("'grub-mkconfig' Binary Present");
        let found = self
            .host_executor
            .spawn_in_host_ns(async {
                ["grub-mkconfig", "grub2-mkconfig"].iter().any(|bin| {
                    std::process::Command::new(bin)
                        .arg("--version")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                })
            })
            .await
            .unwrap_or(false);

        if found {
            CheckResult::new(&name, Passed)
        } else {
            CheckResult::new(
                &name,
                Errored("neither 'grub-mkconfig' nor 'grub2-mkconfig' found in PATH".into()),
            )
        }
    }

    /// Checks that either `systemctl` or `rc-update` is present in PATH,
    /// as the installer requires one or the other to enable services.
    async fn has_service_manager_bin(&self) -> CheckResult {
        let name = String::from("Service Manager Binary Present");
        let found = self
            .host_executor
            .spawn_in_host_ns(async {
                ["systemctl", "rc-update"].iter().any(|bin| {
                    std::process::Command::new(bin)
                        .arg("--version")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status()
                        .map(|s| s.success())
                        .unwrap_or(false)
                })
            })
            .await
            .unwrap_or(false);

        if found {
            CheckResult::new(&name, Passed)
        } else {
            CheckResult::new(
                &name,
                Errored("neither 'systemctl' nor 'rc-update' found in PATH".into()),
            )
        }
    }

    /// Checks that at least one supported package manager is present in PATH,
    /// as the installer requires one to install system packages.
    async fn has_package_manager_bin(&self) -> CheckResult {
        let name = String::from("Package Manager Binary Present");
        let found = self
            .host_executor
            .spawn_in_host_ns(async {
                ["dnf", "yum", "zypper", "apt-get", "apk"]
                    .iter()
                    .any(|bin| {
                        std::process::Command::new(bin)
                            .arg("--version")
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null())
                            .status()
                            .map(|s| s.success())
                            .unwrap_or(false)
                    })
            })
            .await
            .unwrap_or(false);

        if found {
            CheckResult::new(&name, Passed)
        } else {
            CheckResult::new(
                &name,
                Errored("no supported package manager found in PATH (tried: dnf, yum, zypper, apt-get, apk)".into()),
            )
        }
    }

    async fn enough_disk(&self, mount_path: String, free_thresh: u64) -> CheckResult {
        let name = format!("Enough Disk Space for {}", &mount_path);
        let result = match self
            .host_executor
            .spawn_in_host_ns(async move {
                match available_space(Path::new(&mount_path)) {
                    Ok(avail) if avail >= free_thresh => Passed,
                    Ok(avail) => Failed(format!(
                        "{} has {} free, need at least {}",
                        mount_path,
                        ByteSize(avail),
                        ByteSize(free_thresh),
                    )),
                    Err(e) => Errored(format!(
                        "could not read free space for {}: {}",
                        mount_path, e
                    )),
                }
            })
            .await
        {
            Ok(result) => result,
            Err(e) => Errored(e.to_string()),
        };
        CheckResult::new(&name, result)
    }
}

#[async_trait]
impl CheckGroup for SystemChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "System requirement checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Required
    }
}
