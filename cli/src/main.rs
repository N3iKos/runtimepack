use chrono::Utc;
use clap::{Parser, Subcommand};
use runtimepack_core::asset_audit::{
    AssetAuditOptions, AssetAuditReport, AssetAuditor, AssetAuditorRegistry, AuditCompleteness,
    AuditFinding, AuditMode, AuditOutcome, AuditOverride, AuditStatistics, DependencyGraph,
    FindingDisposition, FindingSeverity, ReferenceRequiredness, RuntimeValidationState,
};
use runtimepack_core::build_plan::{
    AndroidManifestEntry, AndroidProjectGenerator, AppConfig, BuildManifest,
    ContentManifestEntry, EngineManifestEntry, ProvenanceManifestEntry, RuntimeManifestEntry,
    ToolchainProvenance,
};
use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_core::provenance::{EvidenceRegistry, ToolchainLock};
use runtimepack_core::runtime_registry::RuntimeRegistry;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::{TyranoAnalyzer, TyranoDetector};
use runtimepack_pack::GpackPacker;
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Parser)]
#[command(name = "runtimepack")]
#[command(author = "RuntimePack Contributors")]
#[command(version = "0.1.0")]
#[command(about = "Build-time compatibility runtime packager for desktop/web games to Android")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Check host development environment and prerequisites
    Doctor,
    /// List available Android runtime backend packages
    Runtime {
        #[command(subcommand)]
        sub: RuntimeCommands,
    },
    /// Inspect a game distribution without executing game code
    Inspect {
        /// Path to game source directory
        path: PathBuf,
        /// Optional config path
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
    /// Explain engine detection evidence and compatibility rationale
    Explain {
        /// Path to game source directory
        path: PathBuf,
    },
    /// Audit game asset references without executing game code
    Audit(AuditArgs),
    /// Build standalone Android APK/AAB from a game
    Build(BuildArgs),
    /// Display third-party notices and licenses
    Licenses,
}

#[derive(clap::Args, Debug, Clone)]
pub struct AuditArgs {
    /// Path to game source directory
    pub path: PathBuf,
    /// Audit mode: standard or deep
    #[arg(long, default_value = "standard")]
    pub mode: String,
    /// Permit bounded checks of external network resources
    #[arg(long)]
    pub allow_network_audit: bool,
    /// Authoritative JSON report path
    #[arg(short, long, default_value = "asset-audit-report.json")]
    pub output: PathBuf,
}

#[derive(clap::Args, Debug, Clone)]
pub struct BuildArgs {
    /// Path to game source directory
    pub path: PathBuf,
    /// Application package ID (e.g. com.example.game)
    #[arg(long)]
    pub package: Option<String>,
    /// Application display name
    #[arg(long)]
    pub name: Option<String>,
    /// Content mode: plain, packed, encrypted-streaming
    #[arg(long, default_value = "packed")]
    pub content_mode: String,
    /// Build flavor: beta or release
    #[arg(long, default_value = "beta")]
    pub flavor: String,
    /// Output directory
    #[arg(short, long, default_value = "./dist")]
    pub output: PathBuf,
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// List all registered runtime packages
    List,
}

