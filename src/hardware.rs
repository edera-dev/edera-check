use super::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use std::path::{Path, PathBuf};
use std::process::Command;

const GROUP_IDENTIFIER: &str = "HardwareChecks";
const NAME: &str = "Hardware Checks";

pub struct HardwareChecks;

impl HardwareChecks {
    pub fn run_all(&self) -> CheckGroupResult {
        let results = vec![
            self.record_lspci(),
            self.record_dmidecode(),
            self.record_cpuinfo(),
            self.record_cmdline(),
            self.record_grub_cfg(),
        ];

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

    fn run_tool(&self, tool: &str) -> CheckResult {
        let name = format!("Record {tool}");

        let mut tool_args: Vec<&str> = tool.split(" ").collect();
        let cmd = tool_args.remove(0);

        let output = Command::new(cmd).args(tool_args).output();
        if let Err(e) = output {
            return CheckResult::new(&name, Errored(e.to_string()));
        }
        let output = output.unwrap();

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            return CheckResult::new(&name, Errored(error_message.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        CheckResult::new_with_output(&name, Passed, Some(stdout))
    }

    fn record_lspci(&self) -> CheckResult {
        self.run_tool("lspci -vvv")
    }

    fn record_dmidecode(&self) -> CheckResult {
        self.run_tool("dmidecode")
    }

    fn record_file(&self, file: &Path) -> CheckResult {
        let name = format!("Record {}", file.display());
        let output = std::fs::read_to_string(file);
        if let Err(e) = output {
            return CheckResult::new(
                &name,
                Errored(format!("failed to read {}: {e}", file.display())),
            );
        }
        let output = output.unwrap();
        CheckResult::new_with_output(&name, Passed, Some(output))
    }

    fn record_cpuinfo(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/cpuinfo").as_ref())
    }

    fn record_cmdline(&self) -> CheckResult {
        self.record_file(PathBuf::from("/proc/cmdline").as_ref())
    }

    fn record_grub_cfg(&self) -> CheckResult {
        let files = vec!["/boot/grub2/grub.cfg", "/boot/grub/grub.cfg"];
        for file in files.iter() {
            let file = PathBuf::from(file);
            if file.exists() {
                return self.record_file(&file);
            }
        }
        CheckResult::new(
            "Record grub config",
            Errored(format!("failed to find any {:?}", files)),
        )
    }
}

impl CheckGroup for HardwareChecks {
    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Hardware requirement checks - records for informational purposes"
    }

    fn run(&self) -> CheckGroupResult {
        self.run_all()
    }
}
