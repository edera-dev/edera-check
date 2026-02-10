use crate::helpers::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use log::debug;
use sysinfo::{Disks, System};

const GROUP_IDENTIFIER: &str = "SystemChecks";
const NAME: &str = "System Checks";
const MINIMUM_MEMORY: u64 = 4 * 1024 * 1024 * 1024; // 4GB
const MINIMUM_DISK: u64 = 20 * 1024 * 1024 * 1024; // 20GB

pub struct SystemChecks;

impl SystemChecks {
    pub fn run_all(&self) -> CheckGroupResult {
        let results = vec![self.enough_memory(), self.enough_disk()];

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

    fn enough_disk(&self) -> CheckResult {
        let name = String::from("Enough Disk");
        let mut result = Failed(String::from("Not enough disk space on any disk"));
        let disks = Disks::new_with_refreshed_list();
        for disk in &disks {
            if disk.available_space() < MINIMUM_DISK {
                debug!(
                    "Not enough space on disk mounted at {} - {}",
                    disk.mount_point().display(),
                    disk.available_space()
                );
            } else {
                debug!(
                    "Enough space on disk mounted at {} - {}",
                    disk.mount_point().display(),
                    disk.available_space()
                );
                result = Passed;
            }
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
