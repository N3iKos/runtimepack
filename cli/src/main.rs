use clap::{Parser, Subcommand};
use runtimepack_core::compatibility::CompatibilityReport;
use runtimepack_core::detection::DetectorRegistry;
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_core::provenance::{EvidenceRegistry, ToolchainLock};
use runtimepack_core::runtime_registry::RuntimeRegistry;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
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

    let report = CompatibilityReport::new(&descriptor);

    println!("Engine:         {}", descriptor.id);
    println!("Confidence:     {}", descriptor.confidence);
    println!("Score:          {}", descriptor.score);
    println!("Status:         {}", report.status);
    if let Some(family) = descriptor.id.default_runtime_family() {
        println!("Runtime Family: {}", family);
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
        Some(Commands::Build { path, .. }) => {
            println!("Build command will package {}", path.display());
            // Build orchestration is Milestone 2+
            println!("Notice: Build pipeline milestone is under development.");
        }
        Some(Commands::Licenses) => cmd_licenses(),
        None => {
            println!("runtimepack 0.1.0");
            println!("Run 'runtimepack --help' for usage.");
        }
    }
}
