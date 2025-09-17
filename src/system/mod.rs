use super::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use log::debug;
use sysinfo::System;

const GROUP_IDENTIFIER: &str = "SystemChecks";
const NAME: &str = "System Checks";
const MINIMUM_MEMORY: u64 = 10000;

pub struct SystemChecks;

impl SystemChecks {
    pub fn run_all(&self) -> CheckGroupResult {
        let results = vec![self.enough_memory()];

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

    fn enough_memory(&self) -> CheckResult {
        let name = String::from("Enough Memory");
        let mut sys = System::new_all();
        sys.refresh_all();

        let total_mem = sys.total_memory();
        debug!("total memory = {total_mem}");

        let mut result = Passed;
        if total_mem < MINIMUM_MEMORY {
            let reason = format!("total memory is less than {}", MINIMUM_MEMORY);
            result = Failed(reason);
        }
        CheckResult::new(&name, result)
    }
}

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

    fn run(&self) -> CheckGroupResult {
        self.run_all()
    }
}
