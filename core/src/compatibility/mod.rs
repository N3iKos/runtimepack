use crate::detection::EngineDescriptor;
use crate::diagnostics::{DiagnosticSeverity, RemediationCategory};
use crate::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
use crate::ingest::GameSourceIndex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityIssue {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<RemediationCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatibilityReport {
    pub schema_version: u32,
    pub engine_id: EngineId,
    pub status: CompatibilityStatus,
    pub confidence: DetectionConfidence,
    pub score: i32,
    pub evidence: Vec<crate::detection::Evidence>,
    pub conflicts: Vec<String>,
    pub issues: Vec<CompatibilityIssue>,
    pub required_capabilities: Vec<String>,
    pub required_shims: Vec<String>,
}

impl CompatibilityReport {
    pub fn new(descriptor: &EngineDescriptor) -> Self {
        let status = if descriptor.confidence == DetectionConfidence::None {
            CompatibilityStatus::Unknown
        } else if descriptor.confidence == DetectionConfidence::Ambiguous {
            CompatibilityStatus::Unknown
        } else {
            CompatibilityStatus::Compatible
        };

        Self {
            schema_version: 1,
            engine_id: descriptor.id,
            status,
            confidence: descriptor.confidence,
            score: descriptor.score,
            evidence: descriptor.evidence.clone(),
            conflicts: descriptor.conflicts.clone(),
            issues: Vec::new(),
            required_capabilities: Vec::new(),
            required_shims: Vec::new(),
        }
    }

    pub fn add_issue(&mut self, issue: CompatibilityIssue) {
        if issue.severity == DiagnosticSeverity::Error
            && self.status == CompatibilityStatus::Compatible
        {
            self.status = CompatibilityStatus::UnsupportedDependency;
        }
        self.issues.push(issue);
    }
}

pub trait Analyzer: Send + Sync {
    fn supports(&self, engine: EngineId) -> bool;
    fn analyze(&self, source: &GameSourceIndex, engine: &EngineDescriptor) -> CompatibilityReport;
}

#[derive(Default)]
pub struct AnalyzerRegistry {
    analyzers: Vec<Box<dyn Analyzer>>,
}

impl AnalyzerRegistry {
    pub fn new() -> Self {
        Self {
            analyzers: Vec::new(),
        }
    }

    pub fn register(&mut self, analyzer: Box<dyn Analyzer>) {
        self.analyzers.push(analyzer);
    }

    pub fn analyze(
        &self,
        source: &GameSourceIndex,
        engine: &EngineDescriptor,
    ) -> CompatibilityReport {
        for a in &self.analyzers {
            if a.supports(engine.id) {
                return a.analyze(source, engine);
            }
        }
        CompatibilityReport::new(engine)
    }
}
