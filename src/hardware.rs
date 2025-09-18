use super::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use log::debug;
use std::process::Command;

const GROUP_IDENTIFIER: &str = "HardwareChecks";
const NAME: &str = "Hardware Checks";

pub struct HardwareChecks;

impl HardwareChecks {
    pub fn run_all(&self) -> CheckGroupResult {
        let results = vec![self.record_lspci(), self.record_dmidecode()];

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

        let output = Command::new(tool).output();
        if let Err(e) = output {
            return CheckResult::new(&name, Errored(e.to_string()));
        }
        let output = output.unwrap();

        if !output.status.success() {
            let error_message = String::from_utf8_lossy(&output.stderr);
            return CheckResult::new(&name, Errored(error_message.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        for line in stdout.lines() {
            debug!("{}", line);
        }

        CheckResult::new(&name, Passed)
    }

    fn record_lspci(&self) -> CheckResult {
        self.run_tool("lspci")
    }

    fn record_dmidecode(&self) -> CheckResult {
        self.run_tool("dmidecode")
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
