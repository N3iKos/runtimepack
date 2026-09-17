use chrono::Utc;
use clap::{Parser, Subcommand};
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
use runtimepack_engine_tyrano::TyranoDetector;
use runtimepack_pack::GpackPacker;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    /// Build standalone Android APK/AAB from a game
    Build {
        /// Path to game source directory
        path: PathBuf,
        /// Application package ID (e.g. com.example.game)
        #[arg(long)]
        package: Option<String>,
        /// Application display name
        #[arg(long)]
        name: Option<String>,
        /// Content mode: plain, packed, encrypted-streaming
        #[arg(long, default_value = "packed")]
        content_mode: String,
        /// Output directory
        #[arg(short, long, default_value = "./dist")]
        output: PathBuf,
    },
    /// Display third-party notices and licenses
    Licenses,
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
        .ok();

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

fn cmd_build(
    path: &Path,
    package: Option<String>,
    name: Option<String>,
    content_mode: String,
    output: &Path,
) {
    println!("RuntimePack Build — Starting build pipeline\n");
    println!("Source game:   {}", path.display());
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
        eprintln!("\n[TYR-101] Build BLOCKED by TYRANO-LEGAL gate.");
        eprintln!("TyranoScript terms restrict redistribution/smartphone conversion packaging.");
        eprintln!("Engine detection and analysis are supported, but APK bundling is blocked until legal clearance.");
        std::process::exit(1);
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
    println!("\nPacking game content into GPACK container...");
    let pack_path = output.join("game.pack");
    let total_bytes = match GpackPacker::pack_directory(path, &pack_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[CNT-102] Failed to pack content: {}", e);
            std::process::exit(1);
        }
    };

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
    if let Err(e) = AndroidProjectGenerator::emit_project(&android_project_dir, &manifest, Some(&pack_path)) {
        eprintln!("[AND-101] Project generation error: {}", e);
        std::process::exit(1);
    }
    println!("Android project: {}", android_project_dir.display());

    // 9. Emit Build Reports and Notices
    println!("\nWriting build artifacts and third-party notices...");
    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(output.join("build-manifest.json"), &manifest_json).unwrap();

    let report_json = serde_json::to_string_pretty(&report).unwrap();
    fs::write(output.join("compatibility-report.json"), &report_json).unwrap();

    // Checksums
    let checksums = format!(
        "{}\tgame.pack\n{}\tbuild-manifest.json\n",
        pack_hash,
        format!("{:x}", Sha256::digest(manifest_json.as_bytes()))
    );
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
    println!("\n[SUCCESS] Build preparation completed successfully!");
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Doctor) => cmd_doctor(),
        Some(Commands::Runtime { sub: RuntimeCommands::List }) => cmd_runtime_list(),
        Some(Commands::Inspect { path, config }) => cmd_inspect(&path, config.as_deref()),
        Some(Commands::Explain { path }) => cmd_explain(&path),
        Some(Commands::Build {
            path,
            package,
            name,
            content_mode,
            output,
        }) => cmd_build(&path, package, name, content_mode, &output),
        Some(Commands::Licenses) => cmd_licenses(),
        None => {
            println!("runtimepack 0.1.0");
            println!("Run 'runtimepack --help' for usage.");
        }
    }
}
