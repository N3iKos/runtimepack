mod audit_command;
pub mod build_finalize;
pub mod gradle;
pub mod probe;

use chrono::Utc;
use clap::{Parser, Subcommand};
use runtimepack_core::asset_audit::{
    render_canonical_artifacts, write_audit_artifacts_with_options, AssetAuditOptions,
    AssetAuditReport, AssetAuditorRegistry, AuditCache, AuditCacheKey, AuditMode, AuditOutcome,
    CURRENT_AUDIT_RULESET_VERSION,
};
use runtimepack_core::build_plan::{
    evaluate_build_policy_with_compatibility, AndroidManifestEntry, AndroidProjectGenerator,
    AppConfig, AppliedBuildPolicy, AppliedCompatibilityBuildPolicy, AuditBuildPolicy,
    AuditManifestEntry, AuditPolicyDecision, BuildManifest, ContentManifestEntry,
    EngineManifestEntry, ProvenanceManifestEntry, RuntimeManifestEntry, ThemeManifestEntry,
    ToolchainProvenance, TypedBuildDecision, BUILD_POLICY_RULESET_VERSION,
};
use runtimepack_core::build_policy::{
    sanitize_untrusted_line, BuildPolicy, CompatibilityBuildPolicy, UNTRUSTED_LINE_LIMIT,
};
use runtimepack_core::detection::Detector;
use runtimepack_core::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use runtimepack_core::domain::{CompatibilityStatus, DetectionConfidence, EngineId};
use runtimepack_core::ingest::GameSourceIndex;
use runtimepack_core::provenance::{EvidenceRegistry, ToolchainLock};
use runtimepack_core::runtime_registry::RuntimeRegistry;
use runtimepack_core::theme::ThemePackage;
use runtimepack_engine_renpy::{RenPyAnalyzer, RenPyAssetAuditor};
use runtimepack_engine_rgss::{RgssAnalyzer, RgssAssetAuditor};
use runtimepack_engine_rpgmaker_web::{RpgMakerWebAnalyzer, RpgMakerWebAssetAuditor};
use runtimepack_engine_tyrano::{TyranoAnalyzer, TyranoAssetAuditor};
use runtimepack_engine_vnmaker::{VnMakerAnalyzer, VnMakerAssetAuditor};
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
    /// Manage and inspect overlay themes
    Theme {
        #[command(subcommand)]
        sub: ThemeCommands,
    },
    /// Inspect a game distribution without executing game code
    Inspect {
        /// Path to game source directory
        path: PathBuf,
        /// Optional config path
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Optional engine override
        #[arg(long)]
        engine: Option<String>,
    },
    /// Explain engine detection evidence and compatibility rationale
    Explain {
        /// Path to game source directory
        path: PathBuf,
        /// Optional engine override
        #[arg(long)]
        engine: Option<String>,
    },
    /// Audit game asset references without executing game code
    Audit(AuditArgs),
    /// Build standalone Android APK/AAB from a game
    Build(BuildArgs),
    /// Automated APK smoke test on Android emulator or connected device (T1)
    Probe(ProbeArgs),
    /// Display third-party notices and licenses
    Licenses,
    /// Package a portable distribution bundle according to the positive allowlist specification
    PackBundle(PackBundleArgs),
}

#[derive(clap::Args, Debug, Clone)]
pub struct PackBundleArgs {
    /// Destination directory for the packaged distribution bundle
    #[arg(short, long)]
    pub output: PathBuf,
    /// Optional path to CLI binary to bundle into bin/
    #[arg(short, long)]
    pub bin: Option<PathBuf>,
    /// Include prebuilt RGSS native libraries for LOCAL DIAGNOSTIC use only.
    ///
    /// The RGSS engine's buildable source is unverified in this tree, so these
    /// binaries are excluded from distribution bundles. Enabling this stamps the
    /// bundle with a LOCAL_DIAGNOSTIC_ONLY / NO_REMOTE_DISTRIBUTION marker and
    /// does not establish licensing compliance.
    #[arg(long)]
    pub local_diagnostic_native: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct ProbeArgs {
    /// Path to game APK to probe
    pub apk: Option<PathBuf>,
    /// Device ID from config or raw address (e.g. 127.0.0.1:16384)
    #[arg(long)]
    pub device: Option<String>,
    /// Number of DPAD down presses before Enter to select New Game
    #[arg(long, default_value = "0")]
    pub dpad_down: u32,
    /// Path to output probe report JSON
    #[arg(short, long, default_value = "dist/probe-report.json")]
    pub report: PathBuf,
    /// List discovered ADB devices and configured device profiles
    #[arg(long)]
    pub list_devices: bool,
    /// Path to device profiles config
    #[arg(long)]
    pub config: Option<PathBuf>,
}

#[derive(clap::Args, Debug, Clone)]
pub struct AuditArgs {
    /// Path to game source directory
    pub path: PathBuf,
    /// Path to RTP (Run-Time Package) directory to resolve missing standard assets
    #[arg(long)]
    pub rtp_path: Option<PathBuf>,
    /// Audit mode: standard or deep
    #[arg(long, default_value = "standard")]
    pub mode: String,
    /// Permit bounded checks of external network resources
    #[arg(long)]
    pub allow_network_audit: bool,
    /// Authoritative JSON report path
    #[arg(short, long, default_value = "asset-audit-report.json")]
    pub output: PathBuf,
    /// Do not emit standalone HTML report viewer
    #[arg(long)]
    pub no_html: bool,
}

#[derive(clap::Args, Debug, Clone)]
pub struct BuildArgs {
    /// Path to game source directory
    pub path: PathBuf,
    /// Path to RTP (Run-Time Package) directory to bundle missing standard assets
    #[arg(long)]
    pub rtp_path: Option<PathBuf>,
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
    #[arg(
        long,
        default_value = "full",
        help = "Overlay mode: full or passive-watermark"
    )]
    pub overlay_mode: String,
    #[arg(
        long,
        default_value = "off",
        help = "VNM dialogue advance input guard: off or transition-aware"
    )]
    pub vnm_input_guard: String,
    /// Static asset audit mode: standard or deep
    #[arg(long, default_value = "standard")]
    pub audit_mode: String,
    /// Themes to package into overlay (comma-separated or repeated, e.g. --themes matte,liquid_glass)
    #[arg(long, value_delimiter = ',')]
    pub themes: Vec<String>,
    /// Default active theme ID (defaults to 'matte' if available, or first in list)
    #[arg(long)]
    pub default_theme: Option<String>,
    /// Path to theme catalog directory (defaults to runtimes/themes)
    #[arg(long)]
    pub theme_catalog: Option<PathBuf>,
    /// Block build on FAIL/INDETERMINATE asset audit (default: report and continue)
    #[arg(long)]
    pub strict_audit: bool,
    /// Explicitly accept a static audit FAIL outcome, including in strict mode
    #[arg(long)]
    pub accept_audit_fail: bool,
    /// Accept an INDETERMINATE static audit outcome
    #[arg(long)]
    pub accept_audit_unknown: bool,
    /// Forward findings that static analysis could only classify as Unknown into
    /// an explicitly experimental build. This never clears a Blocked finding, a
    /// raw native code, a security/integrity failure, or a toolchain/path/signing
    /// gate, and it is per-invocation only: it is never read from configuration
    /// and never persisted as a project default.
    ///
    /// Combined with --strict-audit, an audit FAIL caused only by forwarded
    /// findings is treated as a WARNING for the gate decision; the recorded audit
    /// outcome stays FAIL, and the build stays labelled experimental.
    #[arg(long)]
    pub allow_unknown: bool,
    /// Packaging compatibility policy for unproven desktop-only web features:
    /// `warn` (default) continues with a prominent, recorded warning; `block`
    /// stops on the same fact.
    ///
    /// Only a `CMP-104` finding whose structurally valid trailing proof carrier
    /// says `unknown` or `required` can be accepted. Raw native codes, security
    /// and integrity failures, unsafe paths, malformed carriers and toolchain or
    /// runtime failures stay mandatory stops under every value. An accepted risk
    /// is a packaging decision, never a claim that the game works.
    #[arg(long, default_value = "warn")]
    pub compatibility_policy: String,
    /// Skip Gradle APK compilation (generates Android project and assets only)
    #[arg(long)]
    pub no_compile: bool,
    /// Do not emit standalone HTML report viewer
    #[arg(long)]
    pub no_html: bool,
    /// Output directory
    #[arg(short, long, default_value = "./dist")]
    pub output: PathBuf,
    /// Declarative compatibility profile JSON path for bounded staging transformation
    #[arg(long)]
    pub compat_profile: Option<PathBuf>,
    /// Force a specific engine family. The selection is validated against static
    /// evidence: a family without supporting evidence, or contradicted by live
    /// entrypoint evidence, is rejected with DET-101. This does not bypass
    /// genuine detection ambiguity (DET-110).
    #[arg(long)]
    pub engine: Option<String>,
    /// Accept a detected engine with medium confidence
    #[arg(long)]
    pub accept_detection: bool,
    /// Run T1 smoke probe on built APK (exits non-zero if probe fails; skips gracefully if no device available)
    #[arg(long)]
    pub probe: bool,
    /// Target ADB device address or profile ID for post-build probe
    #[arg(long)]
    pub probe_device: Option<String>,
    /// Keep intermediate build files (scratch workspace, staged assets, and android-project)
    #[arg(long, visible_alias = "keep")]
    pub keep_build_files: bool,
}

#[derive(Subcommand)]
enum RuntimeCommands {
    /// List all registered runtime packages
    List,
}

#[derive(Subcommand, Debug, Clone)]
enum ThemeCommands {
    /// List all discovered themes in catalog with validation status
    List {
        /// Optional path to theme catalog directory (defaults to runtimes/themes)
        #[arg(long)]
        catalog: Option<PathBuf>,
    },
    /// Validate a theme package directory
    Validate {
        /// Path to theme package directory
        path: PathBuf,
        /// Expected Theme API version (defaults to 1.0.0)
        #[arg(long, default_value = "1.0.0")]
        api_version: String,
    },
}

fn create_default_analyzer_registry() -> runtimepack_core::compatibility::AnalyzerRegistry {
    let mut registry = runtimepack_core::compatibility::AnalyzerRegistry::new();
    registry.register(Box::new(RgssAnalyzer));
    registry.register(Box::new(RpgMakerWebAnalyzer));
    registry.register(Box::new(RenPyAnalyzer));
    registry.register(Box::new(TyranoAnalyzer));
    registry.register(Box::new(VnMakerAnalyzer));
    registry
}

pub(crate) fn create_default_asset_auditor_registry() -> AssetAuditorRegistry {
    let mut registry = AssetAuditorRegistry::new();
    registry
        .register(Box::new(RgssAssetAuditor))
        .expect("RGSS asset auditor must register");
    registry
        .register(Box::new(RpgMakerWebAssetAuditor))
        .expect("RPG Maker Web asset auditor must register");
    registry
        .register(Box::new(RenPyAssetAuditor))
        .expect("Ren'Py asset auditor must register");
    registry
        .register(Box::new(TyranoAssetAuditor))
        .expect("Tyrano asset auditor must register");
    registry
        .register(Box::new(VnMakerAssetAuditor))
        .expect("Visual Novel Maker asset auditor must register");
    registry
}

