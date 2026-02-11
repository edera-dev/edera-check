use crate::helpers::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use anyhow::Result;
use async_trait::async_trait;
use log::{debug, warn};
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

const GROUP_IDENTIFIER: &str = "ScriptedChecks";
const NAME: &str = "Scripted Checks";

// TODO(found-it): encoding this here for the time being. Need a better way to handle builtin
// scripts, but would like to get customer feedback first.
const PVH_SCRIPT: &str = r#"#!/bin/sh
set -eu

echo "EDERA_PREFLIGHT_CHECK_NAME=Can PVH Be Enabled"

# prerequisites: msr-tools (rdmsr), and the msr kernel module
modprobe msr 2>/dev/null || true

err() {
  echo "$@" >&2
}

do_rdmsr() {
  MSR="$1"
  if [ ! -e /dev/cpu/0/msr ]; then
    err "rdmsr ${MSR}: /dev/cpu/0/msr doesn't exist, load 'msr' kernel module"
    return 1
  fi

  if command -v rdmsr >/dev/null; then
    rdmsr -p0 -d "$MSR" 2>/dev/null || echo 0
  elif command -v dd >/dev/null && command -v od >/dev/null; then
    (dd if=/dev/cpu/0/msr bs=1 skip=$(($MSR)) count=8 status=none 2>/dev/null | od -An -tu8 -N8) || echo 0
  else
    err "rdmsr ${MSR}: need either 'rdmsr' or 'dd' and 'od' commands and none were found"
    return 1
  fi
}

cpu_vendor=$(awk -F: '/vendor_id/{print $2; exit}' /proc/cpuinfo | xargs)
flags=$(awk -F: '/^flags/{print $2; exit}' /proc/cpuinfo)

case "$cpu_vendor" in
"GenuineIntel")
  echo "$flags" | grep -qw vmx && cap=yes || cap=no

  if do_rdmsr "0x3a" >/dev/null; then
    val=$(do_rdmsr 0x3a 2>/dev/null || echo 0)
    lock=$(((val >> 0) & 1))
    vmx_outside_smx=$(((val >> 2) & 1))
    bios_enabled=no
    if [ $lock -eq 1 ] && [ $vmx_outside_smx -eq 1 ]; then
      bios_enabled=yes
    fi
    printf "Intel VT-x capability: %s\n" "$cap"
    printf "IA32_FEATURE_CONTROL (0x3A): 0x%x (lock=%d, vmx_outside_smx=%d)\n" "$val" "$lock" "$vmx_outside_smx"
    printf "BIOS permits VT-x: %s\n" "$bios_enabled"
  else
    printf "Intel VT-x capability: %s (install msr-tools to verify MSR 0x3A)\n" "$cap"
  fi
  ;;

"AuthenticAMD")
  echo "$flags" | grep -qw svm && cap=yes || cap=no
  if do_rdmsr "0xC0010114" >/dev/null; then
    vmcr=$(do_rdmsr 0xC0010114 2>/dev/null || echo 0)
    lock=$(((vmcr >> 3) & 1))   # VM_CR.LOCK
    svmdis=$(((vmcr >> 4) & 1)) # VM_CR.SVMDIS (1 = disabled by BIOS/firmware)
    bios_enabled=$([ $svmdis -eq 0 ] && echo yes || echo no)

    efer=$(do_rdmsr 0xC0000080 2>/dev/null || echo 0)
    svme=$(((efer >> 12) & 1)) # EFER.SVME (runtime: 1 if OS/hypervisor enabled SVM)

    printf "AMD SVM capability: %s\n" "$cap"
    printf "VM_CR (0xC0010114): 0x%x (lock=%d, svmdis=%d)\n" "$vmcr" "$lock" "$svmdis"
    printf "BIOS permits SVM: %s\n" "$bios_enabled"
    printf "EFER (0xC0000080): 0x%x (SVME=%d)\n" "$efer" "$svme"
  else
    printf "AMD SVM capability: %s (install msr-tools to verify VM_CR/EFER)\n" "$cap"
  fi
  ;;

*)
  echo "Unknown CPU vendor: $cpu_vendor"
  ;;
esac

# vim: set ts=2 sts=2 sw=2 et:
"#;

/// ScriptChecks is a special type of check that is intended to run a series of
/// small shell scripts. The intent here is to make a pluggable interface for to
/// quickly implement checks. Ideally all checks end up in their own CheckGroup.
pub struct ScriptChecks;

