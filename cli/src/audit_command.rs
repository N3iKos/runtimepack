use runtimepack_core::asset_audit::{AssetAuditOptions, AuditMode, AuditOutcome};
use runtimepack_core::detection::{DetectorRegistry, EngineDescriptor};
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
use runtimepack_engine_vnmaker::VnMakerDetector;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub struct AuditRequest {
    pub source: PathBuf,
    pub rtp_path: Option<PathBuf>,
    pub mode: String,
    pub allow_network: bool,
    pub output: PathBuf,
    pub emit_html: bool,
}

pub struct AuditSummary {
    pub engine: EngineId,
    pub mode: AuditMode,
    pub allow_network: bool,
    pub outcome: AuditOutcome,
    pub output: PathBuf,
    pub rtp_path: Option<PathBuf>,
}

pub trait AuditExecutionContext {
    fn index_source(&self, source: &Path, rtp_path: Option<&Path>) -> Result<GameSourceIndex, String>;
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String>;
    fn create_dir_all(&self, path: &Path) -> Result<(), String>;
    fn write_report(&self, path: &Path, contents: &[u8]) -> Result<(), String>;
}

pub struct StaticAuditExecutionContext;

impl AuditExecutionContext for StaticAuditExecutionContext {
    fn index_source(&self, source: &Path, rtp_path: Option<&Path>) -> Result<GameSourceIndex, String> {
        GameSourceIndex::build_with_auto_rtp(source, rtp_path).map_err(|error| error.to_string())
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        fs::canonicalize(path).map_err(|error| error.to_string())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|error| error.to_string())
    }

    fn write_report(&self, path: &Path, contents: &[u8]) -> Result<(), String> {
        runtimepack_core::asset_audit::write_atomic(path, contents)
            .map_err(|error| error.to_string())
    }
}