fn cmd_doctor() {
    println!("RuntimePack Doctor — Environment Diagnostics\n");
    println!("System & Toolchains:");

    // Rust
    let rustc_res = Command::new("rustc")
        .arg("--version")
        .output()
        .or_else(|_| {
            if let Ok(profile) = std::env::var("USERPROFILE") {
                let p = PathBuf::from(profile).join(".cargo/bin/rustc.exe");
                Command::new(p).arg("--version").output()
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "not found",
                ))
            }
        });

    match rustc_res {
        Ok(out) if out.status.success() => {
            println!(
                "  [OK] Rust: {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
        _ => println!("  [WARN] Rust compiler (rustc) not found in active PATH"),
    }

    // Git
    let git_ok = Command::new("git").arg("--version").output();
    match git_ok {
        Ok(out) if out.status.success() => {
            println!(
                "  [OK] Git: {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
        _ => println!("  [WARN] Git not found"),
    }

    // Node.js
    let node_ok = Command::new("node").arg("--version").output();
    match node_ok {
        Ok(out) if out.status.success() => {
            println!(
                "  [OK] Node.js: {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
        _ => println!("  [INFO] Node.js not detected"),
    }

    // Java / JDK
    let java_res = Command::new("java").arg("-version").output().or_else(|_| {
        let java_bin_name = if cfg!(windows) {
            "bin/java.exe"
        } else {
            "bin/java"
        };
        if let Ok(jh) = std::env::var("JAVA_HOME") {
            let p = PathBuf::from(jh).join(java_bin_name);
            Command::new(p).arg("-version").output()
        } else if cfg!(windows) {
            if let Ok(profile) = std::env::var("USERPROFILE") {
                let p = PathBuf::from(profile)
                    .join(".runtimepack/toolchains/jdk-17")
                    .join(java_bin_name);
                Command::new(p).arg("-version").output()
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "not found",
                ))
            }
        } else if cfg!(unix) {
            if let Ok(home) = std::env::var("HOME") {
                let p = PathBuf::from(home)
                    .join(".runtimepack/toolchains/jdk-17")
                    .join(java_bin_name);
                Command::new(p).arg("-version").output()
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "not found",
                ))
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "not found",
            ))
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
            if cfg!(windows) {
                std::env::var("LOCALAPPDATA").ok().and_then(|la| {
                    let p = PathBuf::from(la).join("Android/Sdk");
                    if p.exists() {
                        Some(p.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            } else if cfg!(unix) {
                std::env::var("HOME").ok().and_then(|h| {
                    let p = PathBuf::from(h).join("Android/Sdk");
                    if p.exists() {
                        Some(p.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
            } else {
                None
            }
        });

    match android_home {
        Some(path) => println!("  [OK] Android SDK: {}", path),
        None => println!(
            "  [WARN] ANDROID_HOME / ANDROID_SDK_ROOT not set; required for packaging APKs"
        ),
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

    // Resource Root & Assets
    println!("\nResource Root & Distribution Assets:");
    let mut resources_ok = true;
    match runtimepack_core::resource_root::resolve_resource_root() {
        Ok(root) => {
            println!("  [OK] Resource Root:     {}", root.display());

            // Android Shell
            let shell_build = root.join("android/shell/build.gradle.kts");
            if shell_build.is_file() {
                println!("  [OK] Android Shell:     android/shell/build.gradle.kts (Gradle 8.7, AGP 8.3.0)");
            } else {
                println!("  [FAIL] Android Shell:   android/shell/build.gradle.kts missing");
                resources_ok = false;
            }

            // Runtime Manifests
            let manifests_dir = root.join("runtimes/manifests");
            let mut manifest_count = 0;
            if manifests_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&manifests_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map_or(false, |ext| ext == "toml") {
                            manifest_count += 1;
                        }
                    }
                }
            }
            if manifest_count > 0 {
                println!("  [OK] Runtime Manifests: {} registered (rgss, rpgmaker-web, renpy-7/8, tyrano, vnmaker)", manifest_count);
            } else {
                println!("  [FAIL] Runtime Manifests: runtimes/manifests/*.toml missing");
                resources_ok = false;
            }

            // Theme Profiles
            let themes_dir = root.join("runtimes/themes");
            let mut theme_count = 0;
            if themes_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&themes_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() && entry.path().join("theme.json").is_file() {
                            theme_count += 1;
                        }
                    }
                }
            }
            if theme_count > 0 {
                println!("  [OK] Theme Profiles:    {} installed", theme_count);
            } else {
                println!("  [FAIL] Theme Profiles:  runtimes/themes/* missing");
                resources_ok = false;
            }

            // Cheat Profiles
            let cheats_dir = root.join("profiles/games");
            let mut cheat_count = 0;
            if cheats_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&cheats_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() && entry.path().join("cheats.js").is_file() {
                            cheat_count += 1;
                        }
                    }
                }
            }
            if cheat_count > 0 {
                println!("  [OK] Cheat Profiles:    {} games registered", cheat_count);
            } else {
                println!("  [WARN] Cheat Profiles:  profiles/games/*/cheats.js missing");
            }

            // Compatibility DB
            let compat_dir = root.join("compatibility-db");
            let mut compat_count = 0;
            if compat_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&compat_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map_or(false, |ext| ext == "toml") {
                            compat_count += 1;
                        }
                    }
                }
            }
            if compat_count > 0 {
                println!(
                    "  [OK] Compatibility DB:  {} capability manifests loaded",
                    compat_count
                );
            } else {
                println!("  [FAIL] Compatibility DB: compatibility-db/*.toml missing");
                resources_ok = false;
            }

            // JSON Schemas
            let schemas_dir = root.join("schemas");
            let mut schema_count = 0;
            if schemas_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(&schemas_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map_or(false, |ext| ext == "json") {
                            schema_count += 1;
                        }
                    }
                }
            }
            if schema_count > 0 {
                println!(
                    "  [OK] JSON Schemas:      {} validation schemas intact",
                    schema_count
                );
            } else {
                println!("  [FAIL] JSON Schemas:    schemas/*.schema.json missing");
                resources_ok = false;
            }

            // Toolchain Lock
            let lock_file = root.join("toolchain.lock");
            if lock_file.is_file() {
                println!("  [OK] Toolchain Lock:    Pinned baseline verified");
            } else {
                println!("  [FAIL] Toolchain Lock:  toolchain.lock missing");
                resources_ok = false;
            }

            // Checksums Integrity
            let checksums_file = root.join("checksums.txt");
            if checksums_file.is_file() {
                match runtimepack_core::resource_root::verify_bundle_checksums(&root) {
                    Ok(count) => {
                        println!("  [OK] Checksums Integrity: {} entries verified (SHA-256 integrity record)", count);
                    }
                    Err(e) => {
                        println!("  [FAIL] Checksums Integrity: {}", e);
                        resources_ok = false;
                    }
                }
            } else if root.join("Cargo.toml").is_file() {
                println!(
                    "  [INFO] Checksums Integrity: NOT_AVAILABLE (source development checkout)"
                );
            } else {
                println!(
                    "  [WARN] Checksums Integrity: checksums.txt missing from distribution bundle"
                );
            }
        }
        Err(err) => {
            println!("  [FAIL] Resource Root:   {}", err);
            resources_ok = false;
        }
    }

    if !resources_ok {
        eprintln!("\nDiagnostics failed: one or more required distribution resources are missing or invalid.");
        std::process::exit(1);
    }

    println!("\nDiagnostics complete.");
}

fn cmd_runtime_list() {
    let registry = RuntimeRegistry::new();
    println!("Available Android Runtime Packages:\n");
    println!(
        "{:<20} {:<10} {:<12} {:<24} {:<8}",
        "ID", "VERSION", "FAMILY", "SUPPORTED ENGINES", "MIN SDK"
    );
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

fn cmd_inspect(path: &Path, _config_path: Option<&Path>, engine: Option<&str>) {
    println!("Inspecting: {}\n", path.display());

    let source_index = match GameSourceIndex::build(path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error reading source: {}", e);
            std::process::exit(1);
        }
    };

    println!(
        "Indexed {} files ({:.2} MB, sha256: {})",
        source_index.file_count(),
        source_index.total_size_bytes as f64 / (1024.0 * 1024.0),
        &source_index.source_hash[..8]
    );

    let detectors = runtimepack_cli::detection_registry::default_detector_registry();
    let mut detection_opts = runtimepack_core::detection::DetectionOptions::default();
    if let Some(eng_str) = engine {
        match eng_str.parse::<EngineId>() {
            Ok(eng_id) => detection_opts.forced_engine = Some(eng_id),
            Err(e) => {
                eprintln!("[DET-101] Invalid engine override: {}", e);
                std::process::exit(1);
            }
        }
    }
    let descriptor = detectors.resolve_with_options(&source_index, &detection_opts);

    let is_rgss = matches!(
        descriptor.id,
        EngineId::RpgMakerXp | EngineId::RpgMakerVx | EngineId::RpgMakerVxAce
    );
    let analyzers = create_default_analyzer_registry();
    let report = if is_rgss {
        let mut payload_index = if let Some(ref root) = descriptor.content_root {
            source_index
                .scoped_to(root)
                .unwrap_or_else(|_| source_index.clone())
        } else {
            source_index.clone()
        };
        let mut rtp_notice = None;
        match runtimepack_engine_rgss::resolve_rgss_rtp_dependency(&payload_index, None) {
            Ok(runtimepack_engine_rgss::ResolvedRtp::Resolved {
                path: resolved_rtp,
                source_description,
                ..
            }) => {
                println!(
                    "RTP Resolved:   {} ({})",
                    resolved_rtp.display(),
                    source_description
                );
                let _ = payload_index.mount_external(&resolved_rtp);
            }
            Ok(runtimepack_engine_rgss::ResolvedRtp::DeclaredUnresolved { rtp_name }) => {
                println!(
                    "RTP Notice:     '{}' declared in Game.ini (unresolved in catalog/env)",
                    sanitize_untrusted_line(&rtp_name, UNTRUSTED_LINE_LIMIT)
                );
                rtp_notice = Some(rtp_name);
            }
            Ok(runtimepack_engine_rgss::ResolvedRtp::NotDeclared) => {}
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        let mut rep = analyzers.analyze(&payload_index, &descriptor);
        if let Some(rtp_name) = rtp_notice {
            rep.add_issue(runtimepack_core::compatibility::CompatibilityIssue {
                code: runtimepack_core::diagnostics::DiagnosticCode::RGS_RTP_UNRESOLVED.to_string(),
                severity: runtimepack_core::diagnostics::DiagnosticSeverity::Warning,
                message: format!("RTP package '{}' declared in Game.ini could not be located in catalog or environment.", rtp_name),
                source_path: Some("Game.ini".to_string()),
                remediation: Some(runtimepack_core::diagnostics::RemediationCategory::Dependency),
            });
        }
        rep
    } else {
        analyzers.analyze(&source_index, &descriptor)
    };

    println!("Engine:         {}", descriptor.id);
    println!("Confidence:     {}", descriptor.confidence);
    println!("Score:          {}", descriptor.score);
    if let Some(ref root) = descriptor.content_root {
        println!("Content Root:   {}", sanitize_untrusted_line(root, UNTRUSTED_LINE_LIMIT));
    } else {
        println!("Content Root:   <root>");
    }
    println!("Status:         {}", report.status);
    if let Some(family) = descriptor.id.default_runtime_family() {
        println!("Runtime Family: {}", family);
    }

    if !report.required_capabilities.is_empty() {
        println!("\nCapabilities Required:");
        for cap in &report.required_capabilities {
            println!("  + {}", sanitize_untrusted_line(cap, UNTRUSTED_LINE_LIMIT));
        }
    }

    if !report.required_shims.is_empty() {
        println!("\nCompatibility Shims:");
        for shim in &report.required_shims {
            println!("  [SHIM] {}", sanitize_untrusted_line(shim, UNTRUSTED_LINE_LIMIT));
        }
    }

    if !report.issues.is_empty() {
        println!(
            "\nCompatibility Issues & Diagnostics ({}):",
            report.issues.len()
        );
        for issue in &report.issues {
            println!(
                "  [{}] [{}] {}",
                sanitize_untrusted_line(&issue.code, UNTRUSTED_LINE_LIMIT),
                issue.severity,
                sanitize_untrusted_line(&issue.message, UNTRUSTED_LINE_LIMIT)
            );
        }
    }

    if !descriptor.evidence.is_empty() {
        println!("\nEvidence:");
        for ev in &descriptor.evidence {
            println!(
                "  [{}] {} (+{})",
                sanitize_untrusted_line(&ev.rule, UNTRUSTED_LINE_LIMIT),
                sanitize_untrusted_line(&ev.path, UNTRUSTED_LINE_LIMIT),
                ev.weight
            );
        }
    }

    if !descriptor.conflicts.is_empty() {
        println!("\nConflicts:");
        for c in &descriptor.conflicts {
            println!("  - {}", sanitize_untrusted_line(c, UNTRUSTED_LINE_LIMIT));
        }
    }
}

fn cmd_explain(path: &Path, engine: Option<&str>) {
    println!("Engine Detection Rationale for: {}\n", path.display());

    let source_index = match GameSourceIndex::build(path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Error reading source: {}", e);
            std::process::exit(1);
        }
    };

    let detectors = runtimepack_cli::detection_registry::default_detector_registry();
    let candidates = detectors.detect_all(&source_index);

    let mut detection_opts = runtimepack_core::detection::DetectionOptions::default();
    if let Some(eng_str) = engine {
        match eng_str.parse::<EngineId>() {
            Ok(eng_id) => detection_opts.forced_engine = Some(eng_id),
            Err(e) => {
                eprintln!("[DET-101] Invalid engine override: {}", e);
                std::process::exit(1);
            }
        }
    }
    let resolved = detectors.resolve_with_options(&source_index, &detection_opts);

    println!("Evaluated Candidates:");
    let is_ambiguous_resolution = resolved.confidence == DetectionConfidence::Ambiguous;

    for c in &candidates {
        let root_info = match &c.content_root {
            Some(r) => format!(" (root: {})", r),
            None => " (root: <root>)".to_string(),
        };

        let is_selected = !is_ambiguous_resolution
            && c.engine == resolved.id
            && c.content_root == resolved.content_root
            && resolved.id != EngineId::Unknown;

        let is_eligible_or_conflicted = c.confidence.allows_auto_build()
            || c.confidence == DetectionConfidence::Medium
            || !c.conflicts.is_empty()
            || c.score >= 80;

        let (status_tag, rationale) = if is_ambiguous_resolution {
            if is_eligible_or_conflicted {
                (
                    " [AMBIGUOUS]",
                    Some("      * Conflict: Candidate root participates in unresolved ambiguity [DET-110]".to_string()),
                )
            } else {
                (
                    " [REJECTED]",
                    Some(format!(
                        "      * Rejection: Insufficient score or confidence ({}, score {}) to qualify for resolution",
                        c.confidence, c.score
                    )),
                )
            }
        } else if is_selected {
            (" [SELECTED]", None)
        } else {
            let reason = if c.engine != resolved.id {
                format!(
                    "      * Rejection: Engine does not match final resolution ({})",
                    resolved.id
                )
            } else if c.content_root != resolved.content_root {
                "      * Rejection: Root superseded by higher scoring/priority candidate"
                    .to_string()
            } else {
                "      * Rejection: Candidate superseded".to_string()
            };
            (" [REJECTED]", Some(reason))
        };

        println!(
            "  - Engine: {}{}{} | Score: {} | Confidence: {}",
            c.engine, root_info, status_tag, c.score, c.confidence
        );
        for ev in &c.evidence {
            println!(
                "      * {} -> {} (+{}): {}",
                ev.rule, ev.path, ev.weight, ev.description
            );
        }
        for conflict in &c.conflicts {
            println!("      ! Conflict: {}", conflict);
        }
        if let Some(msg) = rationale {
            println!("{}", msg);
        }
    }

    println!("\nFinal Resolution:");
    println!("  Selected Engine: {}", resolved.id);
    println!("  Confidence:      {}", resolved.confidence);
    println!("  Score:           {}", resolved.score);
    if let Some(ref root) = resolved.content_root {
        println!("  Content Root:    {}", root);
    } else {
        println!("  Content Root:    <root>");
    }

    if !resolved.conflicts.is_empty() {
        println!("  Conflicts:");
        for c in &resolved.conflicts {
            println!("    ! {}", c);
        }
    }

    if matches!(
        resolved.id,
        EngineId::RpgMakerXp | EngineId::RpgMakerVx | EngineId::RpgMakerVxAce
    ) {
        let scoped_index = if let Some(ref root) = resolved.content_root {
            source_index
                .scoped_to(root)
                .unwrap_or_else(|_| source_index.clone())
        } else {
            source_index.clone()
        };
        match runtimepack_engine_rgss::resolve_rgss_rtp_dependency(&scoped_index, None) {
            Ok(runtimepack_engine_rgss::ResolvedRtp::Resolved {
                path: rtp_path,
                source_description,
                ..
            }) => {
                println!(
                    "  RTP Status:      Resolved ({}) -> {}",
                    source_description,
                    rtp_path.display()
                );
            }
            Ok(runtimepack_engine_rgss::ResolvedRtp::DeclaredUnresolved { rtp_name }) => {
                println!(
                    "  RTP Status:      Declared '{}' (Unresolved in catalog/environment)",
                    rtp_name
                );
            }
            Ok(runtimepack_engine_rgss::ResolvedRtp::NotDeclared) => {
                println!("  RTP Status:      None declared");
            }
            Err(e) => {
                println!("  RTP Status:      Error ({})", e);
            }
        }
    }

    let analyzers = create_default_analyzer_registry();
    let report = analyzers.analyze(&source_index, &resolved);
    println!("  Status:          {}", report.status);

    if !report.required_shims.is_empty() {
        println!("  Shims Required:  {}", report.required_shims.join(", "));
    }
}

