use anyhow::{Result, bail};
use std::process::Command;

use crate::helpers::host_executor::HostNamespaceExecutor;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InitSystem {
    Systemd,
    OpenRC,
    Unknown,
}

pub async fn detect_init_system(host_executor: &HostNamespaceExecutor) -> InitSystem {
    host_executor
        .spawn_in_host_ns(async move {
            // Check if systemd is PID 1
            if let Ok(output) = Command::new("ps").args(["-p", "1", "-o", "comm="]).output() {
                let comm = String::from_utf8_lossy(&output.stdout);
                if comm.trim() == "systemd" {
                    return InitSystem::Systemd;
                }
            }

            // otherwise, check if rc-service exists (OpenRC)
            if Command::new("which")
                .arg("rc-service")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return InitSystem::OpenRC;
            }
            InitSystem::Unknown
        })
        .await
        .unwrap_or(InitSystem::Unknown)
}

pub async fn is_running(
    host_executor: &HostNamespaceExecutor,
    service: String,
    init: InitSystem,
) -> Result<bool> {
    host_executor
        .spawn_in_host_ns(async move {
            match init {
                InitSystem::Systemd => {
                    let status = Command::new("systemctl")
                        .args(["is-active", "--quiet", &service])
                        .status()?;
                    Ok(status.success())
                }
                InitSystem::OpenRC => {
                    let status = Command::new("rc-service")
                        .args([&service, "status"])
                        .status()?;
                    Ok(status.success())
                }
                InitSystem::Unknown => bail!("unknown init system"),
            }
        })
        .await?
}
