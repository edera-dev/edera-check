use preflight::{
    CheckGroup,
    CheckResultValue::{Errored, Failed, Passed},
    script::ScriptChecks,
    system::SystemChecks,
};

use anyhow::{Result, bail};
use log::info;
use std::env;

fn main() -> Result<()> {
    env_logger::init();

    let groups: Vec<Box<dyn CheckGroup>> = vec![Box::new(SystemChecks), Box::new(ScriptChecks)];

    let mut final_result = Passed;

    // Run each check group
    for group in groups {
        // if group.id() == "ScriptedChecks" {
        //     continue;
        // }
        info!("Running Group [{}] - {}", group.name(), group.description());
        let check_group_result = group.run();
        check_group_result.log_group();
        if env::var("EDERA_PREFLIGHT_VERBOSE").unwrap_or_default() == "true" {
            check_group_result.log_individual_checks();
        }

        // Set final result to Failed if we failed and aren't already in an Errored state
        if !matches!(final_result, Errored(_)) && matches!(check_group_result.result, Failed(_)) {
            final_result = Failed(String::from("group failed"));
        }

        if matches!(check_group_result.result, Errored(_)) {
            final_result = Errored(String::from("group errored"));
        }
    }

    match final_result {
        Errored(_) | Failed(_) => bail!("checks failed"),
        _ => Ok(()),
    }
}
