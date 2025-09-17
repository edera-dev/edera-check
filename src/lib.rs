use log::{error, info, warn};
use std::fmt;

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

pub struct CheckResult {
    pub name: String,
    pub result: CheckResultValue,
}

impl CheckResult {
    pub fn new(name: &str, result: CheckResultValue) -> Self {
        Self {
            name: name.to_string(),
            result,
        }
    }
}

pub struct CheckGroupResult {
    pub name: String,
    pub result: CheckResultValue,
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

pub trait CheckGroup {
    fn name(&self) -> &str;
    fn id(&self) -> &str;
    fn description(&self) -> &str;
    fn run(&self) -> CheckGroupResult;
}

pub mod script;
pub mod system;
