use super::{
    CheckGroup, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};

use anyhow::Result;
use log::warn;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub struct ScriptChecks;

const NAME: &str = "Scripted Checks";

impl ScriptChecks {
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
        let script_list = script_list.unwrap();

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

    fn scripts_dir(&self) -> PathBuf {
        let sdir = env::var("EDERA_PREFLIGHT_SCRIPTS_DIR").unwrap_or("./scripts".to_string());
        PathBuf::from(sdir)
    }

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
                    "skipping load of {}",
                    entry.file_name().to_str().unwrap_or("unknown")
                );
                continue;
            }

            scripts.push(entry.path());
        }

        scripts.push(PathBuf::from("/totally/fake/script"));

        Ok(scripts)
    }

    fn run_script(&self, path: &PathBuf) -> CheckResult {
        let output = Command::new(path).output();

        let mut name = path.to_str().unwrap_or_default().to_string();

        if let Err(e) = output {
            return CheckResult::new(&name, Errored(e.to_string()));
        }

        // We checked if this is an error and handled that case above
        let output = output.unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);

        let mut check_name = None;

        for line in stdout.lines() {
            if let Some(value) = line.strip_prefix("EDERA_PREFLIGHT_CHECK_NAME=") {
                check_name = Some(value.to_string());
                break;
            }
        }

        if let Some(set_name) = check_name {
            name = set_name;
        }

        let result = match output.status.success() {
            true => Passed,
            false => Failed(format!("script returned {:?}", output.status.code())),
        };
        CheckResult::new(&name, result)
    }
}

impl CheckGroup for ScriptChecks {
    fn name(&self) -> &str {
        NAME
    }

    fn description(&self) -> &str {
        "Checks composed through small shell scripts"
    }

    fn run(&self) -> CheckGroupResult {
        self.run_all()
    }
}