impl ScriptChecks {
    /// run_all runs all shell scripts within the directory $EDERA_PREFLIGHT_SCRIPTS_DIR
    pub fn run_all(&self) -> CheckGroupResult {
        let mut results = Vec::new();

        let script_list = self.script_list();
        if let Err(e) = script_list {
            return CheckGroupResult {
                name: NAME.to_string(),
                result: Errored(format!("failed to initialize group: {e}")),
                results: vec![],
            };
        }
        let mut script_list = script_list.unwrap();

        // TODO(found-it): Handle builtin scripts better. This is a temporary workaround while we
        // get folks to test. If they want to copy the binary out of the container then everything
        // will be self contained.
        let script_path = env::temp_dir().join("edera_preflight_pvh.sh");
        let r = fs::write(&script_path, PVH_SCRIPT);
        if r.is_ok()
            && let Ok(metadata) = fs::metadata(&script_path)
        {
            let mut permissions = metadata.permissions();

            permissions.set_mode(0o700);
            let _ = fs::set_permissions(&script_path, permissions);
            script_list.push(script_path);
        }

        let mut group_result = Passed;

        for path in script_list {
            let res = self.run_script(&path);

            // Set group result to Failed if we failed and aren't already in an Errored state
            if !matches!(group_result, Errored(_)) && matches!(res.result, Failed(_)) {
                group_result = Failed(String::from("group failed"));
            }

            if matches!(res.result, Errored(_)) {
                group_result = Errored(String::from("group errored"));
            }

            results.push(res);
        }

        CheckGroupResult {
            name: NAME.to_string(),
            result: group_result,
            results,
        }
    }

    /// scripts_dir sets the directory to search for scripts
    fn scripts_dir(&self) -> PathBuf {
        let sdir = env::var("EDERA_PREFLIGHT_SCRIPTS_DIR").unwrap_or("./scripts".to_string());
        PathBuf::from(sdir)
    }

    /// script_list attempts to collect a list of shell scripts to execute. It
    /// will return an empty list if the path does not exist or is not a
    /// directory. If the path is a directory it will scrape all files (without
    /// recursing into subdirectories) and return them as a list.
    fn script_list(&self) -> Result<Vec<PathBuf>> {
        let scripts_dir = self.scripts_dir();

        let meta = fs::metadata(&scripts_dir);
        if meta.is_err() {
            return Ok(Vec::new());
        }

        let meta = meta.unwrap();

        if !meta.is_dir() {
            return Ok(Vec::new());
        }

        let mut scripts = Vec::new();
        for entry in fs::read_dir(&scripts_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                warn!(
                    "skipping load of {}: not a file",
                    entry.file_name().to_str().unwrap_or("unknown")
                );
                continue;
            }
            if entry.file_name() == "README.md" {
                continue;
            }

            scripts.push(entry.path());
        }

        Ok(scripts)
    }

    /// run_script will run an individual script and return the result. It will
    /// attempt to scrape some details from the script output (like a name for
    /// the check).
    ///
    /// If there is an error running the script (eg: script is not executable) then
    /// the check will return an error result.
    ///
    /// If the script runs successfully but exits with a non-zero exit code, then
    /// the check will reutrn a fail result.
    fn run_script(&self, path: &PathBuf) -> CheckResult {
        let mut name = path.to_str().unwrap_or_default().to_string();

        let output = Command::new(path).output();

        if let Err(e) = output {
            return CheckResult::new(&name, Errored(e.to_string()));
        }

        // We checked if this is an error and handled that case above
        let output = output.unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut check_name = None;

        for line in stdout.lines() {
            if let Some(value) = line.strip_prefix("EDERA_PREFLIGHT_CHECK_NAME=") {
                check_name = Some(value.to_string());
                continue;
            }
            debug!("{}", line);
        }

        if let Some(set_name) = check_name {
            name = set_name;
        }

        let result = match output.status.success() {
            true => Passed,
            false => Failed(format!(
                "script returned {:?}: {}",
                output.status.code(),
                stderr
            )),
        };
        CheckResult::new_with_output(&name, result, Some(stdout.to_string()))
    }
}

#[async_trait]
impl CheckGroup for ScriptChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Checks composed through small shell scripts"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all()
    }
}
