use log::{error, info, warn};
use std::fmt;

/// CheckResultValue is the final value for the result of an individual check.
///
/// Failed means a check ran successfully but did not pass. Errored means a check hit an
/// error while executing.
///
/// Failed and Errored should contain a descriptive string explaining the result.
pub enum CheckResultValue {
    Passed,
    Failed(String),
    Errored(String),
    Unknown,
}

impl fmt::Display for CheckResultValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CheckResultValue::Passed => write!(f, "Passed"),
            CheckResultValue::Failed(msg) => write!(f, "Failed: {}", msg),
            CheckResultValue::Errored(msg) => write!(f, "Errored: {}", msg),
            CheckResultValue::Unknown => write!(f, "Unknown"),
        }
    }
}

/// CheckResult is the end result of an individual check. It carries the name of the individual
/// check as well as the end result.
pub struct CheckResult {
    /// name is the name of the individual check
    pub name: String,

    /// result is the final result of an individual check
    pub result: CheckResultValue,

    /// output_to_record is an optional field used to return output that should be recorded into an
    /// information bundle
    pub output_to_record: Option<String>,
}

impl CheckResult {
    pub fn new(name: &str, result: CheckResultValue) -> Self {
        Self::new_with_output(name, result, None)
    }

    pub fn new_with_output(name: &str, result: CheckResultValue, output_to_record: Option<String>) -> Self {
        Self {
            name: name.to_string(),
            result,
            output_to_record,
        }
    }
}

/// CheckGroupResult is the result for a top-level group of checks. The result field is calculated
/// from the set of individual checks within that group.
pub struct CheckGroupResult {
    /// name is the name of the group of checks
    pub name: String,

    /// result is the top-level result of the group of checks. It is calculated from the results of
    /// each individual check.
    pub result: CheckResultValue,

    /// results is the list of results from each individual check within this group.
    pub results: Vec<CheckResult>,
}

impl CheckGroupResult {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            result: CheckResultValue::Unknown,
            results: Vec::new(),
        }
    }

    /// log_group is a pretty-print helper to log the result of the group based on what the result
    /// value is.
    pub fn log_group(&self) {
        let name = &self.name;
        let result = &self.result;
        let s = format!("[{}] {}", name, result);
        match result {
            CheckResultValue::Passed => info!("{}", s),
            CheckResultValue::Failed(_) => warn!("{}", s),
            CheckResultValue::Errored(_) => error!("{}", s),
            CheckResultValue::Unknown => warn!("{}", s),
        }
    }

    /// log_individual_checks is a pretty-print helper to log the results of each individual check
    /// within a group.
    pub fn log_individual_checks(&self) {
        let group_name = &self.name;
        for check_result in self.results.iter() {
            let name = &check_result.name;
            let result = &check_result.result;
            let s = format!("[{}] {}: {}", group_name, name, result);
            match result {
                CheckResultValue::Passed => info!("{}", s),
                CheckResultValue::Failed(_) => warn!("{}", s),
                CheckResultValue::Errored(_) => error!("{}", s),
                CheckResultValue::Unknown => warn!("{}", s),
            }
        }
    }
}

/// CheckGroup is a trait representing a group of checks.
pub trait CheckGroup {
    /// name is the name of the check group
    fn name(&self) -> &str;

    /// id is the identifier for the check group. This field is used when skipping or selecting
    /// certain check groups to run so it should be env/cli friendly.
    fn id(&self) -> &str;

    /// description is a longer form text field explaining what this check group is intended for.
    fn description(&self) -> &str;

    /// run is the main entry point that runs the checks within the check group.
    fn run(&self) -> CheckGroupResult;
}

// modules
pub mod hardware;
pub mod kernel;
pub mod script;
pub mod system;
