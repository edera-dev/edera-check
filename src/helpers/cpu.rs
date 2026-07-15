use std::fs;

use anyhow::Result;

use crate::helpers::host_executor::HostNamespaceExecutor;

pub async fn read_cpuinfo(host_executor: &HostNamespaceExecutor) -> Result<String> {
    let cpuinfo = host_executor
        .spawn_in_host_ns(async { fs::read_to_string("/proc/cpuinfo") })
        .await??;
    Ok(cpuinfo)
}

pub fn extract_cpu_vendor(cpuinfo: &str) -> String {
    for line in cpuinfo.lines() {
        if line.starts_with("vendor_id")
            && let Some(value) = line.split(':').nth(1)
        {
            return value.trim().to_string();
        }
    }
    String::from("Unknown")
}

pub fn extract_flags(cpuinfo: &str) -> String {
    for line in cpuinfo.lines() {
        if line.starts_with("flags")
            && let Some(value) = line.split(':').nth(1)
        {
            return value.trim().to_string();
        }
    }
    String::new()
}
