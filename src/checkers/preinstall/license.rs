use crate::helpers::{
    CheckGroup, CheckGroupCategory, CheckGroupResult, CheckResult,
    CheckResultValue::{Errored, Failed, Passed},
};
use async_trait::async_trait;
use edera_license_client::{LICENSE_KEY_ENV_VAR, LicenseClient, ValidationStatus};
use futures::{FutureExt, future::join_all};

const GROUP_IDENTIFIER: &str = "license";
const NAME: &str = "License Checks";

pub struct LicenseChecks {}

impl LicenseChecks {
    // Allowing new_without_default as our other checks don't derive Default
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {}
    }

    async fn run_all(&self) -> CheckGroupResult {
        let license_client = self
            .get_license_client()
            .await
            .map_err(|e| CheckGroupResult {
                name: NAME.to_string(),
                result: Errored("Could not create license client".to_string()),
                results: vec![e],
            });

        // This is a bit clunky as CheckGroupResult isn't currently defined like a typical Result,
        // so the ? operator can't be used to return early.
        let license_client = match license_client {
            Ok(client) => client,
            Err(e) => return e,
        };

        let results = join_all([self.check_license_legit(&license_client).boxed()]).await;

        let mut group_result = Passed;
        for res in results.iter() {
            // Set group result to Failed if we failed and aren't already in an Errored state
            if !matches!(group_result, Errored(_)) && matches!(res.result, Failed(_)) {
                group_result = Failed("".into());
            }

            if matches!(res.result, Errored(_)) {
                group_result = Errored("".into());
            }
        }

        CheckGroupResult {
            name: NAME.to_string(),
            result: group_result,
            results,
        }
    }

    async fn get_license_client(&self) -> Result<LicenseClient, CheckResult> {
        let license_key = std::env::var(LICENSE_KEY_ENV_VAR).map_err(|_| {
            CheckResult::new(NAME, Errored("LICENCE_KEY_ENV_VAR is not set".to_string()))
        })?;

        LicenseClient::new(&license_key).map_err(|e| CheckResult::new(NAME, Errored(e.to_string())))
    }

    async fn check_license_legit(&self, license_client: &LicenseClient) -> CheckResult {
        let validation_status = match license_client.check_validation_status().await {
            Ok(status) => status,
            Err(e) => return CheckResult::new(NAME, Failed(e.to_string())),
        };

        if matches!(validation_status, ValidationStatus::LicenseNotActivated) {
            CheckResult::new(NAME, Passed)
        } else {
            CheckResult::new(
                NAME,
                Failed(format!(
                    "Unexpected ValidationStatus: {}",
                    validation_status
                )),
            )
        }
    }
}

#[async_trait]
impl CheckGroup for LicenseChecks {
    fn name(&self) -> &str {
        NAME
    }

    fn id(&self) -> &str {
        GROUP_IDENTIFIER
    }

    fn description(&self) -> &str {
        "Checks that the license provided by the EDERA_LICENSE_KEY environment variable is legitimate and not registered to another instance"
    }

    async fn run(&self) -> CheckGroupResult {
        self.run_all().await
    }

    fn category(&self) -> CheckGroupCategory {
        CheckGroupCategory::Optional("License checks can be omitted if user wishes to provide the license after edera-check is run".into())
    }
}
