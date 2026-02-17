use anyhow::{Result, bail};
use async_trait::async_trait;
use flate2::read::GzDecoder;
use futures::{FutureExt, future::join_all};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

use crate::helpers::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    host_executor::HostNamespaceExecutor,
};

const GROUP_IDENTIFIER: &str = "sysinfo";
const NAME: &str = "System Info Recorder";

pub struct SystemRecorder {
    host_executor: HostNamespaceExecutor,
}

impl SystemRecorder {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        SystemRecorder { host_executor }
    }

    /// Run all the recorders asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([
            self.record_lspci().boxed(),
            self.record_dmidecode().boxed(),
            self.record_cpuinfo().boxed(),
            self.record_cmdline().boxed(),
            self.record_grub_cfg().boxed(),
            self.record_kernel_cfg().boxed(),
        ])
        .await;

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

    /// Runs the given command + args in host namespaces and captures the results.
    async fn run_tool(&self, tool: &str) -> CheckResult {
        let name = format!("Captured {tool}");
        let mut tool_args: Vec<String> = tool.split(" ").map(|s| s.to_string()).collect();
        let cmd = tool_args.remove(0);

        let output = match self
            .host_executor
            .spawn_in_host_ns(async move { Command::new(cmd).args(tool_args).output() })
            .await
        {
            Ok(output) => output,
            Err(e) => return CheckResult::new(&name, Errored(e.to_string())),
        };

        let output = match output {
            Ok(output) => output,
            Err(e) => return CheckResult::new(&name, Errored(e.to_string())),
        };

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            return CheckResult::new(&name, Errored(error_message.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        CheckResult::new_with_output(&name, Passed, Some(stdout))
    }

    /// Captures the content of a given file on the host.
    async fn record_file(&self, file: &Path) -> Option<CheckResult> {
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

    async fn record_lspci(&self) -> CheckResult {
        self.run_tool("lspci -vvv").await
    }

    async fn record_dmidecode(&self) -> CheckResult {
        self.run_tool("dmidecode").await
    }

    async fn record_cpuinfo(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/cpuinfo").as_ref())
            .await
            .expect("/proc/cpuinfo not found")
    }

    async fn record_cmdline(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/cmdline").as_ref())
            .await
            .expect("/proc/cmdline not found")
    }

    async fn record_grub_cfg(&self) -> CheckResult {
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
            Errored(format!("failed to find any {:?}", files)),
        )
    }

    async fn record_kernel_cfg(&self) -> CheckResult {
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
        CheckResult::new(name, Errored(format!("failed to find any {:?}", files)))
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
}