fn cmd_audit(args: AuditArgs) {
    let request = audit_command::AuditRequest {
        source: args.path,
        rtp_path: args.rtp_path,
        mode: args.mode,
        allow_network: args.allow_network_audit,
        output: args.output,
        emit_html: !args.no_html,
    };
    match audit_command::run(request, &audit_command::StaticAuditExecutionContext) {
        Ok(summary) => {
            println!("Detected engine: {}", summary.engine);
            println!("Audit mode: {:?}", summary.mode);
            if let Some(ref rtp) = summary.rtp_path {
                println!("RTP path: {}", rtp.display());
            }
            println!(
                "Network audit: {}",
                if summary.allow_network {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!("Outcome: {:?}", summary.outcome);
            println!("Report: {}", summary.output.display());
        }
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    }
}

/// Build-time post-pack integrity gate for ASAR-sourced content.
///
/// Reads the original archive independently of the extractor (re-deriving the
/// data offset straight from the raw header) and compares a deterministic
/// sample of entries against their packed counterparts by SHA-256. This
/// catches the historical class of bug where an ASAR header-padding
/// miscalculation silently shifted every extracted byte before it was packed.
fn verify_asar_pack_roundtrip(
    asar_path: &Path,
    archive: &runtimepack_engine_tyrano::AsarArchive,
    pack_path: &Path,
) -> Result<usize, String> {
    use std::io::{Read, Seek, SeekFrom};

    const MAX_SAMPLES: usize = 32;
    const MAX_SAMPLED_ENTRY_BYTES: u64 = 8 * 1024 * 1024;

    // Derive the data offset directly from the raw header so the gate never
    // shares the extractor's offset computation (regression guard).
    let mut asar_file = fs::File::open(asar_path)
        .map_err(|e| format!("cannot open archive '{}': {}", asar_path.display(), e))?;
    let mut header = [0u8; 16];
    asar_file
        .read_exact(&mut header)
        .map_err(|e| format!("cannot read archive header: {}", e))?;
    let header_size = u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64;
    let data_offset = 8 + header_size;

    let mut reader = runtimepack_pack::GpackReader::open(pack_path)
        .map_err(|e| format!("cannot open game.pack: {}", e))?;

    let mut candidate_pairs: Vec<(&runtimepack_engine_tyrano::asar::AsarFileEntry, String)> =
        Vec::new();
    for entry in archive.files.values() {
        if reader.has_file(&entry.path) {
            candidate_pairs.push((entry, entry.path.clone()));
        } else if let Some((_, rest)) = entry.path.split_once('/') {
            if reader.has_file(rest) {
                candidate_pairs.push((entry, rest.to_string()));
            }
        }
    }

    if candidate_pairs.is_empty() {
        // Returning Ok(0) lets the caller fail closed: an ASAR with entries
        // that map to no pack entry means the gate cannot vouch for anything.
        return Ok(0);
    }
    candidate_pairs.sort_by(|a, b| a.0.path.cmp(&b.0.path));

    // Prefer smaller entries so the gate stays cheap while remaining
    // deterministic and representative of every region of the archive.
    let sampled: Vec<(&runtimepack_engine_tyrano::asar::AsarFileEntry, String)> = candidate_pairs
        .iter()
        .filter(|(entry, _)| entry.size <= MAX_SAMPLED_ENTRY_BYTES)
        .cloned()
        .collect();
    let pool = if sampled.is_empty() {
        candidate_pairs
    } else {
        sampled
    };

    let stride = pool.len().div_ceil(MAX_SAMPLES);
    let mut verified = 0usize;
    for (entry, target_path) in pool.iter().step_by(stride.max(1)) {
        let mut source_bytes = vec![0u8; entry.size as usize];
        asar_file
            .seek(SeekFrom::Start(data_offset + entry.offset))
            .map_err(|e| format!("cannot seek to '{}': {}", entry.path, e))?;
        asar_file
            .read_exact(&mut source_bytes)
            .map_err(|e| format!("cannot read source entry '{}': {}", entry.path, e))?;
        let source_hash: [u8; 32] = Sha256::digest(&source_bytes).into();

        let packed_bytes = reader
            .read_file(target_path)
            .map_err(|e| format!("cannot read packed entry '{}': {}", target_path, e))?;
        let packed_hash: [u8; 32] = Sha256::digest(&packed_bytes).into();

        if source_hash != packed_hash {
            return Err(format!(
                "entry '{}' shifted or corrupted (source sha256={}, packed sha256={})",
                entry.path,
                hex_digest(&source_hash),
                hex_digest(&packed_hash)
            ));
        }
        verified += 1;
    }

    Ok(verified)
}

fn hex_digest(bytes: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{:02x}", byte));
    }
    out
}

/// Locates the Ren'Py launcher that RAPT renames to `main.py`.
///
/// RAPT's `split_renpy` renames every top-level `*.py` file (skipping
/// `android-*` entries) to `main.py`, which the Android runtime then executes
/// from `<ANDROID_PRIVATE>/main.py`. RuntimePack reproduces that alias inside
/// the content container so the immutable source tree is never touched.
fn find_renpy_root_launcher(index: &GameSourceIndex) -> Result<Option<String>, Vec<String>> {
    runtimepack_engine_renpy::find_renpy_root_launcher(index)
}

/// Picks a runtime id to prefer for engine resolution, derived from the
/// compatibility report in an engine-agnostic way: if an analyzer emitted an
/// informational finding that names one of the candidate runtime ids (e.g.
/// Ren'Py's `RNP-100-VERSION` maps a detected generation to
/// `renpy-android-8`), that id is preferred. Returns `None` when no candidate
/// is named, in which case the registry's deterministic version ordering wins.
fn preferred_runtime_id_from_report(
    report: &runtimepack_core::compatibility::CompatibilityReport,
    candidates: &[&runtimepack_core::runtime_registry::RuntimeManifest],
) -> Option<String> {
    report.issues.iter().find_map(|issue| {
        candidates
            .iter()
            .find(|candidate| issue.message.contains(&candidate.id))
            .map(|candidate| candidate.id.clone())
    })
}

fn cmd_build(args: BuildArgs) {
    let normalized_flavor = args.flavor.to_lowercase();
    if normalized_flavor != "beta" && normalized_flavor != "release" {
        eprintln!(
            "[BLD-101] Invalid build flavor '{}'. Must be 'beta' or 'release'.",
            args.flavor
        );
        std::process::exit(1);
    }
    let normalized_overlay_mode = args.overlay_mode.to_lowercase();
    if normalized_overlay_mode != "full" && normalized_overlay_mode != "passive-watermark" {
        eprintln!(
            "[BLD-101] Invalid overlay mode '{}'. Must be 'full' or 'passive-watermark'.",
            args.overlay_mode
        );
        std::process::exit(1);
    }
    let normalized_vnm_input_guard = args.vnm_input_guard.to_lowercase();
    if normalized_vnm_input_guard != "off" && normalized_vnm_input_guard != "transition-aware" {
        eprintln!(
            "[BLD-101] Invalid vnm-input-guard '{}'. Must be 'off' or 'transition-aware'.",
            args.vnm_input_guard
        );
        std::process::exit(1);
    }
    let audit_mode = match args.audit_mode.to_ascii_lowercase().as_str() {
        "standard" => AuditMode::Standard,
        "deep" => AuditMode::Deep,
        _ => {
            eprintln!(
                "[CMP-AUD-010] Invalid audit mode '{}'. Must be 'standard' or 'deep'.",
                args.audit_mode
            );
            std::process::exit(1);
        }
    };
    // The packaging compatibility policy is validated before anything is read,
    // created or packed, so a rejected invocation costs nothing.
    let compatibility_policy = match args.compatibility_policy.to_ascii_lowercase().as_str() {
        "warn" => CompatibilityBuildPolicy::Warn,
        "block" => CompatibilityBuildPolicy::Block,
        _ => {
            eprintln!(
                "[{}] Invalid compatibility policy '{}'. Must be 'warn' or 'block'.",
                DiagnosticCode::BLD_COMPATIBILITY_POLICY_INVALID,
                args.compatibility_policy
            );
            std::process::exit(1);
        }
    };
    // The two flags ask for contradictory things: `block` stops on the same facts
    // `--allow-unknown` forwards, so one of the two would be silently ignored.
    if compatibility_policy == CompatibilityBuildPolicy::Block && args.allow_unknown {
        eprintln!(
            "[{}] --compatibility-policy block conflicts with --allow-unknown: the first stops \
             on unproven compatibility facts while the second forwards them. Choose one.",
            DiagnosticCode::BLD_COMPATIBILITY_POLICY_CONFLICT
        );
        std::process::exit(1);
    }

    let path = &args.path;
    let rtp_path = args.rtp_path.as_deref();
    let package = args.package;
    let name = args.name;
    let content_mode = args.content_mode;
    let output = &args.output;

    println!("RuntimePack Build — Starting build pipeline\n");
    println!("Source game:   {}", path.display());
    if let Some(rtp) = rtp_path {
        println!("RTP path:      {}", rtp.display());
    }
    println!("Build flavor:  {}", normalized_flavor);
    println!("Output target: {}", output.display());

    // Validate output directory is bidirectionally disjoint from immutable source game root BEFORE creating directory
    if let Err(e) = runtimepack_core::build_plan::validate_disjoint_output_dir(path, output) {
        eprintln!("{e}");
        std::process::exit(1);
    }

    // Refuse to overwrite or modify an output directory that already contains a
    // published notice from a previous build (F2). RuntimePack's release integrity
    // contract mandates never silently overwriting stale deliverables, regardless
    // of whether the new invocation runs in warn, experimental, or default mode,
    // and regardless of compile/staging/no-compile paths.
    //
    // Failing closed here — before scratch creation, staging, or toolchain resolution —
    // guarantees that prior deliverables survive byte-identical and no intermediate
    // scratch workspace is created in the target directory.
    let durable_notice = output.join("EXPERIMENTAL-NOTICE.txt");
    if durable_notice.symlink_metadata().is_ok() {
        eprintln!(
            "[{}] '{}' records a previous build, which conflicts with this build. Use a new output directory; existing artifacts were preserved.",
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            durable_notice.display()
        );
        std::process::exit(1);
    }

    // Toolchain preflight runs after cheap argument/detection/audit-policy
    // validation (so those keep their documented precedence) but still before
    // any output directory, staging area or generated project is created, so a
    // missing SDK/JDK costs the operator nothing but a diagnostic.
    // `--no-compile` (and RUNTIMEPACK_SKIP_GRADLE) never compiles, so it never
    // preflights — previous behaviour is retained.
    let will_compile = !args.no_compile && std::env::var("RUNTIMEPACK_SKIP_GRADLE").is_err();
    let mut early_toolchain: Option<gradle::AndroidToolchain> = None;

    // 1. Ingest
    let source_index = match GameSourceIndex::build(path) {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("[ING-101] Failed to ingest source: {}", e);
            std::process::exit(1);
        }
    };
    println!(
        "Indexed {} files ({:.2} MB, source_hash: {})",
        source_index.file_count(),
        source_index.total_size_bytes as f64 / (1024.0 * 1024.0),
        &source_index.source_hash[..8]
    );

    // 2. Detect
    let detectors = runtimepack_cli::detection_registry::default_detector_registry();
    let mut detection_opts = runtimepack_core::detection::DetectionOptions::default();
    if let Some(ref eng_str) = args.engine {
        match eng_str.parse::<EngineId>() {
            Ok(eng_id) => detection_opts.forced_engine = Some(eng_id),
            Err(e) => {
                eprintln!("[DET-101] Invalid engine override: {}", e);
                std::process::exit(1);
            }
        }
    }
    detection_opts.accept_detection = args.accept_detection;
    let descriptor = detectors.resolve_with_options(&source_index, &detection_opts);

    // A rejected explicit engine selection is reported with its structured
    // DET-101 marker, never as generic ambiguity or an unknown engine.
    if let Some(rejection) = runtimepack_core::detection::engine_override_rejection(&descriptor) {
        eprintln!(
            "[{}] Forced engine override rejected: {}",
            runtimepack_core::diagnostics::DiagnosticCode::DET_INVALID_OVERRIDE,
            rejection.description
        );
        for conflict in &descriptor.conflicts {
            eprintln!("  ! {}", conflict);
        }
        std::process::exit(1);
    }

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
    if let Err(code) = runtimepack_core::detection::can_proceed_to_build(
        descriptor.confidence,
        args.accept_detection,
    ) {
        match code {
            "DET-102" => {
                eprintln!(
                    "[{}] Detection confidence is MEDIUM (score {}). Automatic build requires --accept-detection flag.",
                    code, descriptor.score
                );
            }
            "DET-103" => {
                eprintln!(
                    "[{}] Detection failed: Engine confidence is {} (score {}). Builds cannot proceed with low or none confidence.",
                    code, descriptor.confidence, descriptor.score
                );
            }
            _ => {
                eprintln!(
                    "[{}] Detection confidence {} does not permit building.",
                    code, descriptor.confidence
                );
            }
        }
        std::process::exit(1);
    }

    let is_rgss = matches!(
        descriptor.id,
        EngineId::RpgMakerXp | EngineId::RpgMakerVx | EngineId::RpgMakerVxAce
    );
    if rtp_path.is_some() && !is_rgss {
        eprintln!(
            "[{}] Explicit --rtp-path option is unsupported for engine '{}'. RTP assets can only be mounted for RGSS engines.",
            DiagnosticCode::BLD_OPTION_UNSUPPORTED_FOR_ENGINE,
            descriptor.id
        );
        std::process::exit(1);
    }

    println!(
        "Detected:      {} (Confidence: {})",
        descriptor.id, descriptor.confidence
    );
    if let Some(ref root) = descriptor.content_root {
        println!("Content Root:  {}", root);
    } else {
        println!("Content Root:  <root>");
    }

    if normalized_overlay_mode == "passive-watermark" {
        if normalized_flavor != "beta" {
            eprintln!(
                "[BLD-101] Overlay mode 'passive-watermark' is only supported for beta flavor builds."
            );
            std::process::exit(1);
        }
        if descriptor.id != EngineId::VisualNovelMaker {
            eprintln!(
                "[BLD-101] Overlay mode 'passive-watermark' is only supported for Visual Novel Maker engine."
            );
            std::process::exit(1);
        }
    }

    if normalized_vnm_input_guard == "transition-aware" {
        if normalized_flavor != "beta" {
            eprintln!(
                "[BLD-101] VNM input guard 'transition-aware' is only supported for beta flavor builds."
            );
            std::process::exit(1);
        }
        if descriptor.id != EngineId::VisualNovelMaker {
            eprintln!(
                "[BLD-101] VNM input guard 'transition-aware' is only supported for Visual Novel Maker engine."
            );
            std::process::exit(1);
        }
    }

    // Ren'Py Version Resolution Check before staging and output creation
    if descriptor.id == EngineId::RenPy {
        let unverified_payload = if let Some(ref root) = descriptor.content_root {
            source_index.scoped_to(root).ok()
        } else {
            Some(source_index.clone())
        };
        let supported_version = unverified_payload
            .as_ref()
            .and_then(|idx| runtimepack_engine_renpy::extract_static_renpy_version(idx))
            .filter(|v| v.major == 7 || v.major == 8);
        if supported_version.is_none() {
            eprintln!(
                "[RNP-110] Ren'Py runtime generation could not be strictly resolved to a supported runtime family (7 or 8) from static metadata in candidate distribution root. Cannot determine compatible Android runtime backend."
            );
            std::process::exit(1);
        }
    }

    // In compile mode with compatibility profile, initialize owned scratch directory early for staging
    let mut scratch_ws = if !args.no_compile
        && std::env::var("RUNTIMEPACK_SKIP_GRADLE").is_err()
        && args.compat_profile.is_some()
    {
        match runtimepack_core::build_plan::scratch::WorkspaceScratch::create(output) {
            Ok(ws) => Some(ws),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    // Staging & Compatibility Profile
    let (effective_source_index, applied_profile_id, applied_profile_caps) = if let Some(
        ref prof_path,
    ) =
        args.compat_profile
    {
        let profile =
            match runtimepack_core::compatibility::profile::CompatibilityProfile::load_from_file(
                prof_path,
            ) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!(
                        "[CMP-104] Failed to load compatibility profile '{}': {}",
                        prof_path.display(),
                        e
                    );
                    std::process::exit(1);
                }
            };

        println!(
            "Compat Profile: {} ({})",
            profile.profile_id,
            prof_path.display()
        );

        let caps = profile.collected_capabilities();
        let staging_res = if let Some(ref mut ws) = scratch_ws {
            match ws.register_owned_dir("staging") {
                Ok(staging_dir) => {
                    runtimepack_core::ingest::staging::stage_and_apply_profile_with_target(
                        &source_index,
                        descriptor.content_root.as_deref(),
                        output,
                        &staging_dir,
                        &profile,
                    )
                }
                Err(e) => Err(e),
            }
        } else {
            runtimepack_core::ingest::staging::stage_and_apply_profile(
                &source_index,
                descriptor.content_root.as_deref(),
                output,
                &profile,
            )
        };

        match staging_res {
            Ok(res) => {
                println!(
                    "Staged {} files in '{}' (staged content hash: {})",
                    res.staged_index.file_count(),
                    res.staging_path.display(),
                    &res.staged_index.source_hash[..8]
                );
                (res.staged_index, Some(profile.profile_id.clone()), caps)
            }
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        (source_index.clone(), None, Vec::new())
    };

    // 3. Compatibility & Legal Gate
    let analyzers = create_default_analyzer_registry();
    let mut report = analyzers.analyze(&effective_source_index, &descriptor);

    if let Some(ref profile_id) = applied_profile_id {
        report.add_issue(runtimepack_core::compatibility::CompatibilityIssue {
            code: DiagnosticCode::CMP_COMPAT_PATCH_APPLIED.to_string(),
            severity: DiagnosticSeverity::Warning,
            message: format!(
                "Declarative compatibility profile '{}' applied to staged content",
                profile_id
            ),
            source_path: None,
            remediation: None,
        });
        if report.status == CompatibilityStatus::Compatible
            || report.status == CompatibilityStatus::CompatibleWithShims
        {
            report.status = CompatibilityStatus::Degraded;
        }
    }

    // 4. Static Asset Audit and Runtime Resolution
    let mut hard_blockers: Vec<Diagnostic> = Vec::new();
    let mut payload_index = if let Some(ref root) = descriptor.content_root {
        match effective_source_index.scoped_to(root) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("[ING-101] Failed to scope content root '{}': {}", root, e);
                std::process::exit(1);
            }
        }
    } else {
        effective_source_index.clone()
    };

    if is_rgss {
        match runtimepack_engine_rgss::resolve_rgss_rtp_dependency(&payload_index, rtp_path) {
            Ok(runtimepack_engine_rgss::ResolvedRtp::Resolved {
                path: resolved_rtp,
                source_description,
                ..
            }) => {
                println!(
                    "RTP source:    {} ({})",
                    resolved_rtp.display(),
                    source_description
                );
                if let Err(e) = payload_index.mount_external(&resolved_rtp) {
                    eprintln!("[ING-101] Failed to mount external RTP directory: {}", e);
                    std::process::exit(1);
                }
            }
            Ok(runtimepack_engine_rgss::ResolvedRtp::DeclaredUnresolved { rtp_name }) => {
                println!("RTP dependency: '{}' declared in Game.ini was not found in catalog or environment", rtp_name);
                report.add_issue(runtimepack_core::compatibility::CompatibilityIssue {
                    code: runtimepack_core::diagnostics::DiagnosticCode::RGS_RTP_UNRESOLVED.to_string(),
                    severity: runtimepack_core::diagnostics::DiagnosticSeverity::Warning,
                    message: format!("RTP package '{}' declared in Game.ini could not be located in catalog or environment.", rtp_name),
                    source_path: Some("Game.ini".to_string()),
                    remediation: Some(runtimepack_core::diagnostics::RemediationCategory::Dependency),
                });
            }
            Ok(runtimepack_engine_rgss::ResolvedRtp::NotDeclared) => {}
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    }

    let audit_options = AssetAuditOptions {
        mode: audit_mode,
        allow_network: false,
        ..AssetAuditOptions::default()
    };
    let audit_cache_key = match AuditCacheKey::new(
        payload_index.scoped_content_hash(),
        descriptor.id,
        audit_mode,
        CURRENT_AUDIT_RULESET_VERSION,
        env!("CARGO_PKG_VERSION"),
    )
    .with_limits(audit_options.limits.clone())
    .with_allow_network(false)
    .with_detected_version(descriptor.detected_version.clone())
    .with_content_root(descriptor.content_root.clone())
    {
        Ok(k) => k,
        Err(e) => {
            eprintln!("[CNT-103] Failed to set content root for audit cache: {e}");
            std::process::exit(1);
        }
    };

    let is_web = matches!(descriptor.id, EngineId::RpgMakerMv | EngineId::RpgMakerMz);
    let audit_source = if is_web {
        &effective_source_index
    } else {
        &payload_index
    };

    let audit_cache = AuditCache::new(AuditCache::default_dir());
    let cache_usable = audit_cache
        .validate_outside_source(&source_index.root_path)
        .is_ok();
    let cache_enabled = cache_usable && !is_web;

    let (mut audit_report, from_cache) = if cache_enabled {
        if let Some(cached_artifacts) = audit_cache.get(&audit_cache_key) {
            match serde_json::from_slice::<AssetAuditReport>(&cached_artifacts.report_json) {
                Ok(report) => (report, true),
                Err(_) => {
                    let _ = audit_cache.invalidate(&audit_cache_key);
                    let report = create_default_asset_auditor_registry().audit(
                        audit_source,
                        &descriptor,
                        &audit_options,
                    );
                    (report, false)
                }
            }
        } else {
            let report = create_default_asset_auditor_registry().audit(
                audit_source,
                &descriptor,
                &audit_options,
            );
            (report, false)
        }
    } else {
        let report = create_default_asset_auditor_registry().audit(
            audit_source,
            &descriptor,
            &audit_options,
        );
        (report, false)
    };
    let registry = RuntimeRegistry::new();
    let runtimes = registry.find_for_engine(descriptor.id);
    if runtimes.is_empty() {
        hard_blockers.push(Diagnostic::error(
            DiagnosticCode::RT_UNRESOLVED,
            format!("no compatible runtime package found for {}", descriptor.id),
        ));
    }
    let audit_policy = if args.strict_audit {
        AuditBuildPolicy::Strict
    } else {
        AuditBuildPolicy::Informational
    };
    let build_policy = if args.allow_unknown {
        BuildPolicy::Experimental
    } else {
        BuildPolicy::Default
    };
    let typed_decision = evaluate_build_policy_with_compatibility(
        &report,
        &audit_report,
        audit_policy,
        build_policy,
        compatibility_policy,
        args.accept_audit_fail,
        args.accept_audit_unknown,
        &hard_blockers,
    );
    let audit_overrides = match typed_decision.decision.clone() {
        AuditPolicyDecision::Allow { overrides } => overrides,
        AuditPolicyDecision::Block { diagnostics } => {
            for diagnostic in &diagnostics {
                eprintln!(
                    "[{}] {}",
                    sanitize_untrusted_line(&diagnostic.code.0, UNTRUSTED_LINE_LIMIT),
                    sanitize_untrusted_line(&diagnostic.message, UNTRUSTED_LINE_LIMIT)
                );
            }
            if !audit_report.findings.is_empty() {
                eprintln!("\nAsset Audit Findings ({}):", audit_report.findings.len());
                for finding in &audit_report.findings {
                    eprintln!(
                        "  [{}] [{:?}] {}",
                        sanitize_untrusted_line(&finding.code, UNTRUSTED_LINE_LIMIT),
                        finding.severity,
                        sanitize_untrusted_line(&finding.message, UNTRUSTED_LINE_LIMIT)
                    );
                }
            }
            if diagnostics.iter().all(|d| d.code.0 == DiagnosticCode::BLD_AUDIT_POLICY_BLOCKED) {
                match audit_report.outcome {
                    AuditOutcome::Fail => {
                        eprintln!(
                            "\nSuggested remediation: Address missing or corrupted assets, or pass --accept-audit-fail to override."
                        );
                    }
                    AuditOutcome::Indeterminate => {
                        eprintln!(
                            "\nSuggested remediation: Pass --accept-audit-unknown to accept incomplete or unknown static asset audit coverage."
                        );
                    }
                    _ => {}
                }
            } else {
                eprintln!("\nHard blockers cannot be bypassed with audit override flags or --allow-unknown.");
            }
            std::process::exit(1);
        }
    };
    // Unknown findings must not claim compatibility even when optional under Default,
    // and an accepted compatibility risk is never a compatible report.
    report.status = typed_decision.compatibility_status(report.status);
    // Rendered once so the APK asset, the durable copy and the manifest digest are
    // guaranteed to describe the same bytes. Any accepted risk — forwarded unknowns
    // or compatibility warnings — must be disclosed.
    let accepted_risk = typed_decision.carries_accepted_risk();
    let experimental_notice: Option<String> = if accepted_risk {
        Some(render_experimental_notice(&typed_decision))
    } else {
        None
    };
    if typed_decision.is_experimental() {
        eprintln!(
            "[EXPERIMENTAL] Build policy: experimental (--allow-unknown). {} unknown finding(s) forwarded; this build is NOT device-validated.",
            typed_decision.forwarded.len()
        );
        for finding in &typed_decision.forwarded {
            eprintln!(
                "  [{}] {} — {}",
                sanitize_untrusted_line(&finding.code, UNTRUSTED_LINE_LIMIT),
                sanitize_untrusted_line(
                    finding.source_path.as_deref().unwrap_or("<unknown site>"),
                    UNTRUSTED_LINE_LIMIT
                ),
                sanitize_untrusted_line(&finding.message, UNTRUSTED_LINE_LIMIT)
            );
        }
    }
    // Compatibility warnings are a separate, clearly labelled disclosure: the
    // analysis fact keeps its original class and nothing here claims the game works.
    if !typed_decision.compatibility_warnings.is_empty() {
        eprintln!(
            "[COMPATIBILITY WARNING] Policy: {} ({} accepted risk(s); runtime validation NOT_RUN). \
             These desktop-only features are NOT proven safe and this build is NOT device-validated.",
            typed_decision.compatibility_policy.label(),
            typed_decision.compatibility_warnings.len()
        );
        for warning in &typed_decision.compatibility_warnings {
            eprintln!(
                "  [{}] state={} class={} requiredness={} {} — {}",
                sanitize_untrusted_line(&warning.finding.code, UNTRUSTED_LINE_LIMIT),
                warning.state.label(),
                warning.finding.class.label(),
                warning.finding.requiredness.label(),
                sanitize_untrusted_line(
                    warning.finding.source_path.as_deref().unwrap_or("<unknown site>"),
                    UNTRUSTED_LINE_LIMIT
                ),
                sanitize_untrusted_line(&warning.finding.message, UNTRUSTED_LINE_LIMIT)
            );
        }
        eprintln!(
            "  Disclosure: {}/EXPERIMENTAL-NOTICE.txt (packaged in APK assets when an APK is produced).",
            output.display()
        );
    }
    if !args.strict_audit && matches!(audit_report.outcome, AuditOutcome::Fail | AuditOutcome::Indeterminate) {
        eprintln!("[AUDIT WARNING] Asset audit {:?}; continuing with informational policy. Missing assets may fail during gameplay. Use --strict-audit to enforce audit rejection.", audit_report.outcome);
        for finding in &audit_report.findings {
            eprintln!(
                "  [{}] [{:?}] {}",
                sanitize_untrusted_line(&finding.code, UNTRUSTED_LINE_LIMIT),
                finding.severity,
                sanitize_untrusted_line(&finding.message, UNTRUSTED_LINE_LIMIT)
            );
        }
    }
    // Put pristine report into audit_cache before attaching ephemeral build policy overrides
    if cache_enabled && !from_cache {
        if let Ok(clean_artifacts) = render_canonical_artifacts(&audit_report) {
            let _ = audit_cache.put(&audit_cache_key, &clean_artifacts);
        }
    }

    audit_report.overrides = audit_overrides
        .iter()
        .map(|record| runtimepack_core::asset_audit::AuditOverride {
            flag: record.flag.clone(),
            rationale: record.rationale.clone(),
        })
        .collect();
    let audit_artifacts = match render_canonical_artifacts(&audit_report) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            eprintln!("[CMP-AUD-010] Failed to render canonical asset audit report: {error}");
            std::process::exit(1);
        }
    };
    let audit_hash = audit_artifacts.report_sha256.clone();
    let preferred_runtime_id = preferred_runtime_id_from_report(&report, &runtimes);
    let resolved_runtimes =
        registry.resolve_for_engine(descriptor.id, preferred_runtime_id.as_deref());
    let runtime = if descriptor.id == EngineId::RenPy {
        match resolved_runtimes.first().copied() {
            Some(rt) if preferred_runtime_id.is_some() => rt,
            _ => {
                eprintln!(
                    "[RNP-110] Ren'Py runtime generation could not be strictly resolved. Cannot determine compatible Android runtime backend."
                );
                std::process::exit(1);
            }
        }
    } else {
        resolved_runtimes.first().copied().unwrap_or(runtimes[0])
    };
    let cache_tag = if from_cache { " [cache hit]" } else { "" };
    println!(
        "Audit:         {:?} ({:?}){}",
        audit_report.outcome, audit_mode, cache_tag
    );
    println!(
        "Resolved:      {} v{} ({})",
        runtime.name, runtime.version, runtime.id
    );

    // 5. App details
    let detected_title = if matches!(descriptor.id, EngineId::RpgMakerMv | EngineId::RpgMakerMz) {
        runtimepack_engine_rpgmaker_web::extract_game_title(&payload_index)
    } else {
        None
    };

    let app_name = name.or(detected_title).unwrap_or_else(|| {
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

    let sanitized_apk_name = derive_apk_filename(&app_name, output);

    // Early collision check: prevent build execution if target APK already exists
    if !args.no_compile && std::env::var("RUNTIMEPACK_SKIP_GRADLE").is_err() {
        let final_dest = output.join(&sanitized_apk_name);
        if final_dest.exists() {
            eprintln!(
                "[{}] Target APK already exists at '{}'. Preserving existing deliverable without overwrite.",
                DiagnosticCode::BLD_ARTIFACT_COLLISION,
                final_dest.display()
            );
            if let Some(ws) = scratch_ws.as_ref() {
                println!("Workspace retained: {}", ws.scratch_root().display());
            }
            std::process::exit(1);
        }
    }

    if will_compile {
        // A retained project from a previous --keep-build-files run may carry a
        // local.properties that Gradle would actually read; honour it here.
        let retained_project = output.join("android-project");
        let retained = retained_project.is_dir().then_some(retained_project.as_path());
        let resolved = gradle::resolve_android_toolchain(retained);
        if let Err(diag) = gradle::validate_android_toolchain(&resolved, retained) {
            eprintln!("{}", diag);
            std::process::exit(1);
        }
        early_toolchain = Some(resolved);
        // Report the pin source once, so the operator knows which requirements
        // were enforced (the shipped toolchain.lock or built-in defaults).
        let pins = gradle::resolve_toolchain_pins_from_resource_root();
        println!(
            "Toolchain pins: compileSdk {}, build-tools {}, JDK {}+ [{}]",
            pins.lock.android_compile_sdk,
            pins.lock.build_tools_version,
            pins.lock.jdk_min_version,
            pins.source.label()
        );
    }

    if let Err(e) = fs::create_dir_all(output) {
        eprintln!("[BLD-101] Failed to create output directory: {}", e);
        std::process::exit(1);
    }

    if !args.no_compile && std::env::var("RUNTIMEPACK_SKIP_GRADLE").is_err() && scratch_ws.is_none()
    {
        scratch_ws = match runtimepack_core::build_plan::scratch::WorkspaceScratch::create(output) {
            Ok(ws) => Some(ws),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };
    }

    // Write audit artifacts early into output directory so project generator packages them
    if let Err(error) = write_audit_artifacts_with_options(output, &audit_artifacts, !args.no_html)
    {
        eprintln!("[CMP-AUD-010] Failed to write asset audit artifacts: {error}");
        std::process::exit(1);
    }

    // 6. Content Packing (GPACK)
    println!("\nPacking game content (mode: {})...", content_mode);
    let pack_path = if let Some(ref mut ws) = scratch_ws {
        match ws.register_owned_file("game.pack") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        output.join("game.pack")
    };

    let encryption_key = if content_mode == "encrypted-streaming" {
        Some(runtimepack_pack::ContentCrypto::derive_package_key(
            &pkg_id, &app_name,
        ))
    } else {
        None
    };

    let mut content_source_dir = path.to_path_buf();
    let mut staging_dir_to_cleanup: Option<PathBuf> = None;
    let mut asar_integrity_source: Option<(PathBuf, runtimepack_engine_tyrano::AsarArchive)> = None;
    if matches!(descriptor.id, EngineId::RpgMakerMv | EngineId::RpgMakerMz) {
        if let Some(ref root) = descriptor.content_root {
            println!(
                "Nested payload detected. Packaging content from scoped root '{}'",
                root
            );
        }
    } else if descriptor.id == EngineId::Tyrano {
        let asar_file = payload_index
            .get_file("resources/app.asar")
            .or_else(|| effective_source_index.get_file("resources/app.asar"));
        if let Some(indexed_asar) = asar_file {
            let asar_path = indexed_asar.real_path(&payload_index.root_path);
            if asar_path.exists() {
                println!("Electron package detected: resources/app.asar. Extracting to build staging directory...");
                let staging_dir = if let Some(ref mut ws) = scratch_ws {
                    match ws.register_owned_dir("staging_content") {
                        Ok(p) => p,
                        Err(e) => {
                            eprintln!("{}", e);
                            std::process::exit(1);
                        }
                    }
                } else {
                    let d = output.join("staging_content");
                    if let Err(e) = fs::create_dir_all(&d) {
                        eprintln!("[CNT-101] Failed to create staging directory: {}", e);
                        std::process::exit(1);
                    }
                    d
                };
                let archive = match runtimepack_engine_tyrano::AsarArchive::load(&asar_path) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("[CNT-102] Failed to parse app.asar: {}", e);
                        std::process::exit(1);
                    }
                };
                asar_integrity_source = Some((asar_path.clone(), archive.clone()));
                let extracted_count = match archive.extract_all(&asar_path, &staging_dir) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("[CNT-103] Failed to extract app.asar: {}", e);
                        std::process::exit(1);
                    }
                };
                println!(
                    "Extracted {} files from app.asar into {}",
                    extracted_count,
                    staging_dir.display()
                );
                content_source_dir = staging_dir.clone();
                staging_dir_to_cleanup = Some(staging_dir);
            }
        }
    }

    // Ren'Py's Android runtime executes `<ANDROID_PRIVATE>/main.py`; RAPT
    // renames the top-level `<game>.py` launcher to `main.py`. Reproduce that
    // alias inside the content container without mutating the source tree.
    let mut payload_index_for_pack = payload_index.clone();
    if descriptor.id == EngineId::RenPy && !payload_index_for_pack.files.contains_key("main.py") {
        match find_renpy_root_launcher(&payload_index_for_pack) {
            Ok(Some(launcher_path)) => {
                if let Some(launcher_file) =
                    payload_index_for_pack.get_file(&launcher_path).cloned()
                {
                    let mut main_alias = launcher_file.clone();
                    main_alias.logical_path = "main.py".to_string();
                    payload_index_for_pack
                        .files
                        .insert("main.py".to_string(), main_alias.clone());
                    payload_index_for_pack
                        .files
                        .insert("main.py".to_ascii_lowercase(), main_alias);
                    println!("Ren'Py launcher alias: {} -> main.py", launcher_path);
                }
            }
            Ok(None) => {
                eprintln!(
                    "Warning: no top-level Ren'Py launcher '<game>.py' found; the runtime \
                     expects <ANDROID_PRIVATE>/main.py and may not boot."
                );
            }
            Err(ambiguous_launchers) => {
                eprintln!(
                    "[{}] Multiple ambiguous root launchers found in Ren'Py distribution root without authoritative 'main.py':",
                    DiagnosticCode::RNP_LAUNCHER_AMBIGUOUS
                );
                for launcher in &ambiguous_launchers {
                    eprintln!("  ! candidate launcher: '{}'", launcher);
                }
                std::process::exit(1);
            }
        }
    }

    let mut pack_index = if let Some(ref staging_dir) = staging_dir_to_cleanup {
        match GameSourceIndex::build(staging_dir) {
            Ok(idx) => {
                if idx.has_file("index.html") {
                    idx
                } else {
                    let detector = runtimepack_engine_tyrano::TyranoDetector;
                    let candidates = detector.probe_all(&idx);
                    let aggregated = runtimepack_engine_tyrano::aggregate_candidates(candidates);
                    if let Some(ref inner_root) = aggregated.content_root {
                        match idx.scoped_to(inner_root) {
                            Ok(scoped) => scoped,
                            Err(_) => idx,
                        }
                    } else {
                        idx
                    }
                }
            }
            Err(e) => {
                eprintln!("[ING-101] Failed to ingest content source: {}", e);
                std::process::exit(1);
            }
        }
    } else if descriptor.content_root.is_some() || descriptor.id == EngineId::RenPy {
        payload_index_for_pack
    } else if content_source_dir == *path {
        payload_index_for_pack
    } else {
        match GameSourceIndex::build(&content_source_dir) {
            Ok(idx) => idx,
            Err(e) => {
                eprintln!("[ING-101] Failed to ingest content source: {}", e);
                std::process::exit(1);
            }
        }
    };

    if normalized_flavor == "release" {
        println!(
            "Hardening: bundling overlay, web shims, and fonts into GPACK (plain assets omitted)"
        );
        let overlay_src = match AndroidProjectGenerator::find_overlay_src() {
            Some(p) => p,
            None => {
                eprintln!("[BLD-109] Required overlay template not found in resource root");
                std::process::exit(1);
            }
        };
        let shims_src = match AndroidProjectGenerator::find_template_file(
            "android/shell/app/src/main/assets/web_shims",
        ) {
            Some(p) => p,
            None => {
                eprintln!("[BLD-109] Required web shims template not found in resource root");
                std::process::exit(1);
            }
        };
        let fonts_src =
            AndroidProjectGenerator::find_template_file("android/shell/app/src/main/assets/fonts");

        if let Err(e) = AndroidProjectGenerator::append_runtime_overlay_assets(
            &mut pack_index,
            &overlay_src,
            &shims_src,
            fonts_src.as_deref(),
        ) {
            eprintln!("[CNT-102] Failed to bundle runtime overlay assets: {}", e);
            std::process::exit(1);
        }
    }

    let total_bytes = match GpackPacker::pack_from_index_with_options(
        &pack_index,
        &pack_path,
        encryption_key.as_ref(),
    ) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[CNT-102] Failed to pack content: {}", e);
            std::process::exit(1);
        }
    };

    if let Some(_staging_dir) = staging_dir_to_cleanup {
        // Preserved staging directory without deletion for audit trail
    }

    // Post-pack integrity gate: guarantee that ASAR-sourced content survives the
    // extract → stage → pack pipeline byte-exact. This catches the historical
    // class of bug where an ASAR header-padding miscalculation silently shifted
    // every packed entry before it could ship.
    if let Some((asar_path, archive)) = &asar_integrity_source {
        if encryption_key.is_some() {
            println!("ASAR round-trip integrity: skipped (encrypted-streaming mode)");
        } else {
            match verify_asar_pack_roundtrip(asar_path, archive, &pack_path) {
                Ok(0) => {
                    eprintln!(
                        "[CNT-102] Post-pack ASAR round-trip integrity check failed: the archive \
                         has entries but none could be matched to a game.pack entry (possible \
                         path-mapping break); refusing to ship unverified content."
                    );
                    std::process::exit(1);
                }
                Ok(verified) => {
                    println!(
                        "ASAR round-trip integrity: verified {} entries (SHA-256 sample)",
                        verified
                    );
                }
                Err(reason) => {
                    eprintln!(
                        "[CNT-102] Post-pack ASAR round-trip integrity check failed: {}",
                        reason
                    );
                    std::process::exit(1);
                }
            }
        }
    }

    // Calculate pack hash
    let mut hasher = Sha256::new();
    if let Ok(mut pf) = fs::File::open(&pack_path) {
        use std::io::Read;
        let mut buf = [0u8; 8192];
        while let Ok(n) = pf.read(&mut buf) {
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
    }
    let pack_hash = format!("{:x}", hasher.finalize());
    let packed_file_count = runtimepack_pack::GpackReader::open(&pack_path)
        .map(|r| r.file_count())
        .unwrap_or_else(|_| {
            pack_index
                .files
                .values()
                .filter(|f| runtimepack_pack::GpackPacker::should_include_in_pack(&f.logical_path))
                .count()
        });
    println!(
        "GPACK created: {} ({:.2} MB, hash: {})",
        pack_path.file_name().unwrap().to_string_lossy(),
        total_bytes as f64 / (1024.0 * 1024.0),
        &pack_hash[..8]
    );

    // 7. Assemble BuildManifest
    let audit_manifest = AuditManifestEntry {
        policy: audit_policy,
        build_policy: AppliedBuildPolicy::from(typed_decision.policy),
        compatibility_policy: AppliedCompatibilityBuildPolicy::from(
            typed_decision.compatibility_policy,
        ),
        policy_ruleset_version: BUILD_POLICY_RULESET_VERSION,
        forwarded_unknowns: typed_decision.forwarded_codes(),
        compatibility_warnings: typed_decision.compatibility_warning_records(),
        notice_sha256: experimental_notice.as_ref().map(|notice| {
            format!("{:x}", Sha256::digest(notice.as_bytes()))
        }),
        report_path: "asset-audit-report.json".into(),
        report_sha256: audit_hash,
        mode: audit_mode,
        outcome: audit_report.outcome,
        overrides: audit_overrides,
    };
    let rtp_source_str = payload_index
        .rtp_path()
        .map(|p| p.to_string_lossy().to_string());

    // Themes resolution
    let theme_manifest_entry = if !args.themes.is_empty() || args.default_theme.is_some() {
        let selected_themes = if args.themes.is_empty() {
            vec![args.default_theme.clone().unwrap()]
        } else {
            args.themes.clone()
        };
        let default_theme = args.default_theme.clone().unwrap_or_else(|| {
            if selected_themes.iter().any(|t| t == "matte") {
                "matte".to_string()
            } else {
                selected_themes
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "matte".to_string())
            }
        });

        if !selected_themes.contains(&default_theme) {
            eprintln!(
                "[{}] Default theme '{}' is not in packaged themes list: {:?}",
                runtimepack_core::diagnostics::THM_DEFAULT_NOT_PACKAGED,
                default_theme,
                selected_themes
            );
            std::process::exit(1);
        }

        Some(ThemeManifestEntry {
            default_theme,
            selected_themes,
        })
    } else {
        None
    };

    let completion_hooks = if matches!(descriptor.id, EngineId::RpgMakerMv | EngineId::RpgMakerMz) {
        let hooks = runtimepack_engine_rpgmaker_web::declared_script_completion_hooks(
            &pack_index,
            descriptor.id,
        );
        if !hooks.is_empty() {
            Some(hooks)
        } else {
            None
        }
    } else {
        None
    };

    let mut runtime_capabilities = runtime.capabilities.clone();
    if completion_hooks.is_some() && !runtime_capabilities.contains(&"CAP-WEB-029".to_string()) {
        runtime_capabilities.push("CAP-WEB-029".to_string());
    }
    for cap in &applied_profile_caps {
        if !runtime_capabilities.contains(cap) {
            runtime_capabilities.push(cap.clone());
        }
    }

    let mut manifest = BuildManifest {
        schema_version: 1,
        runtimepack_version: "0.1.0".to_string(),
        git_commit: None,
        build_timestamp: Utc::now().to_rfc3339(),
        overlay_mode: normalized_overlay_mode.clone(),
        vnm_input_guard: normalized_vnm_input_guard.clone(),
        flavor: Some(normalized_flavor.clone()),
        audit: Some(audit_manifest.clone()),
        rtp_source: rtp_source_str.clone(),
        themes: theme_manifest_entry.clone(),
        compat_profile: applied_profile_id.clone(),
        runtime_integrity_sha256: None,
        app: AppConfig {
            name: app_name.clone(),
            package: pkg_id.clone(),
            version_name: "1.0.0".to_string(),
            version_code: 1,
        },
        engine: EngineManifestEntry {
            id: descriptor.id.to_string(),
            confidence: descriptor.confidence.to_string(),
            overridden: descriptor.overridden,
            detected_version: descriptor.detected_version.clone(),
            content_root: descriptor.content_root.clone(),
            evidence: descriptor.evidence.clone(),
            conflicts: descriptor.conflicts.clone(),
        },
        runtime: RuntimeManifestEntry {
            id: runtime.id.clone(),
            version: runtime.version.clone(),
            family: runtime.family.to_string(),
            runtime_api: runtime.runtime_api,
            capabilities: runtime_capabilities,
            script_completion_hooks: completion_hooks,
            native_libs_dir: runtime
                .artifacts
                .as_ref()
                .and_then(|a| a.native_libs_dir.clone()),
        },
        content: ContentManifestEntry {
            mode: content_mode.clone(),
            content_hash: pack_hash.clone(),
            file_count: packed_file_count,
            total_bytes,
        },
        android: AndroidManifestEntry {
            abis: {
                // Record only ABIs that are declared *and* backed by real
                // native libraries in the resource root, so the manifest never
                // advertises an ABI the APK cannot load. Narrowing is reported,
                // never silent.
                let declared = runtime.abis.clone();
                let jni_dir = runtime
                    .artifacts
                    .as_ref()
                    .and_then(|a| a.native_libs_dir.as_deref())
                    .and_then(AndroidProjectGenerator::find_template_file);
                match runtimepack_core::runtime_registry::effective_abis_report(
                    &declared,
                    jni_dir.as_deref(),
                ) {
                    Ok((abis, dropped)) => {
                        if !dropped.is_empty() {
                            eprintln!(
                                "[BLD-109] ABI narrowing for runtime '{}': declared [{}] but no libraries shipped for [{}]; packaging [{}]. The APK will not advertise the dropped ABIs.",
                                runtime.id,
                                declared.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", "),
                                dropped.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", "),
                                abis.iter().map(|a| a.to_string()).collect::<Vec<_>>().join(", ")
                            );
                        }
                        abis.iter().map(|a| a.to_string()).collect::<Vec<_>>()
                    }
                    Err(_) => {
                        // Resolution/generation will fail closed with BLD-109;
                        // record the declaration so the manifest is not silent.
                        declared.iter().map(|a| a.to_string()).collect()
                    }
                }
            },
            min_sdk: runtime.android.min_sdk,
            target_sdk: runtime.android.target_sdk,
            permissions: vec![],
        },
        provenance: ProvenanceManifestEntry {
            source_hash: source_index.source_hash.clone(),
            config_hash: "".to_string(),
            apk_hash: None,
            flavor: Some(normalized_flavor.clone()),
            audit: Some(audit_manifest),
            rtp_source: rtp_source_str,
            themes: theme_manifest_entry,
            compat_profile: applied_profile_id.clone(),
            toolchain: ToolchainProvenance {
                agp_version: Some("8.3.0".to_string()),
                gradle_version: Some("8.7".to_string()),
                jdk_version: Some("17".to_string()),
                android_sdk_platform: Some("34".to_string()),
                ndk_version: Some("26.1.10909125".to_string()),
            },
        },
    };

    if let Some(ref entry) = manifest.themes {
        println!(
            "Themes packaged: {} (default: {})",
            entry.selected_themes.join(", "),
            entry.default_theme
        );
    }

    // 8. Generate Android Project
    println!("\nGenerating standalone Android project structure...");
    let android_project_dir = if let Some(ref mut ws) = scratch_ws {
        match ws.register_owned_dir("android-project") {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        output.join("android-project")
    };
    // The generator must emit exactly the toolchain and ABI set that was
    // validated, so `local.properties`, `abiFilters` and the manifest cannot
    // disagree with the Gradle child environment.
    let project_options = runtimepack_core::build_plan::AndroidProjectOptions {
        sdk_dir: early_toolchain
            .as_ref()
            .and_then(|t| t.sdk_root.clone()),
        abi_filters: manifest.android.abis.clone(),
    };
    if let Err(e) = AndroidProjectGenerator::emit_project_with_options(
        &android_project_dir,
        &mut manifest,
        Some(&pack_path),
        Some(path),
        Some(&normalized_flavor),
        args.theme_catalog.as_deref(),
        &project_options,
    ) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
    println!("Android project: {}", android_project_dir.display());

    // Ensure asset-dependency-graph.json is packaged into APK assets
    let target_graph = android_project_dir.join("app/src/main/assets/asset-dependency-graph.json");
    if !target_graph.exists() {
        let source_graph = output.join("asset-dependency-graph.json");
        if source_graph.is_file() {
            let _ = fs::copy(&source_graph, &target_graph);
        } else {
            let _ = fs::write(&target_graph, &audit_artifacts.graph_json);
        }
    }
    let target_audit_report =
        android_project_dir.join("app/src/main/assets/asset-audit-report.json");
    if !target_audit_report.exists() {
        let source_report = output.join("asset-audit-report.json");
        if source_report.is_file() {
            let _ = fs::copy(&source_report, &target_audit_report);
        } else {
            let _ = fs::write(&target_audit_report, &audit_artifacts.report_json);
        }
    }

    if content_mode != "plain" {
        // The recorded drop set must describe the artifact, so it is computed from
        // the **same index the packer received** (`pack_index`, already scoped to
        // the content root and including the release overlay). Computing it from
        // the full source index would name game-root-relative paths the packer
        // never evaluates.
        let exclusions = GpackPacker::compute_packaging_exclusions(&pack_index);
        let exclusions_json = serde_json::to_string_pretty(&exclusions).unwrap_or_default();
        let source_exclusions = output.join("packaging-exclusions.json");
        let _ = fs::write(&source_exclusions, &exclusions_json);
        let target_exclusions =
            android_project_dir.join("app/src/main/assets/packaging-exclusions.json");
        if !target_exclusions.exists() {
            let _ = fs::write(&target_exclusions, &exclusions_json);
        }
    }

    // Experimental builds ship the notice inside the APK assets. The durable copy
    // and the manifest digest are written only after the artifact is published, so
    // a rejected or collided build can never leave a notice describing a build
    // that does not exist.
    let experimental_notice = if let Some(notice) = experimental_notice {
        let assets_dir = android_project_dir.join("app/src/main/assets");
        if let Err(e) = fs::create_dir_all(&assets_dir) {
            eprintln!(
                "Failed to prepare APK assets directory '{}': {}",
                assets_dir.display(),
                e
            );
            std::process::exit(1);
        }
        let asset_notice = assets_dir.join("EXPERIMENTAL-NOTICE.txt");
        if let Ok(meta) = asset_notice.symlink_metadata() {
            if meta.file_type().is_symlink() {
                eprintln!(
                    "Refusing to write the experimental notice through a symlink at '{}'",
                    asset_notice.display()
                );
                std::process::exit(1);
            }
        }
        if let Err(e) = fs::write(&asset_notice, &notice) {
            eprintln!(
                "Failed to write experimental build notice '{}': {}",
                asset_notice.display(),
                e
            );
            std::process::exit(1);
        }
        Some(notice)
    } else {
        None
    };

    if descriptor.id == EngineId::VisualNovelMaker {
        let vnm_profile = runtimepack_engine_vnmaker::VnmPreparationProfile::detect(&source_index);
        let vnm_json = serde_json::to_string_pretty(&vnm_profile).unwrap_or_default();
        let source_vnm_profile = output.join("vnm-preparation-profile.json");
        let _ = fs::write(&source_vnm_profile, &vnm_json);
        let target_vnm_profile =
            android_project_dir.join("app/src/main/assets/vnm-preparation-profile.json");
        if !target_vnm_profile.exists() {
            let _ = fs::write(&target_vnm_profile, &vnm_json);
        }
    }

    // 9. Invoke Gradle compiler if toolchains are present
    if args.no_compile || std::env::var("RUNTIMEPACK_SKIP_GRADLE").is_ok() {
        println!(
            "\nSkipping Gradle compilation (--no-compile). Project structure generated at {}.",
            android_project_dir.display()
        );

        // In no-compile mode, write durable reports and checksums directly
        println!("\nWriting build artifacts and third-party notices...");
        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        fs::write(output.join("build-manifest.json"), &manifest_json).unwrap();

        let report_json = serde_json::to_string_pretty(&report).unwrap();
        fs::write(output.join("compatibility-report.json"), &report_json).unwrap();
        if let Err(error) =
            write_audit_artifacts_with_options(output, &audit_artifacts, !args.no_html)
        {
            eprintln!("[CMP-AUD-010] Failed to write asset audit artifacts: {error}");
            std::process::exit(1);
        }

        let notices_dir = output.join("third-party-notices");
        fs::create_dir_all(&notices_dir).unwrap();
        let mut notices_text = String::from("Third-Party Notices:\n\n");
        for item in EvidenceRegistry::get_canonical_evidence() {
            notices_text.push_str(&format!(
                "[{}] {}\nURL: {}\n\n",
                item.id, item.title, item.url
            ));
        }
        fs::write(notices_dir.join("NOTICES.txt"), notices_text).unwrap();

        // Selected-runtime license payload is written before checksums so the
        // durable artifact and its checksum list are published together.
        write_durable_runtime_notices_or_exit(output, &manifest.runtime.id);

        // This path produces no APK, so the durable notice is the shipped copy.
        if let Some(notice) = experimental_notice.as_ref() {
            let durable_notice = output.join("EXPERIMENTAL-NOTICE.txt");
            if let Ok(meta) = durable_notice.symlink_metadata() {
                if meta.file_type().is_symlink() {
                    eprintln!(
                        "Refusing to write the experimental notice through a symlink at '{}'",
                        durable_notice.display()
                    );
                    std::process::exit(1);
                }
            }
            if let Err(e) = fs::write(&durable_notice, notice) {
                eprintln!(
                    "Failed to write experimental build notice '{}': {}",
                    durable_notice.display(),
                    e
                );
                std::process::exit(1);
            }
            println!("Experimental:  {}", durable_notice.display());
        }

        let mut checksums = format!(
            "{}  game.pack\n{}  build-manifest.json\n",
            pack_hash,
            format!("{:x}", Sha256::digest(manifest_json.as_bytes()))
        );
        if !checksums.ends_with('\n') {
            checksums.push('\n');
        }
        fs::write(output.join("checksums.txt"), checksums).unwrap();
        // Fail closed if the notice files cannot all be accounted for: the
        // integrity list must cover the durable notices it ships with.
        append_notice_checksums_or_exit(output);

        println!(
            "Build report:   {}",
            output.join("build-manifest.json").display()
        );
        println!(
            "Compatibility:  {}",
            output.join("compatibility-report.json").display()
        );
        println!(
            "Notices:        {}",
            notices_dir.join("NOTICES.txt").display()
        );

        if args.probe {
            println!("[probe] skipping runtime verification — no APK was produced by build");
        }
        println!("\n[SUCCESS] Standalone Android project generation complete!");
        return;
    }

    // Fail closed on a missing/incomplete SDK or JDK *before* Gradle starts, so
    // the operator gets BLD-101 instead of an opaque Gradle
    // "SDK location not found". The same resolved toolchain is then used for
    // the Gradle command environment and the artifact verifier.
    // Re-validate against the *generated* project: its local.properties is what
    // Gradle will read, and the early check ran before that file existed. Values
    // resolved earlier (for example a retained project's sdk.dir) are carried
    // forward so a second resolution cannot silently lose them.
    let toolchain = gradle::merge_toolchain(
        gradle::resolve_android_toolchain(Some(&android_project_dir)),
        early_toolchain.as_ref(),
    );
    if let Err(diag) = gradle::validate_android_toolchain(&toolchain, Some(&android_project_dir)) {
        eprintln!("{}", diag);
        if let Some(ws) = scratch_ws.as_ref() {
            println!("Workspace retained: {}", ws.scratch_root().display());
        }
        std::process::exit(1);
    }

    let (mut gradle_cmd, strategy) = match gradle::resolve_gradle_command_with(&android_project_dir, &toolchain) {
        Ok(res) => res,
        Err(diag) => {
            eprintln!("{}", diag);
            if let Some(ws) = scratch_ws.as_ref() {
                println!("Workspace retained: {}", ws.scratch_root().display());
            }
            std::process::exit(1);
        }
    };

    println!("\nCompiling standalone Android package with Gradle ({strategy})...");
    if strategy.starts_with("project-wrapper") {
        println!("Using Gradle Wrapper in: {}", android_project_dir.display());
    }

    // Invoke assembleDebug without clean to prevent wiping existing project artifacts
    gradle_cmd.arg("assembleDebug");
    gradle_cmd.arg("--console=plain");
    gradle_cmd.current_dir(&android_project_dir);

    match gradle_cmd.output() {
        Ok(res) if res.status.success() => {
            let apk_source = android_project_dir.join("app/build/outputs/apk/debug/app-debug.apk");
            if !apk_source.exists() {
                eprintln!(
                    "[{}] Gradle build reported success (exit 0) but output artifact was not found at {}.",
                    DiagnosticCode::BLD_GRADLE_FAILED,
                    apk_source.display()
                );
                if let Some(ws) = scratch_ws.as_ref() {
                    println!("Workspace retained: {}", ws.scratch_root().display());
                }
                std::process::exit(1);
            }

            // Execute finalization lifecycle via build_finalize
            let mut owned_verifier =
                runtimepack_core::build_plan::artifact_verifier::OwnedArtifactVerifier::from_env();
            // Use the same resolved toolchain that drove the Gradle invocation,
            // so a host relying on a bundled JDK or a local.properties sdk.dir is
            // verified against the toolchain that actually built the APK.
            if let Some(jh) = toolchain.java_home.clone() {
                owned_verifier.verifier.java_home = Some(jh);
            }
            if let Some(sdk) = toolchain.sdk_root.clone() {
                owned_verifier.verifier.sdk_root = Some(sdk);
            }
            let mut verifier =
                runtimepack_core::build_plan::artifact_verifier::ArtifactVerifier::with_verifiers(
                    &owned_verifier.verifier,
                    &owned_verifier.zip_validator,
                );
            // An experimental build claims its APK ships the notice. Prove it before
            // publication, while the artifact is still a temporary copy.
            if let Some(notice) = experimental_notice.as_ref() {
                verifier = verifier.with_required_asset(
                    "assets/EXPERIMENTAL-NOTICE.txt",
                    &format!("{:x}", Sha256::digest(notice.as_bytes())),
                );
            }

            let probe_fn: Option<
                Box<dyn Fn(&Path) -> Result<bool, runtimepack_core::diagnostics::DiagnosticError>>,
            > = if args.probe {
                Some(Box::new(|dest_apk: &Path| {
                    println!("\n--- Runtime Verification (T1 Probe) ---");
                    let probe_report_path = output.join("probe-report.json");
                    match probe::resolve_device_target(
                        args.probe_device.as_deref(),
                        None,
                        probe::AdbTarget::scan_connected_devices,
                    ) {
                        Ok(probe::DeviceResolution::Resolved(device)) => {
                            match probe::run_probe(dest_apk, &device, &probe_report_path) {
                                Ok(probe_report) => {
                                    if probe_report.overall != "PASS" {
                                        eprintln!(
                                            "\n[probe] FAILED — see {}",
                                            probe_report_path.display()
                                        );
                                        Ok(false)
                                    } else {
                                        println!(
                                            "\n[probe] PASSED — runtime verification succeeded"
                                        );
                                        Ok(true)
                                    }
                                }
                                Err(e) => {
                                    eprintln!("\n[probe] FAILED: {}", e);
                                    Ok(false)
                                }
                            }
                        }
                        Ok(probe::DeviceResolution::NoneAvailable) => {
                            println!("[probe] no device available — skipping runtime verification");
                            Ok(true)
                        }
                        Ok(probe::DeviceResolution::Ambiguous(msg)) => {
                            println!(
                                "[probe] multiple devices available — skipping runtime verification ({})",
                                msg
                            );
                            Ok(true)
                        }
                        Err(e) => {
                            println!(
                                "[probe] no device available ({}) — skipping runtime verification",
                                e
                            );
                            Ok(true)
                        }
                    }
                }))
            } else {
                None
            };

            let probe_ref = probe_fn.as_deref();
            let mut final_manifest = manifest;

            // Durable runtime notices are gathered and validated BEFORE the APK is
            // published: a missing notice bundle fails the build while nothing has
            // been claimed as a complete artifact yet.
            write_durable_runtime_notices_or_exit(output, &final_manifest.runtime.id);

            let publish_dir = if let Some(ref mut ws) = scratch_ws {
                match ws.register_owned_dir("publish") {
                    Ok(p) => Some(p),
                    Err(e) => {
                        eprintln!("{}", e);
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };
            match build_finalize::finalize_build(
                scratch_ws.as_ref(),
                publish_dir.as_deref(),
                output,
                &sanitized_apk_name,
                &apk_source,
                args.keep_build_files,
                &verifier,
                probe_ref,
                &mut final_manifest,
                &report,
                &audit_artifacts,
                args.no_html,
                experimental_notice.as_deref(),
            ) {
                Ok(_outcome) => {
                    append_notice_checksums_or_exit(output);
                    // A dropped `.node` a game requires is invisible on a clean
                    // build unless it is printed here (MEDIUM-4).
                    let notices = pack_excluded_notice_lines(&report);
                    if !notices.is_empty() {
                        eprintln!(
                            "\nPack-excluded native files (not shipped; game may fail at runtime):"
                        );
                        for line in &notices {
                            eprintln!("  {line}");
                        }
                    }
                    println!("\n[SUCCESS] Standalone Android packaging complete!");
                }
                Err(e) => {
                    eprintln!("{}", e);
                    if let Some(ws) = scratch_ws.as_ref() {
                        println!("Workspace retained: {}", ws.scratch_root().display());
                    }
                    std::process::exit(1);
                }
            }
        }
        Ok(res) => {
            let code = res.status.code().unwrap_or(1);
            eprintln!(
                "[{}] Gradle build failed with exit code {}. Android project is available in {}.",
                DiagnosticCode::BLD_GRADLE_FAILED,
                code,
                android_project_dir.display()
            );
            let stdout = String::from_utf8_lossy(&res.stdout);
            let stderr = String::from_utf8_lossy(&res.stderr);
            if !stdout.is_empty() {
                eprintln!("Gradle stdout:\n{}", stdout);
            }
            if !stderr.is_empty() {
                eprintln!("Gradle stderr:\n{}", stderr);
            }
            if let Some(ws) = scratch_ws.as_ref() {
                println!("Workspace retained: {}", ws.scratch_root().display());
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!(
                "[{}] Failed to execute Gradle compiler: {}. Project structure generated at {}.",
                DiagnosticCode::BLD_GRADLE_FAILED,
                e,
                android_project_dir.display()
            );
            if let Some(ws) = scratch_ws.as_ref() {
                println!("Workspace retained: {}", ws.scratch_root().display());
            }
            std::process::exit(1);
        }
    }
}

/// Operator-facing lines for the pack-excluded native disclosures of a build.
///
/// A successful build prints no findings at all, so a `.node` addon the game may
/// `require` disappears silently when the packer drops it. These lines name the
/// diagnostic code, the dropped path and the packer's exclusion reason (carried
/// verbatim in the disclosure message), labelled with the runtime risk
/// (MEDIUM-4). Only disclosures are listed: nothing here can block a build.
fn pack_excluded_notice_lines(
    report: &runtimepack_core::compatibility::CompatibilityReport,
) -> Vec<String> {
    report
        .issues
        .iter()
        .filter(|issue| {
            issue.code == DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED
                || issue.code == DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED
        })
        .map(|issue| {
            format!(
                "[{}] {} — {}",
                sanitize_untrusted_line(&issue.code, UNTRUSTED_LINE_LIMIT),
                sanitize_untrusted_line(
                    issue.source_path.as_deref().unwrap_or("<unknown path>"),
                    UNTRUSTED_LINE_LIMIT
                ),
                sanitize_untrusted_line(&issue.message, UNTRUSTED_LINE_LIMIT)
            )
        })
        .collect()
}

/// Human-readable notice shipped with a build that carries accepted risk.
///
/// Two disclosures live here because both are unvalidated risk: `--allow-unknown`
/// forwarded unknowns and compatibility-warning acceptance. Every field that comes
/// from target-derived analysis text is rendered through
/// [`sanitize_untrusted_line`], so a crafted plugin path or reason cannot inject
/// lines or terminal escapes into the notice. The authoritative evidence stays
/// unmodified in the audit report.
fn render_experimental_notice(decision: &TypedBuildDecision) -> String {
    let mut notice = String::new();
    notice.push_str("EXPERIMENTAL BUILD NOTICE\n");
    notice.push_str("========================\n\n");
    if decision.compatibility_warnings.is_empty() {
        notice.push_str(
            "This build is EXPERIMENTAL. It was produced with --allow-unknown, which forwards\n\
             findings that static analysis could only classify as UNKNOWN: not proven safe and\n\
             not proven blocked. Forwarded findings are listed below with the site that raised\n\
             them.\n\n",
        );
    } else if decision.forwarded.is_empty() {
        let has_required = decision
            .compatibility_warnings
            .iter()
            .any(|warning| {
                matches!(
                    warning.state,
                    runtimepack_core::build_policy::CompatibilityWarningState::Required
                )
            });
        let has_unknown = decision
            .compatibility_warnings
            .iter()
            .any(|warning| {
                matches!(
                    warning.state,
                    runtimepack_core::build_policy::CompatibilityWarningState::Unknown
                )
            });
        if has_required && has_unknown {
            notice.push_str(
                "This build is EXPERIMENTAL. It was produced with --compatibility-policy warn, which\n\
                 continues past both unproven (UNKNOWN) compatibility facts and proven (REQUIRED)\n\
                 desktop-only requirements. Every accepted finding keeps its original knowledge\n\
                 class: accepting the risk did not make the feature safe.\n\n",
            );
        } else if has_required {
            notice.push_str(
                "This build is EXPERIMENTAL. It was produced with --compatibility-policy warn, which\n\
                 continues past proven (REQUIRED) desktop-only requirements. The underlying findings\n\
                 remain classified as BLOCKED: accepting the risk did not make the feature safe.\n\n",
            );
        } else {
            notice.push_str(
                "This build is EXPERIMENTAL. It was produced with --compatibility-policy warn, which\n\
                 continues past unproven (UNKNOWN) compatibility facts. The underlying findings keep\n\
                 their original knowledge class: accepting the risk did not make the feature safe.\n\n",
            );
        }
    } else {
        notice.push_str(
            "This build is EXPERIMENTAL. It was produced with --allow-unknown and\n\
             --compatibility-policy warn, so it forwards UNKNOWN findings and continues past\n\
             accepted compatibility facts. Neither is proof that a feature works: the\n\
             underlying findings keep their original knowledge classes.\n\n",
        );
    }
    notice.push_str(
        "This build is NOT device-validated. No device boot, input, audio, save/load or\n\
         gameplay verification was performed for it. Treat it as unverified software.\n\n",
    );
    notice.push_str(&format!(
        "Build policy:            {}\n",
        if decision.is_experimental() {
            "experimental (--allow-unknown)"
        } else {
            "default"
        }
    ));
    notice.push_str(&format!(
        "Compatibility policy:    {} (--compatibility-policy)\n",
        decision.compatibility_policy.label()
    ));
    notice.push_str(&format!(
        "Policy ruleset version:  {}\n",
        BUILD_POLICY_RULESET_VERSION
    ));
    notice.push_str(&format!(
        "Forwarded findings:      {}\n",
        decision.forwarded.len()
    ));
    notice.push_str(&format!(
        "Distinct codes:          {}\n",
        decision.forwarded_codes().len()
    ));
    notice.push_str(&format!(
        "Accepted compatibility:  {}\n\n",
        decision.compatibility_warnings.len()
    ));

    if decision.forwarded.is_empty() && decision.compatibility_warnings.is_empty() {
        notice.push_str("(no accepted risk was recorded by this invocation)\n");
        return notice;
    }

    for finding in &decision.forwarded {
        let site = sanitize_untrusted_line(
            finding.source_path.as_deref().unwrap_or("<unknown site>"),
            UNTRUSTED_LINE_LIMIT,
        );
        let reason = sanitize_untrusted_line(&finding.message, UNTRUSTED_LINE_LIMIT);
        notice.push_str(&format!("[{}] {} — {}\n", finding.code, site, reason));
    }

    if !decision.compatibility_warnings.is_empty() {
        notice.push_str(
            "\nAccepted compatibility risks (NOT_RUN: no runtime validation was performed)\n",
        );
        for warning in &decision.compatibility_warnings {
            let site = sanitize_untrusted_line(
                warning.finding.source_path.as_deref().unwrap_or("<unknown site>"),
                UNTRUSTED_LINE_LIMIT,
            );
            let reason = sanitize_untrusted_line(&warning.finding.message, UNTRUSTED_LINE_LIMIT);
            let ids = if warning.audit_finding_ids.is_empty() {
                "<analyzer only>".to_string()
            } else {
                warning
                    .audit_finding_ids
                    .iter()
                    .map(|id| sanitize_untrusted_line(id, UNTRUSTED_LINE_LIMIT))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            notice.push_str(&format!(
                "[{}] state={} class={} requiredness={} runtime_validation=NOT_RUN\n  site: {}\n  audit finding ids: {}\n  reason: {}\n",
                sanitize_untrusted_line(&warning.finding.code, UNTRUSTED_LINE_LIMIT),
                warning.state.label(),
                warning.finding.class.label(),
                warning.finding.requiredness.label(),
                site,
                ids,
                reason
            ));
        }
    }
    notice
}

fn write_durable_runtime_notices_or_exit(output: &Path, runtime_id: &str) {
    // Copies the selected runtime's license/provenance files into the durable
    // `<output>/third-party-notices/<runtime_id>/` tree, using the same resource
    // directory (and the same copy routine) the generated Android project uses for
    // its APK assets, so durable and shipped notices cannot drift.
    //
    // The requirement itself comes from runtime metadata
    // (`RuntimeLicenses.required_notice_files`), never from an engine name. A
    // runtime that declares required notices and cannot supply them exits non-zero.
    let resource_root = match runtimepack_core::resource_root::resolve_resource_root() {
        Ok(root) => root,
        Err(err) => {
            eprintln!(
                "[{}] Failed to resolve the resource root for durable third-party notices: {}",
                err.code().unwrap_or(DiagnosticCode::BLD_RESOURCE_ROOT_MISSING),
                err
            );
            std::process::exit(1);
        }
    };

    match runtimepack_core::resource_root::write_durable_runtime_notices(
        output,
        runtime_id,
        &resource_root,
    ) {
        Ok(copied) => {
            if copied > 0 {
                println!(
                    "Runtime notices: {} ({} files)",
                    output.join("third-party-notices").join(runtime_id).display(),
                    copied
                );
            }
        }
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    }
}

/// Adds the durable notice checksums to the existing `checksums.txt` (the compiled
/// path, where the artifact gate writes the file before returning).
///
/// The accounting itself lives in `resource_root::merge_notice_checksums`, which
/// fails closed on a missing/empty baseline and on any unreadable notice file.
fn append_notice_checksums_or_exit(output: &Path) {
    let path = output.join("checksums.txt");
    let text = match runtimepack_core::resource_root::merge_notice_checksums(output) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };
    if let Err(err) = fs::write(&path, text) {
        eprintln!(
            "[{}] Failed to record durable notice checksums: {}",
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            err
        );
        std::process::exit(1);
    }
}

fn cmd_pack_bundle(args: PackBundleArgs) {
    let workspace_root = match runtimepack_core::resource_root::resolve_resource_root() {
        Ok(r) => r,
        Err(e) => {
            eprintln!(
                "[{}] Failed to resolve workspace root for packaging: {}",
                e.code().unwrap_or("BLD-107"),
                e
            );
            std::process::exit(1);
        }
    };

    println!("Packaging portable distribution bundle...");
    println!("  Source workspace: {}", workspace_root.display());
    println!("  Output directory: {}", args.output.display());
    if let Some(ref b) = args.bin {
        println!("  Binary payload:   {}", b.display());
    }
    if args.local_diagnostic_native {
        println!(
            "  Scope:            LOCAL DIAGNOSTIC ONLY (includes prebuilt RGSS native libraries; SOURCE_UNVERIFIED; do not redistribute)"
        );
    }

    let options = runtimepack_core::resource_root::BundlePackageOptions {
        include_local_diagnostic_native: args.local_diagnostic_native,
    };

    match runtimepack_core::resource_root::package_distribution_bundle_with_options(
        &workspace_root,
        &args.output,
        args.bin.as_deref(),
        &options,
    ) {
        Ok(report) => {
            if args.local_diagnostic_native {
                println!(
                    "\nLocal diagnostic bundle packaged successfully (LOCAL_DIAGNOSTIC_ONLY — no remote distribution):"
                );
            } else {
                println!("\nDistribution bundle packaged successfully:");
            }
            println!("  Total files:    {}", report.total_files);
            println!("  Total size:     {} bytes", report.total_bytes);
            println!("  Checksums:      {}", report.checksums_path.display());
            println!("  Manifest:       {}", report.manifest_path.display());
        }
        Err(e) => {
            eprintln!(
                "\n[{}] Failed to package distribution bundle: {}",
                e.code().unwrap_or("BLD-108"),
                e
            );
            std::process::exit(1);
        }
    }
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

fn cmd_theme_list(catalog_path: Option<PathBuf>) {
    let catalog_res = AndroidProjectGenerator::find_theme_catalog_path(catalog_path.as_deref());
    let catalog_dir = match catalog_res {
        Ok(dir) => dir,
        Err(err) => {
            eprintln!("[{}] {}", err.code().unwrap_or("THM-008"), err);
            std::process::exit(1);
        }
    };

    println!(
        "RuntimePack Themes — Catalog at {}\n",
        catalog_dir.display()
    );

    let entries = match fs::read_dir(&catalog_dir) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("[THM-008] Failed to read catalog directory: {}", err);
            std::process::exit(1);
        }
    };

    let mut discovered = 0;
    let mut dir_entries: Vec<_> = entries.flatten().collect();
    dir_entries.sort_by_key(|e| e.path());

    for entry in dir_entries {
        let path = entry.path();
        if path.is_dir() && path.join("theme.json").is_file() {
            discovered += 1;
            let dir_name = path.file_name().unwrap_or_default().to_string_lossy();
            match ThemePackage::load_from_dir(&path) {
                Ok(pkg) => match pkg.validate("1.0.0") {
                    Ok(()) => {
                        println!(
                            "  - [{}] {} v{} (API {}) [VALID]",
                            pkg.manifest.id,
                            pkg.manifest.name,
                            pkg.manifest.version,
                            pkg.manifest.theme_api
                        );
                        if let Some(ref desc) = pkg.manifest.description {
                            println!("      Description: {}", desc);
                        }
                        println!("      Path: {}", path.display());
                    }
                    Err(val_err) => {
                        println!(
                            "  - [{}] {} v{} (API {}) [INVALID]",
                            pkg.manifest.id,
                            pkg.manifest.name,
                            pkg.manifest.version,
                            pkg.manifest.theme_api
                        );
                        println!("      Error: {}", val_err);
                        println!("      Path: {}", path.display());
                    }
                },
                Err(load_err) => {
                    println!("  - [{}] (Failed to load) [INVALID]", dir_name);
                    println!("      Error: {}", load_err);
                    println!("      Path: {}", path.display());
                }
            }
        }
    }

    if discovered == 0 {
        println!("No theme packages discovered in {}", catalog_dir.display());
    } else {
        println!("\nDiscovered {} theme package(s).", discovered);
    }
}

fn cmd_theme_validate(path: &Path, api_version: &str) {
    println!("Validating theme package at {}...\n", path.display());

    let pkg = match ThemePackage::load_from_dir(path) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("{}", err);
            std::process::exit(1);
        }
    };

    println!("  ID:          {}", pkg.manifest.id);
    println!("  Name:        {}", pkg.manifest.name);
    println!("  Version:     {}", pkg.manifest.version);
    println!("  Theme API:   {}", pkg.manifest.theme_api);
    if let Some(ref author) = pkg.manifest.author {
        println!("  Author:      {}", author);
    }

    match pkg.validate(api_version) {
        Ok(()) => {
            println!(
                "\n[PASS] Theme package '{}' is valid for Theme API {}.",
                pkg.manifest.id, api_version
            );
        }
        Err(err) => {
            eprintln!("\n[FAIL] Validation failed: {}", err);
            std::process::exit(1);
        }
    }
}

