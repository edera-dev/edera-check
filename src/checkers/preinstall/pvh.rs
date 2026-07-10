use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
    cpu::{extract_cpu_vendor, extract_flags},
    host_executor::HostNamespaceExecutor,
};
use anyhow::{Result, bail};
use async_trait::async_trait;
use futures::{FutureExt, future::join_all};
use log::{debug, error, warn};
use std::{
    fs,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::Command,
};

const GROUP_IDENTIFIER: &str = "pvh";
const NAME: &str = "PVH Checks";

#[derive(Debug, PartialEq)]
enum VirtStatus {
    Enabled,      // Currently active
    CanBeEnabled, // Available but not active
    Disabled,     // Not available or BIOS disabled
}

pub struct PVHChecks {
    host_executor: HostNamespaceExecutor,
}

#[cfg(target_arch = "x86_64")]
impl PVHChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        PVHChecks { host_executor }
    }

    /// Run all the checkers asynchronously, then
    /// join and collect the results.
    pub async fn run_all(&self) -> CheckGroupResult {
        let results = join_all([self.check_virtualization().boxed()]).await;

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

    async fn ensure_msr_modprobe(&self) {
        let _ = self
            .host_executor
            .spawn_in_host_ns(async {
                // Load msr kernel module (ignore errors)
                Command::new("modprobe").arg("msr").output()
            })
            .await;
    }

    /// Checks that hardware virtualization (Intel VT-x or AMD-V) is available and not
    /// disabled in firmware. Reads the CPU vendor from `/proc/cpuinfo`, then inspects
    /// MSR registers to determine BIOS enablement state.
    ///
    /// Manual equivalent (Intel — reads IA32_FEATURE_CONTROL MSR 0x3a):
    /// ```sh
    /// modprobe msr
    /// grep -m1 '^flags' /proc/cpuinfo | grep -qw vmx && echo "vmx present"
    /// rdmsr 0x3a  # bit 0 = lock, bit 2 = vmx_outside_smx; both must be 1
    /// ```
    ///
    /// Manual equivalent (AMD — reads VM_CR MSR 0xC0010114):
    /// ```sh
    /// modprobe msr
    /// grep -m1 '^flags' /proc/cpuinfo | grep -qw svm && echo "svm present"
    /// rdmsr 0xC0010114  # bit 4 = SVMDIS; must be 0
    /// ```
    async fn check_virtualization(&self) -> CheckResult {
        let name = String::from("PVH Support");

        self.ensure_msr_modprobe().await;

        match self.discover_cpu_virtualization().await {
            Ok(VirtStatus::Enabled) | Ok(VirtStatus::CanBeEnabled) => {
                debug!("Hardware Virtualization is enabled or can be enabled");
                CheckResult::new(&name, Passed)
            }
            Ok(VirtStatus::Disabled) => {
                debug!("Hardware Virtualization disabled");
                CheckResult::new(
                    &name,
                    Failed(String::from("Hardware Virtualization Disabled")),
                )
            }
            Err(e) => {
                error!("Error: {}", e);
                CheckResult::new(&name, Errored(e.to_string()))
            }
        }
    }

    async fn discover_cpu_virtualization(&self) -> Result<VirtStatus> {
        let cpuinfo = self
            .host_executor
            .spawn_in_host_ns(async { fs::read_to_string("/proc/cpuinfo") })
            .await??;

        let cpu_vendor = extract_cpu_vendor(&cpuinfo);
        let flags = extract_flags(&cpuinfo);

        match cpu_vendor.as_str() {
            "GenuineIntel" => self.check_intel(&flags).await,
            "AuthenticAMD" => self.check_amd(&flags).await,
            _ => {
                error!("Unknown CPU vendor: {}", cpu_vendor);
                Ok(VirtStatus::Disabled)
            }
        }
    }

    async fn check_intel(&self, flags: &str) -> Result<VirtStatus> {
        let has_vmx = flags.split_whitespace().any(|f| f == "vmx");
        let cpuid = 0; // TODO(bml) check all CPUs
        let under_hypervisor = flags.split_whitespace().any(|f| f == "hypervisor");

        // Always try to read MSRs to report BIOS settings, even if CPU lacks support
        match self.read_msr(0x3a, cpuid).await {
            Ok(val) => {
                let lock = val & 1;
                let vmx_outside_smx = (val >> 2) & 1;
                let hw_supports = lock == 1 && vmx_outside_smx == 1;
                debug!(
                    "IA32_FEATURE_CONTROL=0x{:x} (lock={}, vmx_outside_smx={})",
                    val, lock, vmx_outside_smx
                );

                if !hw_supports {
                    debug!("Intel VT-x disabled in BIOS or not supported by hardware");
                    Ok(VirtStatus::Disabled)
                } else if has_vmx {
                    debug!("Intel VT-x supported and available (vmx flag present)");
                    Ok(VirtStatus::Enabled)
                } else if under_hypervisor {
                    debug!("Intel VT-x supported but unavailable under hypervisor");
                    Ok(VirtStatus::CanBeEnabled)
                } else {
                    debug!("Intel VT-x supported by system but not current CPU");
                    Ok(VirtStatus::Disabled)
                }
            }
            Err(e) => {
                debug!("error reading MSR registers: {e}");
                warn!("could not read MSR registers, falling back to cpuinfo detection");
                if !has_vmx {
                    debug!("CPU does not support Intel VT-x");
                    Ok(VirtStatus::Disabled)
                } else {
                    debug!("vmx flag present, assuming hardware supports it");
                    Ok(VirtStatus::CanBeEnabled)
                }
            }
        }
    }

    async fn check_amd(&self, flags: &str) -> Result<VirtStatus> {
        let has_svm = flags.split_whitespace().any(|f| f == "svm");
        let cpuid = 0; // TODO(bml) check all CPUs
        let under_hypervisor = flags.split_whitespace().any(|f| f == "hypervisor");

        // Always try to read MSRs to report BIOS settings, even if CPU lacks support
        match self.read_msr(0xC0010114, cpuid).await {
            Ok(vmcr) => {
                let svmdis = (vmcr >> 4) & 1;
                let hw_supports = svmdis == 0;
                debug!("VM_CR=0x{:x} (svmdis={})", vmcr, svmdis);

                if !hw_supports {
                    debug!("AMD-V disabled in BIOS or not supported by hardware");
                    Ok(VirtStatus::Disabled)
                } else {
                    // Hardware supports AMD-V - check current state
                    match self.read_msr(0xC0000080, cpuid).await {
                        Ok(efer) => {
                            let svme = (efer >> 12) & 1;
                            debug!("EFER=0x{:x} (SVME={})", efer, svme);
                            if has_svm {
                                if svme == 1 {
                                    debug!("AMD-V currently enabled and active");
                                    Ok(VirtStatus::Enabled)
                                } else {
                                    debug!("AMD-V supported and available (can be enabled)");
                                    Ok(VirtStatus::CanBeEnabled)
                                }
                            } else if under_hypervisor && (svme == 0 || svme == 1) {
                                debug!("AMD-V supported but unavailable under hypervisor");
                                // Hardware supports but flag absent - likely masked by hypervisor
                                Ok(VirtStatus::CanBeEnabled)
                            } else {
                                debug!("AMD-V supported by system but not current CPU");
                                Ok(VirtStatus::Disabled)
                            }
                        }
                        Err(e) => {
                            debug!("Cannot read EFER: {}", e);
                            if has_svm {
                                debug!("Hardware supports AMD-V, assuming it can be enabled");
                                Ok(VirtStatus::CanBeEnabled)
                            } else {
                                debug!("Cannot determine if AMD-V can be used");
                                Ok(VirtStatus::Disabled)
                            }
                        }
                    }
                }
            }
            Err(e) => {
                debug!("error reading MSR registers: {e}");
                warn!("could not read MSR registers, falling back to cpuinfo detection");
                if !has_svm {
                    debug!("CPU does not support AMD-V");
                    Ok(VirtStatus::Disabled)
                } else {
                    debug!("svm flag present, assuming hardware supports it");
                    Ok(VirtStatus::CanBeEnabled)
                }
            }
        }
    }

    async fn read_msr(&self, msr: u32, cpuid: u32) -> Result<u64> {
        let result = self
            .host_executor
            .spawn_in_host_ns(async move {
                let msr_path = format!("/dev/cpu/{}/msr", cpuid);

                // Check if MSR device exists
                if !Path::new(&msr_path).exists() {
                    bail!(format!(
                        "Failed to read MSR 0x{:x}: /dev/cpu/0/msr doesn't exist, load 'msr' kernel module",
                        msr
                    ));
                }

                // Open and read from the MSR device file
                let mut file = File::open(msr_path)?;
                file.seek(SeekFrom::Start(msr as u64))?;

                let mut buffer = [0u8; 8];
                file.read_exact(&mut buffer)?;

                // Convert little-endian bytes to u64
                Ok(u64::from_le_bytes(buffer))
            }).await??;

        Ok(result)
    }
}

// No-op for other archs
// TODO(bml) arm64 PVH??
#[cfg(not(target_arch = "x86_64"))]
impl PVHChecks {
    pub fn new(host_executor: HostNamespaceExecutor) -> Self {
        PVHChecks { host_executor }
    }

    pub async fn run_all(&self) -> CheckGroupResult {
        use crate::helpers::CheckResultValue::Skipped;
        CheckGroupResult {
            name: NAME.to_string(),
            result: Skipped("not supported on this arch".into()),
            results: vec![],
        }
    }
}

#[async_trait]
impl CheckGroup for PVHChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "PVH capability checks"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Optional("PVH feature may not be available on this system".into())
    }
}