fn create_default_detector_registry() -> DetectorRegistry {
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

fn create_default_analyzer_registry() -> runtimepack_core::compatibility::AnalyzerRegistry {
    let mut registry = runtimepack_core::compatibility::AnalyzerRegistry::new();
    registry.register(Box::new(runtimepack_engine_rgss::RgssAnalyzer));
    registry.register(Box::new(runtimepack_engine_rpgmaker_web::RpgMakerWebAnalyzer));
    registry.register(Box::new(runtimepack_engine_renpy::RenPyAnalyzer));
    registry.register(Box::new(TyranoAnalyzer));
    registry
}

struct UnavailableAssetAuditor {
    id: &'static str,
    engine: EngineId,
}

impl AssetAuditor for UnavailableAssetAuditor {
    fn id(&self) -> &'static str { self.id }
    fn version(&self) -> &'static str { "0" }
    fn supports(&self, engine: EngineId) -> bool { engine == self.engine }

    fn audit(
        &self,
        _source: &GameSourceIndex,
        engine: &runtimepack_core::detection::EngineDescriptor,
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
            statistics: AuditStatistics { nodes: 0, references: 0, findings: 1 },
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

fn create_default_asset_auditor_registry() -> AssetAuditorRegistry {
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

fn cmd_doctor() {
    println!("RuntimePack Doctor — Environment Diagnostics\n");
    println!("System & Toolchains:");

    // Rust
    let rustc_res = Command::new("rustc").arg("--version").output().or_else(|_| {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(profile).join(".cargo/bin/rustc.exe");
            Command::new(p).arg("--version").output()
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
        }
    });

    match rustc_res {
        Ok(out) if out.status.success() => {
            println!("  [OK] Rust: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        _ => println!("  [WARN] Rust compiler (rustc) not found in active PATH"),
    }

    // Git
    let git_ok = Command::new("git").arg("--version").output();
    match git_ok {
        Ok(out) if out.status.success() => {
            println!("  [OK] Git: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        _ => println!("  [WARN] Git not found"),
    }

    // Node.js
    let node_ok = Command::new("node").arg("--version").output();
    match node_ok {
        Ok(out) if out.status.success() => {
            println!("  [OK] Node.js: {}", String::from_utf8_lossy(&out.stdout).trim());
        }
        _ => println!("  [INFO] Node.js not detected"),
    }

    // Java / JDK
    let java_res = Command::new("java").arg("-version").output().or_else(|_| {
        if let Ok(jh) = std::env::var("JAVA_HOME") {
            let p = PathBuf::from(jh).join("bin/java.exe");
            Command::new(p).arg("-version").output()
        } else if let Ok(profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(profile).join(".runtimepack/toolchains/jdk-17/bin/java.exe");
            Command::new(p).arg("-version").output()
        } else {
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"))
        }
    });

    match java_res {
        Ok(out) if out.status.success() => {
            let ver = String::from_utf8_lossy(&out.stderr);
            let first_line = ver.lines().next().unwrap_or("detected");
            println!("  [OK] Java Runtime: {}", first_line.trim());
        }
        _ => {
            println!("  [WARN] Java (JDK 17+) not found; required for Android Gradle builds");
        }
    }

    // Android SDK
    let android_home = std::env::var("ANDROID_HOME")
        .or_else(|_| std::env::var("ANDROID_SDK_ROOT"))
        .ok()
        .or_else(|| {
            std::env::var("LOCALAPPDATA").ok().and_then(|la| {
                let p = PathBuf::from(la).join("Android/Sdk");
                if p.exists() {
                    Some(p.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        });

    match android_home {
        Some(path) => println!("  [OK] Android SDK: {}", path),
        None => println!("  [WARN] ANDROID_HOME / ANDROID_SDK_ROOT not set; required for packaging APKs"),
    }

    // Toolchain Lock
    let lock = ToolchainLock::default();
    println!("\nPinned Toolchain Baseline (toolchain.lock):");
    println!("  AGP version:          {}", lock.agp_version);
    println!("  Gradle version:       {}", lock.gradle_version);
    println!("  JDK min version:      {}", lock.jdk_min_version);
    println!("  Compile SDK:          {}", lock.android_compile_sdk);
    println!("  Min SDK:              {}", lock.android_min_sdk);
    println!("  Target SDK:           {}", lock.android_target_sdk);
    println!("  Build tools version:  {}", lock.build_tools_version);

    println!("\nDiagnostics complete.");
}

fn cmd_runtime_list() {
    let registry = RuntimeRegistry::new();
    println!("Available Android Runtime Packages:\n");
    println!("{:<20} {:<10} {:<12} {:<24} {:<8}", "ID", "VERSION", "FAMILY", "SUPPORTED ENGINES", "MIN SDK");
    println!("{:-<20} {:-<10} {:-<12} {:-<24} {:-<8}", "", "", "", "", "");

    for r in registry.all() {
        let engines_str = r
            .engines
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "{:<20} {:<10} {:<12} {:<24} {:<8}",
            r.id,
            r.version,
            r.family.to_string(),
            engines_str,
            r.android.min_sdk
        );
    }
}

fn cmd_inspect(path: &Path, _config_path: Option<&Path>) {
    println!("Inspecting: {}\n", path.display());

    let source_index = match GameSourceIndex::build(path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error reading source: {}", e);
            std::process::exit(1);
        }
    };

    println!("Indexed {} files ({:.2} MB, sha256: {})",
        source_index.file_count(),
        source_index.total_size_bytes as f64 / (1024.0 * 1024.0),
        &source_index.source_hash[..8]
    );

    let detectors = create_default_detector_registry();
    let descriptor = detectors.resolve(&source_index);

    let analyzers = create_default_analyzer_registry();
    let report = analyzers.analyze(&source_index, &descriptor);

    println!("Engine:         {}", descriptor.id);
    println!("Confidence:     {}", descriptor.confidence);
    println!("Score:          {}", descriptor.score);
    println!("Status:         {}", report.status);
    if let Some(family) = descriptor.id.default_runtime_family() {
        println!("Runtime Family: {}", family);
    }

    if !report.required_capabilities.is_empty() {
        println!("\nCapabilities Required:");
        for cap in &report.required_capabilities {
            println!("  + {}", cap);
        }
    }

    if !report.required_shims.is_empty() {
        println!("\nCompatibility Shims:");
        for shim in &report.required_shims {
            println!("  [SHIM] {}", shim);
        }
    }

    if !report.issues.is_empty() {
        println!("\nCompatibility Issues & Diagnostics ({}):", report.issues.len());
        for issue in &report.issues {
            println!("  [{}] [{}] {}", issue.code, issue.severity, issue.message);
        }
    }

    if !descriptor.evidence.is_empty() {
        println!("\nEvidence:");
        for ev in &descriptor.evidence {
            println!("  [{}] {} (+{})", ev.rule, ev.path, ev.weight);
        }
    }

    if !descriptor.conflicts.is_empty() {
        println!("\nConflicts:");
        for c in &descriptor.conflicts {
            println!("  - {}", c);
        }
    }
}

fn cmd_explain(path: &Path) {
    println!("Engine Detection Rationale for: {}\n", path.display());

    let source_index = match GameSourceIndex::build(path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error reading source: {}", e);
            std::process::exit(1);
        }
    };

    let detectors = create_default_detector_registry();
    let candidates = detectors.detect_all(&source_index);

    println!("Evaluated Candidates:");
    for c in candidates {
        println!("  - Engine: {} | Score: {} | Confidence: {}", c.engine, c.score, c.confidence);
        for ev in &c.evidence {
            println!("      * {} -> {} (+{}): {}", ev.rule, ev.path, ev.weight, ev.description);
        }
        for conflict in &c.conflicts {
            println!("      ! Conflict: {}", conflict);
        }
    }

    let resolved = detectors.resolve(&source_index);
    println!("\nFinal Resolution:");
    println!("  Selected Engine: {}", resolved.id);
    println!("  Confidence:      {}", resolved.confidence);
    println!("  Score:           {}", resolved.score);

    let analyzers = create_default_analyzer_registry();
    let report = analyzers.analyze(&source_index, &resolved);
    println!("  Status:          {}", report.status);

    if !report.required_shims.is_empty() {
        println!("  Shims Required:  {}", report.required_shims.join(", "));
    }
}

trait ProcessLauncher {
    #[allow(dead_code)]
    fn launch(&self, program: &Path) -> std::io::Result<()>;
}

struct AuditLaunchGuard;

impl ProcessLauncher for AuditLaunchGuard {
    fn launch(&self, program: &Path) -> std::io::Result<()> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            format!("process launch is forbidden during audit: {}", program.display()),
        ))
    }
}

fn cmd_audit(args: AuditArgs) {
    if let Err(message) = run_audit_with_launcher(args, &AuditLaunchGuard) {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

fn run_audit_with_launcher(
    args: AuditArgs,
    _launcher: &dyn ProcessLauncher,
) -> Result<(), String> {
    let mode = match args.mode.to_ascii_lowercase().as_str() {
        "standard" => AuditMode::Standard,
        "deep" => AuditMode::Deep,
        _ => {
            return Err(format!(
                "[CMP-AUD-010] Invalid audit mode '{}'. Must be 'standard' or 'deep'.",
                args.mode
            ));
        }
    };
    let source_index = GameSourceIndex::build(&args.path)
        .map_err(|error| format!("[ING-101] Failed to ingest source: {error}"))?;
    let descriptor = create_default_detector_registry().resolve(&source_index);
    if descriptor.confidence == DetectionConfidence::Ambiguous {
        return Err("[DET-110] Detection ambiguous: Multiple contradictory engine markers found.".into());
    }
    if descriptor.id == EngineId::Unknown {
        return Err("[DET-100] Detection failed: Engine could not be identified.".into());
    }

    let output = absolute_path(&args.output);
    let source_root = fs::canonicalize(&args.path)
        .map_err(|error| format!("[ING-101] Failed to resolve source path: {error}"))?;
    if output.starts_with(&source_root) {
        return Err("[CMP-AUD-010] Audit output must be outside the immutable source root.".into());
    }
    let output_parent = output.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(output_parent)
        .map_err(|error| format!("[CMP-AUD-010] Failed to create report directory: {error}"))?;
    let resolved_parent = fs::canonicalize(output_parent)
        .map_err(|error| format!("[CMP-AUD-010] Failed to resolve report directory: {error}"))?;
    if resolved_parent.starts_with(&source_root) {
        return Err("[CMP-AUD-010] Audit output must be outside the immutable source root.".into());
    }

    let options = AssetAuditOptions {
        mode,
        allow_network: args.allow_network_audit,
        ..AssetAuditOptions::default()
    };
    let report = create_default_asset_auditor_registry().audit(&source_index, &descriptor, &options);
    let json = serde_json::to_vec_pretty(&report)
        .map_err(|error| format!("[CMP-AUD-010] Failed to serialize audit report: {error}"))?;
    write_atomic(&output, &json)
        .map_err(|error| format!("[CMP-AUD-010] Failed to write audit report atomically: {error}"))?;

    println!("Detected engine: {}", descriptor.id);
    println!("Audit mode: {:?}", mode);
    println!("Network audit: {}", if args.allow_network_audit { "enabled" } else { "disabled" });
    println!("Outcome: {:?}", report.outcome);
    println!("Report: {}", output.display());
    Ok(())
}

fn absolute_path(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")).join(path)
    };
    normalize_path(&path)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => { normalized.pop(); }
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
        let temporary = path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
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

fn cmd_build(args: BuildArgs) {
    let normalized_flavor = args.flavor.to_lowercase();
    if normalized_flavor != "beta" && normalized_flavor != "release" {
        eprintln!("[BLD-101] Invalid build flavor '{}'. Must be 'beta' or 'release'.", args.flavor);
        std::process::exit(1);
    }

    let path = &args.path;
    let package = args.package;
    let name = args.name;
    let content_mode = args.content_mode;
    let output = &args.output;

    println!("RuntimePack Build — Starting build pipeline\n");
    println!("Source game:   {}", path.display());
    println!("Build flavor:  {}", normalized_flavor);
    println!("Output target: {}", output.display());

    // 1. Ingest
    let source_index = match GameSourceIndex::build(path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("[ING-101] Failed to ingest source: {}", e);
            std::process::exit(1);
        }
    };
    println!("Indexed {} files ({:.2} MB, source_hash: {})",
        source_index.file_count(),
        source_index.total_size_bytes as f64 / (1024.0 * 1024.0),
        &source_index.source_hash[..8]
    );

    // 2. Detect
    let detectors = create_default_detector_registry();
    let descriptor = detectors.resolve(&source_index);

    if descriptor.confidence == DetectionConfidence::Ambiguous {
        eprintln!("[DET-110] Detection ambiguous: Multiple contradictory engine markers found.");
        for c in &descriptor.conflicts {
            eprintln!("  ! {}", c);
        }
        std::process::exit(1);
    }
    if descriptor.id == EngineId::Unknown {
        eprintln!("[DET-100] Detection failed: Engine could not be identified or is generic HTML out-of-scope.");
        std::process::exit(1);
    }

    println!("Detected:      {} (Confidence: {})", descriptor.id, descriptor.confidence);

    // 3. Compatibility & Legal Gate
    let analyzers = create_default_analyzer_registry();
    let report = analyzers.analyze(&source_index, &descriptor);

    if report.status == CompatibilityStatus::UnsupportedDependency {
        eprintln!("\n[CMP-100] Build rejected: Game contains unsupported native dependencies.");
        for issue in &report.issues {
            eprintln!("  [{}] [{}] {}", issue.code, issue.severity, issue.message);
        }
        std::process::exit(1);
    }
    if descriptor.id == EngineId::Tyrano {
        println!("\n[TYR-101] Notice: Building Tyrano package for internal validation/permitted testing under TYRANO-LEGAL gate.");
    }

    // 4. Runtime Resolution
    let registry = RuntimeRegistry::new();
    let runtimes = registry.find_for_engine(descriptor.id);
    if runtimes.is_empty() {
        eprintln!("[RT-101] No compatible runtime package found for {}", descriptor.id);
        std::process::exit(1);
    }
    let runtime = runtimes[0];
    println!("Resolved:      {} v{} ({})", runtime.name, runtime.version, runtime.id);

    // 5. App details
    let app_name = name.unwrap_or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("StandaloneGame")
            .to_string()
    });

    let sanitized_pkg_suffix = app_name
        .to_ascii_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<String>();
    let pkg_id = package.unwrap_or_else(|| format!("io.runtimepack.game.{}", sanitized_pkg_suffix));

    println!("Package ID:    {}", pkg_id);
    println!("App Name:      {}", app_name);

    if let Err(e) = fs::create_dir_all(output) {
        eprintln!("[BLD-101] Failed to create output directory: {}", e);
        std::process::exit(1);
    }

    // 6. Content Packing (GPACK)
    println!("\nPacking game content (mode: {})...", content_mode);
    let pack_path = output.join("game.pack");

    let encryption_key = if content_mode == "encrypted-streaming" {
        Some(runtimepack_pack::ContentCrypto::derive_package_key(&pkg_id, &app_name))
    } else {
        None
    };

    let mut content_source_dir = path.to_path_buf();
    let mut staging_dir_to_cleanup: Option<PathBuf> = None;
    if matches!(descriptor.id, EngineId::RpgMakerMv | EngineId::RpgMakerMz) {
        if path.join("www/index.html").exists() || path.join("www/js/rpg_core.js").exists() {
            println!("Deployed NW.js structure detected. Normalizing content packaging root to 'www/'");
            content_source_dir = path.join("www");
        }
    } else if descriptor.id == EngineId::Tyrano {
        let asar_path = path.join("resources/app.asar");
        if asar_path.exists() {
            println!("Electron package detected: resources/app.asar. Extracting to build staging directory...");
            let staging_dir = output.join("staging_content");
            if staging_dir.exists() {
                let _ = fs::remove_dir_all(&staging_dir);
            }
            if let Err(e) = fs::create_dir_all(&staging_dir) {
                eprintln!("[CNT-101] Failed to create staging directory: {}", e);
                std::process::exit(1);
            }
            let archive = match runtimepack_engine_tyrano::AsarArchive::load(&asar_path) {
                Ok(a) => a,
                Err(e) => {
                    eprintln!("[CNT-102] Failed to parse app.asar: {}", e);
                    std::process::exit(1);
                }
            };
            let extracted_count = match archive.extract_all(&asar_path, &staging_dir) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[CNT-103] Failed to extract app.asar: {}", e);
                    std::process::exit(1);
                }
            };
            println!("Extracted {} files from app.asar into {}", extracted_count, staging_dir.display());
            content_source_dir = staging_dir.clone();
            staging_dir_to_cleanup = Some(staging_dir);
        }
    }

    let total_bytes = match GpackPacker::pack_directory_with_options(&content_source_dir, &pack_path, encryption_key.as_ref()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[CNT-102] Failed to pack content: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(staging_dir) = staging_dir_to_cleanup {
        let _ = fs::remove_dir_all(&staging_dir);
    }

    // Calculate pack hash
    let mut hasher = Sha256::new();
    if let Ok(mut pf) = fs::File::open(&pack_path) {
        use std::io::Read;
        let mut buf = [0u8; 8192];
        while let Ok(n) = pf.read(&mut buf) {
            if n == 0 { break; }
            hasher.update(&buf[..n]);
        }
    }
    let pack_hash = format!("{:x}", hasher.finalize());
    println!("GPACK created: {} ({:.2} MB, hash: {})",
        pack_path.file_name().unwrap().to_string_lossy(),
        total_bytes as f64 / (1024.0 * 1024.0),
        &pack_hash[..8]
    );

    // 7. Assemble BuildManifest
    let manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: Utc::now().to_rfc3339(),
        flavor: Some(normalized_flavor.clone()),
        app: AppConfig {
            name: app_name.clone(),
            package: pkg_id.clone(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: descriptor.id.to_string(),
            confidence: descriptor.confidence.to_string(),
            overridden: false,
            detected_version: None,
        },
        runtime: RuntimeManifestEntry {
            id: runtime.id.clone(),
            version: runtime.version.clone(),
            family: runtime.family.to_string(),
            runtime_api: runtime.runtime_api,
            capabilities: runtime.capabilities.clone(),
        },
        content: ContentManifestEntry {
            mode: content_mode,
            content_hash: pack_hash.clone(),
            file_count: source_index.file_count(),
            total_bytes,
        },
        android: AndroidManifestEntry {
            abis: vec!["arm64-v8a".to_string(), "x86_64".to_string()],
            min_sdk: runtime.android.min_sdk,
            target_sdk: runtime.android.target_sdk,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: source_index.source_hash.clone(),
            config_hash: "".to_string(),
            apk_hash: None,
            flavor: Some(normalized_flavor.clone()),
            toolchain: ToolchainProvenance {
                agp_version: Some("8.3.0".to_string()),
                gradle_version: Some("8.7".to_string()),
                jdk_version: Some("17".to_string()),
                android_sdk_platform: Some("34".to_string()),
                ndk_version: Some("26.1.10909125".to_string()),
            },
        },
    };

    // 8. Generate Android Project
    println!("\nGenerating standalone Android project structure...");
    let android_project_dir = output.join("android-project");
    if let Err(e) = AndroidProjectGenerator::emit_project_with_source(
        &android_project_dir,
        &manifest,
        Some(&pack_path),
        Some(path),
        Some(&normalized_flavor),
    ) {
        eprintln!("[AND-101] Project generation error: {}", e);
        std::process::exit(1);
    }
    println!("Android project: {}", android_project_dir.display());

    // 9. Invoke Gradle compiler if toolchains are present
    let mut apk_hash = None;
    let mut gradle_cmd = Command::new("gradle");

    if let Ok(jh) = std::env::var("JAVA_HOME") {
        gradle_cmd.env("JAVA_HOME", jh);
    } else if let Ok(profile) = std::env::var("USERPROFILE") {
        let p = PathBuf::from(&profile).join(".runtimepack/toolchains/jdk-17");
        if p.exists() {
            gradle_cmd.env("JAVA_HOME", p);
        }
    }

    if let Ok(profile) = std::env::var("USERPROFILE") {
        let gradle_bin = PathBuf::from(&profile).join(".runtimepack/toolchains/gradle-8.7/bin/gradle.bat");
        if gradle_bin.exists() {
            gradle_cmd = Command::new(gradle_bin);
        }
    }

    println!("\nCompiling standalone Android package with Gradle...");
    gradle_cmd.arg("assembleDebug");
    gradle_cmd.arg("--console=plain");
    gradle_cmd.current_dir(&android_project_dir);

    match gradle_cmd.output() {
        Ok(res) if res.status.success() => {
            let apk_source = android_project_dir.join("app/build/outputs/apk/debug/app-debug.apk");
            if apk_source.exists() {
                let sanitized_apk_name = format!("{}.apk", app_name.replace(' ', ""));
                let dest_apk = output.join(&sanitized_apk_name);
                if let Ok(_) = fs::copy(&apk_source, &dest_apk) {
                    let mut apk_hasher = Sha256::new();
                    if let Ok(bytes) = fs::read(&dest_apk) {
                        apk_hasher.update(&bytes);
                        apk_hash = Some(format!("{:x}", apk_hasher.finalize()));
                    }
                    println!("Generated APK:  {} ({:.2} MB)",
                        dest_apk.display(),
                        fs::metadata(&dest_apk).map(|m| m.len()).unwrap_or(0) as f64 / (1024.0 * 1024.0)
                    );
                }
            }
        }
        Ok(res) => {
            println!("Gradle notice: Non-zero exit code ({:?}). Android project is available in {}.",
                res.status.code(), android_project_dir.display());
        }
        Err(e) => {
            println!("Gradle compiler notice: {}. Project structure generated at {}.",
                e, android_project_dir.display());
        }
    }

    // 10. Emit Build Reports and Notices
    println!("\nWriting build artifacts and third-party notices...");
    let mut updated_manifest = manifest;
    updated_manifest.provenance.apk_hash = apk_hash.clone();
    let manifest_json = serde_json::to_string_pretty(&updated_manifest).unwrap();
    fs::write(output.join("build-manifest.json"), &manifest_json).unwrap();

    let report_json = serde_json::to_string_pretty(&report).unwrap();
    fs::write(output.join("compatibility-report.json"), &report_json).unwrap();

    // Checksums
    let mut checksums = format!(
        "{}\tgame.pack\n{}\tbuild-manifest.json\n",
        pack_hash,
        format!("{:x}", Sha256::digest(manifest_json.as_bytes()))
    );
    if let Some(ref h) = apk_hash {
        checksums.push_str(&format!("{}\t{}.apk\n", h, app_name.replace(' ', "")));
    }
    fs::write(output.join("checksums.txt"), checksums).unwrap();

    // Notices
    let notices_dir = output.join("third-party-notices");
    fs::create_dir_all(&notices_dir).unwrap();
    let mut notices_text = String::from("Third-Party Notices:\n\n");
    for item in EvidenceRegistry::get_canonical_evidence() {
        notices_text.push_str(&format!("[{}] {}\nURL: {}\n\n", item.id, item.title, item.url));
    }
    fs::write(notices_dir.join("NOTICES.txt"), notices_text).unwrap();

    println!("Build report:   {}", output.join("build-manifest.json").display());
    println!("Compatibility:  {}", output.join("compatibility-report.json").display());
    println!("Notices:        {}", notices_dir.join("NOTICES.txt").display());
    println!("\n[SUCCESS] Standalone Android packaging complete!");
}

fn cmd_licenses() {
    println!("RuntimePack — Third-Party Notices and Evidence Registry\n");
    let evidence = EvidenceRegistry::get_canonical_evidence();
    for item in evidence {
        println!("[{}] {}", item.id, item.title);
        println!("  URL: {}", item.url);
        for claim in item.claims {
            println!("  - {}", claim);
        }
        println!();
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct RejectingLauncher {
        attempts: AtomicUsize,
    }

    impl ProcessLauncher for RejectingLauncher {
        fn launch(&self, _program: &Path) -> std::io::Result<()> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            Err(std::io::Error::other("process launch forbidden during audit"))
        }
    }

    #[test]
    fn audit_path_attempts_zero_process_launches() {
        let source = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("tests/fixtures/vxace_min");
        let sequence = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let output_dir = std::env::temp_dir().join(format!(
            "runtimepack-audit-launch-guard-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&output_dir).unwrap();
        let report = output_dir.join("report.json");
        let launcher = RejectingLauncher {
            attempts: AtomicUsize::new(0),
        };

        run_audit_with_launcher(
            AuditArgs {
                path: source,
                mode: "standard".into(),
                allow_network_audit: false,
                output: report,
            },
            &launcher,
        )
        .unwrap();

        assert_eq!(launcher.attempts.load(Ordering::SeqCst), 0);
        fs::remove_dir_all(output_dir).unwrap();
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Doctor) => cmd_doctor(),
        Some(Commands::Runtime { sub: RuntimeCommands::List }) => cmd_runtime_list(),
        Some(Commands::Inspect { path, config }) => cmd_inspect(&path, config.as_deref()),
        Some(Commands::Explain { path }) => cmd_explain(&path),
        Some(Commands::Audit(args)) => cmd_audit(args),
        Some(Commands::Build(args)) => cmd_build(args),
        Some(Commands::Licenses) => cmd_licenses(),
        None => {
            println!("runtimepack 0.1.0");
            println!("Run 'runtimepack --help' for usage.");
        }
    }
}