pub fn derive_apk_filename(app_name: &str, output: &Path) -> String {
    if let Some((_prefix, sub)) = app_name.rsplit_once(':') {
        let trimmed_sub = sub.trim().replace(' ', "");
        let output_leaf = output
            .file_name()
            .map(|n| n.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !trimmed_sub.is_empty()
            && (output_leaf == trimmed_sub.to_lowercase() || output_leaf == "termina")
        {
            return format!("{}.apk", trimmed_sub);
        }
    }

    let safe: String = app_name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .filter(|c| *c != ' ')
        .collect();

    let trimmed = safe.trim_matches('_');
    if trimmed.is_empty() {
        "app.apk".to_string()
    } else {
        format!("{}.apk", trimmed)
    }
}

fn cmd_probe(args: ProbeArgs) {
    if args.list_devices {
        probe::print_device_inventory(args.config.as_deref());
        return;
    }

    let apk = match args.apk {
        Some(path) => path,
        None => {
            eprintln!("[PROBE-001] Missing APK path. Usage: runtimepack probe <APK> or runtimepack probe --list-devices");
            std::process::exit(1);
        }
    };

    if !apk.is_file() {
        eprintln!("[PROBE-002] APK file not found: {}", apk.display());
        std::process::exit(1);
    }

    let address = match probe::resolve_device(args.device.as_deref(), args.config.as_deref()) {
        Ok(addr) => addr,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    let report = if args.dpad_down == 0 {
        probe::run_probe(&apk, &address, &args.report)
    } else {
        let config = probe::ScenarioConfig {
            dpad_down: args.dpad_down,
            ..probe::ScenarioConfig::default()
        };
        probe::run_probe_with_config(&apk, &address, &args.report, &config)
    };

    let report = match report {
        Ok(rep) => rep,
        Err(e) => {
            eprintln!("Probe failure: {}", e);
            std::process::exit(1);
        }
    };

    if report.overall != "PASS" {
        std::process::exit(1);
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Doctor) => cmd_doctor(),
        Some(Commands::Runtime {
            sub: RuntimeCommands::List,
        }) => cmd_runtime_list(),
        Some(Commands::Inspect {
            path,
            config,
            engine,
        }) => cmd_inspect(&path, config.as_deref(), engine.as_deref()),
        Some(Commands::Explain { path, engine }) => cmd_explain(&path, engine.as_deref()),
        Some(Commands::Theme { sub }) => match sub {
            ThemeCommands::List { catalog } => cmd_theme_list(catalog),
            ThemeCommands::Validate { path, api_version } => {
                cmd_theme_validate(&path, &api_version)
            }
        },
        Some(Commands::Audit(args)) => cmd_audit(args),
        Some(Commands::Build(args)) => cmd_build(args),
        Some(Commands::Probe(args)) => cmd_probe(args),
        Some(Commands::Licenses) => cmd_licenses(),
        Some(Commands::PackBundle(args)) => cmd_pack_bundle(args),
        None => {
            println!("runtimepack 0.1.0");
            println!("Run 'runtimepack --help' for usage.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MEDIUM-4: a successful build prints no findings, so a dropped `.node` addon
    /// a game may `require` used to be invisible to the operator. The notice lines
    /// carry code, path and the packer's reason, and list disclosures only.
    #[test]
    fn pack_excluded_disclosures_are_reported_with_code_path_and_reason() {
        use runtimepack_core::compatibility::CompatibilityIssue;

        let descriptor = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        let mut report = runtimepack_core::compatibility::CompatibilityReport::new(&descriptor);
        report.add_issue(CompatibilityIssue {
            code: DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED.to_string(),
            severity: DiagnosticSeverity::Warning,
            message: "Native candidate 'lib/greenworks-win64.node' is not shipped: the packer \
                      excludes it (EXCLUDED_WINDOWS_PE_BINARY), so the built artifact does not \
                      contain this file."
                .to_string(),
            source_path: Some("lib/greenworks-win64.node".to_string()),
            remediation: None,
        });
        report.add_issue(CompatibilityIssue {
            code: DiagnosticCode::CMP_PACK_EXCLUDED_NATIVE_NOT_SHIPPED.to_string(),
            severity: DiagnosticSeverity::Info,
            message: "Native candidate 'lib/steam_api64.dll' is not shipped: the packer excludes \
                      it (EXCLUDED_PLATFORM_BINARY), so the built artifact does not contain this \
                      file."
                .to_string(),
            source_path: Some("lib/steam_api64.dll".to_string()),
            remediation: None,
        });
        // A blocking native finding is not a disclosure: it must never be listed
        // as "not shipped; game may fail at runtime".
        report.add_issue(CompatibilityIssue {
            code: DiagnosticCode::CMP_NODE_NATIVE_MODULE.to_string(),
            severity: DiagnosticSeverity::Error,
            message: "Unsupported Node.js native binary addon found in payload".to_string(),
            source_path: Some("addons/other.node".to_string()),
            remediation: None,
        });

        let lines = pack_excluded_notice_lines(&report);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(
            lines[0].starts_with("[CMP-112] lib/greenworks-win64.node — "),
            "the addon disclosure names its code and path: {lines:?}"
        );
        assert!(
            lines[0].contains("EXCLUDED_WINDOWS_PE_BINARY"),
            "the addon disclosure states the packer's reason: {lines:?}"
        );
        assert!(
            lines[1].starts_with("[CMP-111] lib/steam_api64.dll — "),
            "the companion disclosure names its code and path: {lines:?}"
        );
        assert!(
            lines[1].contains("EXCLUDED_PLATFORM_BINARY"),
            "the companion disclosure states the packer's reason: {lines:?}"
        );
        assert!(
            !lines.iter().any(|line| line.contains("addons/other.node")),
            "a blocking finding is never listed as a disclosure: {lines:?}"
        );
    }

    #[test]
    fn optional_unknown_default_decision_downgrades_serialized_compatibility() {
        use runtimepack_core::build_policy::{
            decide, BuildPolicy, ClassifiedFinding, FindingClass, ReportStatus, Requiredness,
        };
        let decision = decide(
            BuildPolicy::Default,
            &[ClassifiedFinding {
                code: "CMP-104".into(),
                class: FindingClass::Unknown,
                requiredness: Requiredness::Optional,
                source_path: Some("optional.js".into()),
                message: "optional feature has unresolved behavior".into(),
            }],
            &[],
        );
        assert!(decision.allow);
        assert_eq!(decision.status, ReportStatus::Experimental);
        for initial in [CompatibilityStatus::Compatible, CompatibilityStatus::CompatibleWithShims] {
            assert_eq!(
                serde_json::to_value(decision.compatibility_status(initial)).unwrap(),
                serde_json::json!("UNKNOWN")
            );
        }
        assert_eq!(
            decision.compatibility_status(CompatibilityStatus::UnsupportedDependency),
            CompatibilityStatus::UnsupportedDependency
        );
    }

    #[test]
    fn test_derive_apk_filename() {
        assert_eq!(
            derive_apk_filename("Fear & Hunger 2: Termina", Path::new("dist/termina")),
            "Termina.apk"
        );
        assert_eq!(
            derive_apk_filename("Black Souls", Path::new("dist/black_souls")),
            "BlackSouls.apk"
        );
        assert_eq!(
            derive_apk_filename("Fear & Hunger 2: Termina", Path::new("dist/other")),
            "Fear&Hunger2_Termina.apk"
        );
    }

    #[test]
    fn test_pack_excluded_disclosures_escape_control_characters_including_newline_esc_u2028() {
        use runtimepack_core::compatibility::CompatibilityIssue;

        let descriptor = runtimepack_core::detection::EngineDescriptor {
            id: EngineId::RpgMakerMz,
            confidence: DetectionConfidence::Exact,
            score: 210,
            evidence: vec![],
            conflicts: vec![],
            detected_version: None,
            overridden: false,
            content_root: Some("www".to_string()),
        };
        let mut report = runtimepack_core::compatibility::CompatibilityReport::new(&descriptor);
        let hostile_path = "lib/hostile\n\u{1b}[31m\u{2028}\u{202e}.node";
        let hostile_msg = "Dropping hostile\nline\u{1b}[0m\u{2028}addon";
        report.add_issue(CompatibilityIssue {
            code: DiagnosticCode::CMP_PACK_EXCLUDED_NODE_ADDON_NOT_SHIPPED.to_string(),
            severity: DiagnosticSeverity::Warning,
            message: hostile_msg.to_string(),
            source_path: Some(hostile_path.to_string()),
            remediation: None,
        });

        let lines = pack_excluded_notice_lines(&report);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        assert!(
            !line.contains('\n')
                && !line.contains('\r')
                && !line.contains('\u{1b}')
                && !line.contains('\u{2028}')
                && !line.contains('\u{202e}'),
            "pack-excluded lines must never contain raw control characters: {line:?}"
        );
        assert!(
            line.contains("\\n")
                && line.contains("\\u{1b}")
                && line.contains("\\u{2028}")
                && line.contains("\\u{202e}"),
            "escapes must be visibly rendered: {line}"
        );
        assert!(
            line.starts_with("[CMP-112] lib/hostile\\n"),
            "line format must be preserved: {line}"
        );
    }
}