pub fn run(
    request: AuditRequest,
    context: &dyn AuditExecutionContext,
) -> Result<AuditSummary, String> {
    let mode = match request.mode.to_ascii_lowercase().as_str() {
        "standard" => AuditMode::Standard,
        "deep" => AuditMode::Deep,
        _ => {
            return Err(format!(
                "[CMP-AUD-010] Invalid audit mode '{}'. Must be 'standard' or 'deep'.",
                request.mode
            ));
        }
    };
    let source_index = context
        .index_source(&request.source, request.rtp_path.as_deref())
        .map_err(|error| format!("[ING-101] Failed to ingest source: {error}"))?;
    if let Some(rtp_path) = source_index.rtp_path() {
        println!("RTP resolved:   {} (auto-detected)", rtp_path.display());
    }
    let descriptor = create_detector_registry().resolve(&source_index);
    validate_detection(&descriptor)?;

    let output = absolute_path(&request.output);
    let source_root = context
        .canonicalize(&request.source)
        .map_err(|error| format!("[ING-101] Failed to resolve source path: {error}"))?;
    if output.starts_with(&source_root) {
        return Err("[CMP-AUD-010] Audit output must be outside the immutable source root.".into());
    }
    let effective_rtp = request
        .rtp_path
        .as_deref()
        .or_else(|| source_index.rtp_path());
    if let Some(rtp) = effective_rtp {
        if rtp.exists() {
            if let Ok(rtp_root) = context.canonicalize(rtp) {
                if output.starts_with(&rtp_root) {
                    return Err("[CMP-AUD-010] Audit output must be outside the RTP directory.".into());
                }
            }
        }
    }
    let output_parent = output.parent().unwrap_or_else(|| Path::new("."));
    context
        .create_dir_all(output_parent)
        .map_err(|error| format!("[CMP-AUD-010] Failed to create report directory: {error}"))?;
    let resolved_parent = context
        .canonicalize(output_parent)
        .map_err(|error| format!("[CMP-AUD-010] Failed to resolve report directory: {error}"))?;
    if resolved_parent.starts_with(&source_root) {
        return Err("[CMP-AUD-010] Audit output must be outside the immutable source root.".into());
    }

    let options = AssetAuditOptions {
        mode,
        allow_network: request.allow_network,
        ..AssetAuditOptions::default()
    };

    let cache_key = runtimepack_core::asset_audit::AuditCacheKey::new(
        &source_index.source_hash,
        descriptor.id,
        mode,
        runtimepack_core::asset_audit::CURRENT_AUDIT_RULESET_VERSION,
        env!("CARGO_PKG_VERSION"),
    )
    .with_limits(options.limits.clone())
    .with_allow_network(options.allow_network)
    .with_detected_version(descriptor.detected_version.clone());

    let cache = runtimepack_core::asset_audit::AuditCache::new(
        runtimepack_core::asset_audit::AuditCache::default_dir(),
    );
    let cache_usable = cache.validate_outside_source(&source_root).is_ok();

    let (artifacts, outcome) = if cache_usable {
        if let Some(cached) = cache.get(&cache_key) {
            let parsed_outcome = serde_json::from_slice::<serde_json::Value>(&cached.report_json)
                .ok()
                .and_then(|val| {
                    val.get("outcome")
                        .and_then(|o| serde_json::from_value(o.clone()).ok())
                });
            if let Some(outcome) = parsed_outcome {
                (cached, outcome)
            } else {
                // Invalidate corrupt cached entry and fall back to fresh scan
                let _ = cache.invalidate(&cache_key);
                let report = crate::create_default_asset_auditor_registry().audit(
                    &source_index,
                    &descriptor,
                    &options,
                );
                let rendered = runtimepack_core::asset_audit::render_canonical_artifacts(&report)
                    .map_err(|e| {
                    format!("[CMP-AUD-010] Failed to render canonical audit artifacts: {e}")
                })?;
                let _ = cache.put(&cache_key, &rendered);
                (rendered, report.outcome)
            }
        } else {
            let report = crate::create_default_asset_auditor_registry().audit(
                &source_index,
                &descriptor,
                &options,
            );
            let rendered = runtimepack_core::asset_audit::render_canonical_artifacts(&report)
                .map_err(|e| {
                    format!("[CMP-AUD-010] Failed to render canonical audit artifacts: {e}")
                })?;
            let _ = cache.put(&cache_key, &rendered);
            (rendered, report.outcome)
        }
    } else {
        let report = crate::create_default_asset_auditor_registry().audit(
            &source_index,
            &descriptor,
            &options,
        );
        let rendered =
            runtimepack_core::asset_audit::render_canonical_artifacts(&report).map_err(|e| {
                format!("[CMP-AUD-010] Failed to render canonical audit artifacts: {e}")
            })?;
        (rendered, report.outcome)
    };

    let is_graph_target = output
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n == "asset-dependency-graph.json")
        .unwrap_or(false);

    if is_graph_target {
        context
            .write_report(&output, &artifacts.graph_json)
            .map_err(|error| {
                format!("[CMP-AUD-010] Failed to write dependency graph atomically: {error}")
            })?;
        let report_output = output.with_file_name("asset-audit-report.json");
        if report_output != output {
            context
                .write_report(&report_output, &artifacts.report_json)
                .map_err(|error| {
                    format!("[CMP-AUD-010] Failed to write audit report atomically: {error}")
                })?;
        }
    } else {
        context
            .write_report(&output, &artifacts.report_json)
            .map_err(|error| {
                format!("[CMP-AUD-010] Failed to write audit report atomically: {error}")
            })?;
        let graph_output = output.with_file_name("asset-dependency-graph.json");
        if graph_output != output {
            context
                .write_report(&graph_output, &artifacts.graph_json)
                .map_err(|error| {
                    format!("[CMP-AUD-010] Failed to write dependency graph atomically: {error}")
                })?;
        }
    }

    if request.emit_html {
        let html_output = output.with_file_name("asset-audit-report.html");
        let html_bytes =
            match &artifacts.html_bytes {
                Some(bytes) => bytes.clone(),
                None => runtimepack_core::asset_audit::render_audit_html(&artifacts).map_err(
                    |error| format!("[CMP-AUD-010] Failed to render HTML audit report: {error}"),
                )?,
            };
        context
            .write_report(&html_output, &html_bytes)
            .map_err(|error| {
                format!("[CMP-AUD-010] Failed to write HTML audit report atomically: {error}")
            })?;
    }

    let resolved_rtp = request
        .rtp_path
        .or_else(|| source_index.rtp_path().map(|p| p.to_path_buf()));

    Ok(AuditSummary {
        engine: descriptor.id,
        mode,
        allow_network: request.allow_network,
        outcome,
        output,
        rtp_path: resolved_rtp,
    })
}

fn validate_detection(descriptor: &EngineDescriptor) -> Result<(), String> {
    if descriptor.confidence == DetectionConfidence::Ambiguous {
        return Err(
            "[DET-110] Detection ambiguous: Multiple contradictory engine markers found.".into(),
        );
    }
    if descriptor.id == EngineId::Unknown {
        return Err("[DET-100] Detection failed: Engine could not be identified.".into());
    }
    Ok(())
}

fn create_detector_registry() -> DetectorRegistry {
    let mut registry = DetectorRegistry::new();
    registry.register(Box::new(RpgMakerXpDetector));
    registry.register(Box::new(RpgMakerVxDetector));
    registry.register(Box::new(RpgMakerVxAceDetector));
    registry.register(Box::new(RpgMakerMvDetector));
    registry.register(Box::new(RpgMakerMzDetector));
    registry.register(Box::new(RenPyDetector));
    registry.register(Box::new(TyranoDetector));
    registry.register(Box::new(VnMakerDetector));
    registry
}

fn absolute_path(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    normalize_path(&path)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}
