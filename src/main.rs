use preflight::{
    CheckGroup,
    CheckResultValue::{Errored, Failed, Passed},
    script::ScriptChecks,
    system::SystemChecks,
};

use anyhow::{Result, bail};
use log::info;
use std::env;

// Skip certain groups. List is separated by ;
fn skip_groups() -> Vec<String> {
    let skips = env::var("EDERA_PREFLIGHT_SKIP_GROUPS").unwrap_or_default();
    skips.split(";").map(|s| s.to_string()).collect()
}

fn main() -> Result<()> {
    env_logger::init();

    let groups: Vec<Box<dyn CheckGroup>> = vec![Box::new(SystemChecks), Box::new(ScriptChecks)];

    let mut final_result = Passed;
    let skip_groups = skip_groups();

    // Run each check group
    for group in groups {
        // Check if we need to explicity skip this group
        if skip_groups.iter().any(|skip| group.id() == skip) {
            continue;
        }

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
