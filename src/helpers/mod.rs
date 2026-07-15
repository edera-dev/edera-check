pub mod cpu;
pub mod host_executor;
pub mod kernel;
pub mod services;

use async_trait::async_trait;
use console::{Emoji, style};
use std::fmt;

static CHECK: Emoji = Emoji("✅", "[+]");
static WARN: Emoji = Emoji("⚠", "[!]");
static ERROR: Emoji = Emoji("❌", "[X]");

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
    Skipped(String),
}

impl fmt::Display for CheckResultValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CheckResultValue::Passed => write!(f, "Passed"),
            CheckResultValue::Failed(msg) if msg.is_empty() => write!(f, "Failed"),
            CheckResultValue::Failed(msg) => write!(f, "Failed: {msg}"),
            CheckResultValue::Errored(msg) if msg.is_empty() => write!(f, "Errored"),
            CheckResultValue::Errored(msg) => write!(f, "Errored: {msg}"),
            CheckResultValue::Skipped(msg) if msg.is_empty() => write!(f, "Skipped"),
            CheckResultValue::Skipped(msg) => write!(f, "Skipped: {msg}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckGroupCategory {
    Required,
    Optional(String),
    Advisory,
}

impl fmt::Display for CheckGroupCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CheckGroupCategory::Required => write!(f, "Required"),
            CheckGroupCategory::Advisory => write!(f, "Advisory"),
            CheckGroupCategory::Optional(_) => write!(f, "Optional"),
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

    pub fn new_with_output(
        name: &str,
        result: CheckResultValue,
        output_to_record: Option<String>,
    ) -> Self {
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
    /// log_group is a pretty-print helper to log the result of the group based on what the result
    /// value is.
    pub fn log_group(&self, category: CheckGroupCategory) {
        let name = &self.name;
        let result = &self.result;
        let s = format!("{}: {}", name, result);
        match result {
            CheckResultValue::Passed => println!("{} {}", CHECK, style(s).green().bold()),
            CheckResultValue::Failed(_) => {
                if let CheckGroupCategory::Optional(opt) = &category {
                    println!(
                        "{} {}\n{} {} [Optional]",
                        WARN,
                        style(opt).yellow().dim(),
                        WARN,
                        style(s).yellow().dim()
                    )
                } else {
                    println!("{} {}", ERROR, style(s).bright().red().bold())
                }
            }
            CheckResultValue::Errored(_) => {
                if let CheckGroupCategory::Optional(opt) = &category {
                    println!(
                        "{} {}\n{} {} [Optional]",
                        WARN,
                        style(opt).yellow().dim(),
                        WARN,
                        style(s).yellow().dim()
                    )
                } else {
                    println!("{} {}", ERROR, style(s).red().bright().bold())
                }
            }
            CheckResultValue::Skipped(reason) => println!(
                "{} {} - {}",
                WARN,
                style(s).yellow().dim(),
                style(reason).yellow().dim()
            ),
        }
    }

    /// log_individual_checks is a pretty-print helper to log the results of each individual check
    /// within a group.
    pub fn log_individual_checks(&self) {
        for check_result in self.results.iter() {
            let name = &check_result.name;
            let result = &check_result.result;
            let s = format!("    • {}: {}", name, result);
            match result {
                CheckResultValue::Passed => println!("{}", style(s).magenta().dim()),
                CheckResultValue::Failed(_) => println!("{}", style(s).red().dim()),
                CheckResultValue::Errored(_) => println!("{}", style(s).red().dim()),
                CheckResultValue::Skipped(_) => println!("{}", style(s).yellow().dim()),
            }
        }
    }
}

/// CheckGroup is a trait representing a group of checks.
#[async_trait]
pub trait CheckGroup {
    /// name is the name of the check group
    fn name(&self) -> &str;

    /// id is the identifier for the check group. This field is used when skipping or selecting
    /// certain check groups to run so it should be env/cli friendly.
    fn id(&self) -> &str;

    /// description is a longer form text field explaining what this check group is intended for.
    fn description(&self) -> &str;

    /// run is the main entry point that runs the checks within the check group.
    async fn run(&self) -> CheckGroupResult;

    /// category indicates whether this check group's pass/fail state
    /// is a blocker, simply means some optional feature is unavailable, or otherwise.
    fn category(&self) -> CheckGroupCategory;
}
