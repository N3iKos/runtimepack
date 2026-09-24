use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditReport, AssetAuditor, AssetAuditorRegistry, AuditCompleteness,
    AuditFinding, AuditMode, AuditOutcome, AuditOverride, AuditStatistics, DependencyGraph,
    FindingDisposition, FindingSeverity, ReferenceRequiredness, RuntimeValidationState,
};
use runtimepack_core::detection::{DetectorRegistry, EngineDescriptor};
use runtimepack_core::domain::{DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct AuditRequest {
    pub source: PathBuf,
    pub mode: String,
    pub allow_network: bool,
    pub output: PathBuf,
}

pub struct AuditSummary {
    pub engine: EngineId,
    pub mode: AuditMode,
    pub allow_network: bool,
    pub outcome: AuditOutcome,
    pub output: PathBuf,
}

pub trait AuditExecutionContext {
    fn index_source(&self, source: &Path) -> Result<GameSourceIndex, String>;
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String>;
    fn create_dir_all(&self, path: &Path) -> Result<(), String>;
    fn write_report(&self, path: &Path, contents: &[u8]) -> Result<(), String>;
}

pub struct StaticAuditExecutionContext;

impl AuditExecutionContext for StaticAuditExecutionContext {
    fn index_source(&self, source: &Path) -> Result<GameSourceIndex, String> {
        GameSourceIndex::build(source).map_err(|error| error.to_string())
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, String> {
        fs::canonicalize(path).map_err(|error| error.to_string())
    }

    fn create_dir_all(&self, path: &Path) -> Result<(), String> {
        fs::create_dir_all(path).map_err(|error| error.to_string())
    }

    fn write_report(&self, path: &Path, contents: &[u8]) -> Result<(), String> {
        write_atomic(path, contents).map_err(|error| error.to_string())
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
        .index_source(&request.source)
        .map_err(|error| format!("[ING-101] Failed to ingest source: {error}"))?;
    let descriptor = create_detector_registry().resolve(&source_index);
    validate_detection(&descriptor)?;

    let output = absolute_path(&request.output);
    let source_root = context
        .canonicalize(&request.source)
        .map_err(|error| format!("[ING-101] Failed to resolve source path: {error}"))?;
    if output.starts_with(&source_root) {
        return Err("[CMP-AUD-010] Audit output must be outside the immutable source root.".into());
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
    let report = create_asset_auditor_registry().audit(&source_index, &descriptor, &options);
    let json = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("[CMP-AUD-010] Failed to serialize audit report: {error}"))?;
    context
        .write_report(&output, &json)
        .map_err(|error| format!("[CMP-AUD-010] Failed to write audit report atomically: {error}"))?;

    Ok(AuditSummary {
        engine: descriptor.id,
        mode,
        allow_network: request.allow_network,
        outcome: report.outcome,
        output,
    })
}

fn validate_detection(descriptor: &EngineDescriptor) -> Result<(), String> {
    if descriptor.confidence == DetectionConfidence::Ambiguous {
        return Err("[DET-110] Detection ambiguous: Multiple contradictory engine markers found.".into());
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
    registry
}

struct UnavailableAssetAuditor {
    id: &'static str,
    engine: EngineId,
}

impl AssetAuditor for UnavailableAssetAuditor {
    fn id(&self) -> &'static str {
        self.id
    }

    fn version(&self) -> &'static str {
        "0"
    }

    fn supports(&self, engine: EngineId) -> bool {
        engine == self.engine
    }

    fn audit(
        &self,
        _source: &GameSourceIndex,
        engine: &EngineDescriptor,
        options: &AssetAuditOptions,
    ) -> AssetAuditReport {
        AssetAuditReport {
            schema_version: 1,
            engine: engine.id,
            mode: options.mode,
            outcome: AuditOutcome::Indeterminate,
            completeness: AuditCompleteness {
                required_scope_complete: false,
                covered_reference_classes: Vec::new(),
                uncovered_reference_classes: vec!["engine_asset_references".into()],
            },
            statistics: AuditStatistics {
                nodes: 0,
                references: 0,
                findings: 1,
            },
            graph: DependencyGraph::default(),
            findings: vec![AuditFinding {
                id: format!("{}-unavailable", self.id),
                code: "CMP-AUD-001".into(),
                severity: FindingSeverity::Warning,
                disposition: FindingDisposition::Indeterminate,
                requiredness: ReferenceRequiredness::Required,
                message: format!("asset auditor {} is not implemented", self.id),
                source_node_ids: Vec::new(),
                orphan: false,
            }],
            overrides: Vec::<AuditOverride>::new(),
            runtime_validation: RuntimeValidationState::NotRun,
        }
    }
}

fn create_asset_auditor_registry() -> AssetAuditorRegistry {
    let mut registry = AssetAuditorRegistry::new();
    for (id, engine) in [
        ("rgss1-placeholder", EngineId::RpgMakerXp),
        ("rgss2-placeholder", EngineId::RpgMakerVx),
        ("rgss3-placeholder", EngineId::RpgMakerVxAce),
        ("rpgmaker-mv-placeholder", EngineId::RpgMakerMv),
        ("rpgmaker-mz-placeholder", EngineId::RpgMakerMz),
        ("renpy-placeholder", EngineId::RenPy),
        ("tyrano-placeholder", EngineId::Tyrano),
    ] {
        registry
            .register(Box::new(UnavailableAssetAuditor { id, engine }))
            .expect("default asset auditor IDs must be unique");
    }
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

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let (temporary, mut file) = create_unique_sibling(path)?;
    let result = (|| {
        file.write_all(contents)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        replace_file(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn create_unique_sibling(path: &Path) -> std::io::Result<(PathBuf, fs::File)> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset-audit-report.json");
    for _ in 0..128 {
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let temporary = path.with_file_name(format!(
            ".{file_name}.{timestamp}.{sequence}.tmp"
        ));
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "could not allocate a unique sibling temporary report",
    ))
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;

    #[link(name = "kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
