use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StepResult {
    pub step: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub detail_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl StepResult {
    pub fn pass(step: impl Into<String>, detail_ms: u64, note: impl Into<String>) -> Self {
        Self {
            step: step.into(),
            status: "PASS".into(),
            code: None,
            detail_ms,
            note: Some(note.into()),
        }
    }

    pub fn fail(
        step: impl Into<String>,
        code: impl Into<String>,
        detail_ms: u64,
        note: impl Into<String>,
    ) -> Self {
        Self {
            step: step.into(),
            status: "FAIL".into(),
            code: Some(code.into()),
            detail_ms,
            note: Some(note.into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProbeReport {
    pub device: String,
    pub apk: String,
    pub package: String,
    pub steps: Vec<StepResult>,
    pub overall: String,
}

impl ProbeReport {
    pub fn new(device: impl Into<String>, apk: impl Into<String>, package: impl Into<String>) -> Self {
        Self {
            device: device.into(),
            apk: apk.into(),
            package: package.into(),
            steps: Vec::new(),
            overall: "PASS".into(),
        }
    }

    pub fn add_step(&mut self, step: StepResult) {
        if step.status == "FAIL" {
            self.overall = "FAIL".into();
        }
        self.steps.push(step);
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}
