use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use crate::diagnostics::{DiagnosticCode, DiagnosticError};

pub const APPROVED_RGSS_JNI_LIBRARIES: &[&str] = &[
    "libc++_shared.so",
    "libmkxp-z.so",
    "libopenal.so",
    "libruby.so",
    "libSDL2.so",
    "libSDL2_image.so",
    "libSDL2_sound.so",
    "libSDL2_ttf.so",
];

pub const APPROVED_RENPY_JNI_LIBRARIES: &[&str] = &[
    "librenpython.so",
];

pub const REQUIRED_RESOURCE_MARKERS: &[&str] = &[
    "android/shell/build.gradle.kts",
    "runtimes/manifests/web-rpgmaker.toml",
    "schemas/build-manifest.schema.json",
    "toolchain.lock",
];

/// Validates whether the given directory contains all mandatory RuntimePack distribution markers.
pub fn is_valid_resource_root(path: &Path) -> bool {
    if !path.is_dir() {
        return false;
    }
    for marker in REQUIRED_RESOURCE_MARKERS {
        let marker_path = marker.split('/').fold(path.to_path_buf(), |acc, part| acc.join(part));
        if !marker_path.is_file() {
            return false;
        }
    }
    true
}

/// Verifies that a child path is structurally and canonically contained within the root path.
/// Prevents directory traversal attacks (`..`) and symlink escapes outside the resource root.
pub fn is_contained_in_root(child: &Path, root: &Path) -> bool {
    // If both paths can be canonicalized on filesystem, enforce canonical prefix containment.
    if let (Ok(canon_root), Ok(canon_child)) = (root.canonicalize(), child.canonicalize()) {
        return canon_child.starts_with(&canon_root);
    }

    // For un-canonicalized or pre-creation paths, perform strict component inspection.
    if child.is_absolute() && !child.starts_with(root) {
        return false;
    }

    let rel = match child.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => return false,
    };

    let mut depth: i32 = 0;
    for comp in rel.components() {
        match comp {
            std::path::Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            std::path::Component::Normal(_) => {
                depth += 1;
            }
            std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                return false;
            }
            _ => {}
        }
    }

    true
}

/// Pure, context-injected resource root resolution helper.
///
/// Precedence:
/// 1. `explicit`: Environment override `RUNTIMEPACK_RESOURCE_ROOT`. Fails closed if invalid (never falls back).
/// 2. `exe`: Executable-relative bundle root. Checks `exe_dir.parent()` (standard `bin/` layout) and `exe_dir` (flat layout).
/// 3. `dev`: Development source root (`CARGO_MANIFEST_DIR`). Only accepted if containing `Cargo.toml` and verified markers.
/// 4. Terminal fail-closed with `BLD-107` (`BLD_RESOURCE_ROOT_MISSING`). Zero CWD probing!
pub fn resolve_resource_root_with_context(
    explicit: Option<&Path>,
    exe: Option<&Path>,
    dev: Option<&Path>,
) -> Result<PathBuf, DiagnosticError> {
    // Tier 1: Explicit Override
    if let Some(explicit_path) = explicit {
        if is_valid_resource_root(explicit_path) {
            return Ok(explicit_path.to_path_buf());
        }
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Configured RUNTIMEPACK_RESOURCE_ROOT '{}' is invalid or missing required markers",
                explicit_path.display()
            ),
        ));
    }

    // Tier 2: Executable-Relative Bundle Root
    if let Some(exe_path) = exe {
        if let Some(bin_dir) = exe_path.parent() {
            // Standard layout: bin/../
            if let Some(bundle_root) = bin_dir.parent() {
                if is_valid_resource_root(bundle_root) {
                    return Ok(bundle_root.to_path_buf());
                }
            }
            // Flat layout: <bundle>/runtimepack.exe
            if is_valid_resource_root(bin_dir) {
                return Ok(bin_dir.to_path_buf());
            }

            // Raw development build inside repo: target/{debug,release}/...
            // Or cross-compilation: target/<triple>/{debug,release}/...
            let mut curr = bin_dir.parent();
            while let Some(ancestor) = curr {
                if ancestor.file_name().and_then(|s| s.to_str()) == Some("target") {
                    if let Some(workspace_dir) = ancestor.parent() {
                        if workspace_dir.join("Cargo.toml").is_file() && is_valid_resource_root(workspace_dir) {
                            return Ok(workspace_dir.to_path_buf());
                        }
                    }
                }
                curr = ancestor.parent();
            }
        }
    }

    // Tier 3: Development Source Fallback
    if let Some(dev_path) = dev {
        // Direct dev dir check (must contain Cargo.toml)
        if dev_path.join("Cargo.toml").is_file() && is_valid_resource_root(dev_path) {
            return Ok(dev_path.to_path_buf());
        }
        // Workspace parent check (e.g. core crate -> workspace root)
        if let Some(parent) = dev_path.parent() {
            if parent.join("Cargo.toml").is_file() && is_valid_resource_root(parent) {
                return Ok(parent.to_path_buf());
            }
        }
    }

    // Terminal Fail-Closed: Never guess CWD!
    Err(DiagnosticError::from_code(
        DiagnosticCode::BLD_RESOURCE_ROOT_MISSING,
        "RuntimePack resource root could not be located. Ensure application is installed correctly or set RUNTIMEPACK_RESOURCE_ROOT.",
    ))
}

/// Resolves the active RuntimePack resource root by inspecting process environment and binary context.
pub fn resolve_resource_root() -> Result<PathBuf, DiagnosticError> {
    let explicit = std::env::var_os("RUNTIMEPACK_RESOURCE_ROOT").map(PathBuf::from);
    let exe = std::env::current_exe().ok();

    // In release mode, dev fallback is disabled per policy:
    // release cannot rescue broken relocated bundle from developer host compile paths
    #[cfg(debug_assertions)]
    let dev = std::env::var_os("CARGO_MANIFEST_DIR").map(PathBuf::from);
    #[cfg(not(debug_assertions))]
    let dev: Option<PathBuf> = None;

    resolve_resource_root_with_context(
        explicit.as_deref(),
        exe.as_deref(),
        dev.as_deref(),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifestFileEntry {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentLicensing {
    pub web: String,
    pub renpy: String,
    pub rgss: String,
    pub product: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePackagingReport {
    pub schema_version: u32,
    pub bundle_version: String,
    pub total_files: usize,
    pub total_bytes: u64,
    pub checksums_path: PathBuf,
    pub manifest_path: PathBuf,
    pub licensing: ComponentLicensing,
    pub entries: Vec<BundleManifestFileEntry>,
}

fn to_hex_string(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{:02x}", b);
    }
    s
}

/// Pre-validates that source workspace root and output directory are strictly disjoint.
/// Rejects ancestor, descendant, identical, or non-empty existing output directories.
pub fn validate_source_output_disjoint(source: &Path, output: &Path) -> Result<(), DiagnosticError> {
    let canon_src = source.canonicalize().map_err(DiagnosticError::Io)?;

    let abs_out = if output.is_absolute() {
        output.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(DiagnosticError::Io)?
            .join(output)
    };

    if abs_out == canon_src || abs_out.starts_with(&canon_src) || canon_src.starts_with(&abs_out) {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Output directory '{}' must be strictly disjoint from source workspace root '{}'",
                output.display(),
                source.display()
            ),
        ));
    }

    if let Ok(canon_out) = output.canonicalize() {
        if canon_out == canon_src || canon_out.starts_with(&canon_src) || canon_src.starts_with(&canon_out) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!(
                    "Output directory '{}' must be strictly disjoint from source workspace root '{}'",
                    output.display(),
                    source.display()
                ),
            ));
        }
        if let Ok(mut rd) = fs::read_dir(&canon_out) {
            if rd.next().is_some() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                    format!(
                        "Output directory '{}' already exists and is not empty; cannot overwrite",
                        output.display()
                    ),
                ));
            }
        }
    }

    Ok(())
}

/// Validates binary framing for executable payload. Accepts only valid ELF or PE binaries.
/// Strictly non-executing and bounds-checked.
pub fn validate_binary_payload(bin_path: &Path) -> Result<(), DiagnosticError> {
    if bin_path.is_symlink() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::ING_SYMLINK_ESCAPE,
            format!("Binary payload '{}' is a symlink, which is prohibited", bin_path.display()),
        ));
    }
    if !bin_path.is_file() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TEMPLATE_MISSING,
            format!("Binary payload '{}' does not exist or is not a regular file", bin_path.display()),
        ));
    }

    let meta = fs::metadata(bin_path).map_err(DiagnosticError::Io)?;
    let len = meta.len();
    if len < 64 {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Binary payload '{}' is too small ({} bytes) to be a valid ELF or PE executable",
                bin_path.display(),
                len
            ),
        ));
    }

    let mut file = fs::File::open(bin_path).map_err(DiagnosticError::Io)?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header).map_err(DiagnosticError::Io)?;

    // Check ELF framing
    if &header[0..4] == b"\x7fELF" {
        let ei_class = header[4]; // 1 = 32-bit, 2 = 64-bit
        let ei_data = header[5];  // 1 = Little Endian, 2 = Big Endian
        let ei_version = header[6];
        if (ei_class == 1 || ei_class == 2) && (ei_data == 1 || ei_data == 2) && ei_version == 1 {
            return Ok(());
        }
    }

    // Check PE framing
    if &header[0..2] == b"MZ" {
        let e_lfanew = u32::from_le_bytes([header[0x3C], header[0x3D], header[0x3E], header[0x3F]]) as u64;
        if e_lfanew >= 64 && e_lfanew + 24 <= len {
            file.seek(SeekFrom::Start(e_lfanew)).map_err(DiagnosticError::Io)?;
            let mut pe_buf = [0u8; 6];
            file.read_exact(&mut pe_buf).map_err(DiagnosticError::Io)?;
            if &pe_buf[0..4] == b"PE\0\0" {
                let machine = u16::from_le_bytes([pe_buf[4], pe_buf[5]]);
                // 0x8664 = AMD64, 0x014c = i386, 0xaa64 = ARM64, 0x01c0 = ARM, 0x01c4 = ARMNT
                if machine == 0x8664 || machine == 0x014c || machine == 0xaa64 || machine == 0x01c0 || machine == 0x01c4 {
                    return Ok(());
                }
            }
        }
    }

    Err(DiagnosticError::from_code(
        DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
        format!(
            "Binary payload '{}' does not match valid ELF or PE executable framing",
            bin_path.display()
        ),
    ))
}

/// Packaging options for `package_distribution_bundle_with_options`.
///
/// The default is the distribution contract: RGSS native libraries stay out
/// because their buildable source is unverified (`SOURCE_UNVERIFIED`, see
/// `Docs/REFERENCES.md` S52 and `Docs/LICENSE_COMPLIANCE.md` §3).
#[derive(Debug, Clone, Copy, Default)]
pub struct BundlePackageOptions {
    /// Include the prebuilt `runtimes/rgss/jni/**` libraries and any
    /// `runtimes/rgss/licenses/**` notices, and stamp the bundle with a
    /// `LOCAL_DIAGNOSTIC_ONLY.txt` scope marker.
    ///
    /// This exists so a local tester can build and run RGSS APKs with the
    /// binaries already present in this tree. It does **not** establish
    /// licensing compliance, does not constitute a source offer, and must not
    /// be used for remote distribution.
    pub include_local_diagnostic_native: bool,
}

/// Scope marker written into local-diagnostic bundles.
const LOCAL_DIAGNOSTIC_MARKER: &str = "\
LOCAL_DIAGNOSTIC_ONLY
NO_REMOTE_DISTRIBUTION

This bundle contains prebuilt RGSS native libraries (mkxp-z lineage, SDL2,
Ruby, OpenAL) taken from the local working tree so that RGSS APKs can be built
and tested on this machine.

SOURCE_UNVERIFIED: no matching buildable engine source, no source offer, and no
redistributable third-party notice bundle for these binaries exists in this
tree. Docs/REFERENCES.md S52 records the binary hash/build id and states that
matching buildable source is unverified.

Therefore:
  * this bundle must not be redistributed, uploaded, or published;
  * it does not constitute licensing compliance for the RGSS runtime;
  * a distributable RGSS bundle remains BLOCKED until verified upstream source
    and its notices are obtained.

RuntimePack's own host source is complete in the repository; only the prebuilt
native engine binary cannot be reproduced from this tree.
";

fn is_allowlisted_relative_path_with(rel: &str, include_local_diagnostic_native: bool) -> bool {
    if include_local_diagnostic_native
        && (rel.starts_with("runtimes/rgss/jni/") || rel.starts_with("runtimes/rgss/licenses/"))
    {
        return true;
    }
    is_allowlisted_relative_path(rel)
}

fn is_allowlisted_relative_path(rel: &str) -> bool {
    // Mandatory exclusions
    if rel.starts_with(".git/") || rel == ".git"
        || rel.starts_with(".worktrees/") || rel == ".worktrees"
        || rel.starts_with(".cargo/") || rel == ".cargo"
        || rel.starts_with("target/") || rel == "target"
        || rel.starts_with("build/") || rel == "build"
        || rel.starts_with("dist/") || rel.starts_with("dist_")
        || rel.starts_with("RawGames/") || rel.starts_with("Raw-Games/")
        || rel.ends_with(".keystore") || rel.ends_with(".jks")
        || rel.ends_with("local.properties")
        || rel.contains("/.gradle/") || rel.contains("/app/build/")
        || rel.contains("/build/") || rel.contains("/src/test/")
        || rel.starts_with("android/shell/app/src/main/jniLibs/")
    {
        return false;
    }

    // Root files
    if rel == "toolchain.lock" || rel == "LICENSE" || rel == "NOTICE.txt" || rel == "Docs/LICENSE_COMPLIANCE.md" {
        return true;
    }

    // Android shell
    if rel == "android/shell/build.gradle.kts"
        || rel == "android/shell/settings.gradle.kts"
        || rel == "android/shell/gradle.properties"
        || rel == "android/shell/gradlew"
        || rel == "android/shell/gradlew.bat"
        || rel == "android/shell/gradle/wrapper/gradle-wrapper.jar"
        || rel == "android/shell/gradle/wrapper/gradle-wrapper.properties"
        || rel == "android/shell/app/build.gradle.kts"
        || rel == "android/shell/app/proguard-rules.pro"
        || rel == "android/shell/app/src/main/AndroidManifest.xml"
    {
        return true;
    }
    if rel.starts_with("android/shell/app/src/main/java/") && (rel.ends_with(".kt") || rel.ends_with(".java")) {
        return true;
    }
    if rel.starts_with("android/shell/app/src/main/res/") && (rel.ends_with(".png") || rel.ends_with(".xml")) {
        return true;
    }
    if rel.starts_with("android/shell/app/src/main/assets/") {
        return true;
    }

    // Runtimes
    if rel.starts_with("runtimes/manifests/") && rel.ends_with(".toml") {
        return true;
    }
    if rel.starts_with("runtimes/themes/") && (rel.ends_with(".css") || rel.ends_with(".json")) {
        return true;
    }
    // Ren'Py verified native runtime
    if rel == "runtimes/renpy/manifest.json"
        || (rel.starts_with("runtimes/renpy/licenses/") && !rel.contains("__pycache__") && !rel.ends_with(".pyc"))
        || rel == "runtimes/renpy/jni/arm64-v8a/librenpython.so"
    {
        return true;
    }
    // Ren'Py 7 verified native runtime (versioned, arm64-only). Keep the
    // allowlist positive: only the pinned native path and its notice bundle
    // travel; no wildcard `*.so` for any engine.
    if rel == "runtimes/renpy-7/manifest.json"
        || (rel.starts_with("runtimes/renpy-7/licenses/") && !rel.contains("__pycache__") && !rel.ends_with(".pyc"))
        || rel == "runtimes/renpy-7/jni/arm64-v8a/librenpython.so"
    {
        return true;
    }
    // RGSS native libraries are omitted from public portable bundle per License Gate LOW 7 (GPL source provenance unverified)
    if rel.starts_with("runtimes/vnmaker/licenses/") {
        return true;
    }

    // Profiles
    if rel.starts_with("profiles/games/") && rel.ends_with("/cheats.js") {
        return true;
    }

    // Compatibility DB
    if rel.starts_with("compatibility-db/") && (rel.ends_with(".toml") || (rel.starts_with("compatibility-db/profiles/") && rel.ends_with(".json"))) {
        return true;
    }

    // Schemas
    if rel.starts_with("schemas/") && rel.ends_with(".schema.json") {
        return true;
    }

    false
}

/// Resolves a runtime-relative notice path without allowing absolute paths,
/// `..` segments, backslashes or NUL bytes. A single trailing `/` is tolerated
/// because `Docs/RUNTIME_CONTRACT.md` documents `notice_bundle = "licenses/"`.
///
/// `pub(crate)` so registry tests can assert the same containment rule the
/// notice copy path applies. It is not exported beyond the crate.
pub(crate) fn safe_runtime_relative_path(
    runtime_dir: &Path,
    rel: &str,
) -> Result<PathBuf, DiagnosticError> {
    let rel = rel.trim_end_matches('/');
    if rel.is_empty()
        || rel.starts_with('/')
        || rel.starts_with('\\')
        || rel.contains('\\')
        || rel.contains('\0')
    {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!("Invalid runtime notice path '{}'", rel),
        ));
    }
    let mut path = runtime_dir.to_path_buf();
    for part in rel.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!("Runtime notice path escapes its runtime directory: '{}'", rel),
            ));
        }
        path.push(part);
    }
    if !is_contained_in_root(&path, runtime_dir) {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!("Runtime notice path escapes its runtime directory: '{}'", rel),
        ));
    }
    Ok(path)
}

/// Rejects a path whose existing components (or the path itself) are symlinks.
/// Used so a notice source or destination can never be redirected outside the
/// directory the caller intends to read from or write into.
fn reject_symlink_components(path: &Path) -> Result<(), DiagnosticError> {
    let mut current = Some(path);
    while let Some(candidate) = current {
        match fs::symlink_metadata(candidate) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::ING_SYMLINK_ESCAPE,
                        format!(
                            "Symlink '{}' is prohibited in runtime notice paths",
                            candidate.display()
                        ),
                    ));
                }
            }
            Err(_) => {}
        }
        current = candidate.parent();
    }
    Ok(())
}

/// Upper bound on staging-name retries before the copy fails closed.
const MAX_STAGING_ATTEMPTS: u32 = 8;

/// Monotonic counter so two copies in the same process never propose the same
/// staging name. The value is not a secret: `create_new` is what guarantees
/// exclusivity.
static STAGING_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Builds an unpredictable staging path next to `destination`.
fn staging_candidate(destination: &Path, attempt: u32) -> PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|delta| delta.as_nanos() as u64)
        .unwrap_or(0);
    let counter = STAGING_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = destination
        .file_name()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "notice".to_string());
    destination.with_file_name(format!(
        ".{name}.rtp-staging-{}-{nonce:x}-{counter:x}-{attempt}",
        std::process::id()
    ))
}

/// Opens the source file and pins the opened inode, so a swap between the
/// symlink check and the read cannot redirect the copy to another file.
fn open_verified_source(source: &Path) -> Result<fs::File, DiagnosticError> {
    let before = fs::symlink_metadata(source).map_err(DiagnosticError::Io)?;
    if before.file_type().is_symlink() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::ING_SYMLINK_ESCAPE,
            format!(
                "Symlink '{}' is prohibited as a runtime notice source",
                source.display()
            ),
        ));
    }
    if !before.is_file() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Runtime notice source '{}' is not a regular file",
                source.display()
            ),
        ));
    }

    let file = fs::File::open(source).map_err(DiagnosticError::Io)?;
    let opened = file.metadata().map_err(DiagnosticError::Io)?;
    if !opened.is_file() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Runtime notice source '{}' is not a regular file",
                source.display()
            ),
        ));
    }

    // Re-inspect the path and require it to still name the inode we opened.
    let after = fs::symlink_metadata(source).map_err(DiagnosticError::Io)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let stable = !after.file_type().is_symlink()
            && before.dev() == after.dev()
            && before.ino() == after.ino()
            && opened.dev() == after.dev()
            && opened.ino() == after.ino();
        if !stable {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!(
                    "Runtime notice source '{}' changed while it was being opened; refusing to copy it",
                    source.display()
                ),
            ));
        }
    }
    #[cfg(not(unix))]
    if after.file_type().is_symlink() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::ING_SYMLINK_ESCAPE,
            format!(
                "Symlink '{}' is prohibited as a runtime notice source",
                source.display()
            ),
        ));
    }

    Ok(file)
}

/// Creates a brand-new staging file with `create_new` semantics.
///
/// `create_new` fails atomically when the path already exists — including when it
/// exists as a symlink — so a pre-populated output tree can never redirect the
/// staged bytes. Collisions only advance to the next nonce; nothing pre-existing
/// is opened, truncated, followed or removed.
fn create_staging_file(destination: &Path) -> Result<(PathBuf, fs::File), DiagnosticError> {
    for attempt in 0..MAX_STAGING_ATTEMPTS {
        let candidate = staging_candidate(destination, attempt);
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(DiagnosticError::Io(err)),
        }
    }

    Err(DiagnosticError::from_code(
        DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
        format!(
            "Could not create a unique staging file next to '{}' after {} attempts; \
             refusing to reuse a pre-existing entry",
            destination.display(),
            MAX_STAGING_ATTEMPTS
        ),
    ))
}

/// Removes a staging file only when the path still names the inode we created.
/// A replaced or symlinked entry is left untouched (it is not our artifact).
fn remove_own_staging(staging: &Path, dev: u64, ino: u64) {
    let Ok(meta) = fs::symlink_metadata(staging) else {
        return;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.file_type().is_symlink() || meta.dev() != dev || meta.ino() != ino {
            return;
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (dev, ino);
        if meta.file_type().is_symlink() {
            return;
        }
    }
    let _ = fs::remove_file(staging);
}

/// Copies `source` to `destination` with create-new semantics.
///
/// Both endpoints are inspected before any write, so a symlink at the exact source
/// or destination file is rejected instead of followed — an attacker-controlled or
/// pre-populated output tree must never redirect a notice write outside the output
/// directory.
///
/// The bytes are streamed into a freshly created staging file (`create_new`, unique
/// nonce), flushed and validated, then published with `hard_link`, which fails
/// atomically if `destination` already exists. An existing file (user artifact or a
/// previous run) is therefore never overwritten; the staged file is removed only
/// after a successful publish, and only while it is still the inode we created.
fn copy_new_file(source: &Path, destination: &Path) -> Result<(), DiagnosticError> {
    let source_file = open_verified_source(source)?;
    let source_len = source_file.metadata().map_err(DiagnosticError::Io)?.len();

    match fs::symlink_metadata(destination) {
        Ok(meta) => {
            let code = if meta.file_type().is_symlink() {
                DiagnosticCode::ING_SYMLINK_ESCAPE
            } else {
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID
            };
            return Err(DiagnosticError::from_code(
                code,
                format!(
                    "Refusing to overwrite existing notice destination '{}'. The output directory \
                     already holds a previous build; choose a NEW --output directory. The existing \
                     artifact is preserved unchanged.",
                    destination.display()
                ),
            ));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(DiagnosticError::Io(err)),
    }

    let (staging_path, mut staging_file) = create_staging_file(destination)?;
    let staging_meta = staging_file.metadata().map_err(DiagnosticError::Io)?;
    #[cfg(unix)]
    let (staging_dev, staging_ino) = {
        use std::os::unix::fs::MetadataExt;
        (staging_meta.dev(), staging_meta.ino())
    };
    #[cfg(not(unix))]
    let (staging_dev, staging_ino) = (0u64, 0u64);

    let result = (|| -> Result<(), DiagnosticError> {
        let mut reader = source_file;
        std::io::copy(&mut reader, &mut staging_file).map_err(DiagnosticError::Io)?;
        staging_file.flush().map_err(DiagnosticError::Io)?;
        staging_file.sync_all().map_err(DiagnosticError::Io)?;
        let written = staging_file.metadata().map_err(DiagnosticError::Io)?.len();
        if written != source_len {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Staged copy of '{}' is {} bytes but the source is {} bytes",
                    destination.display(),
                    written,
                    source_len
                ),
            ));
        }
        fs::hard_link(&staging_path, destination).map_err(|err| {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                    format!(
                        "Refusing to overwrite existing notice destination '{}'. The output directory \
                         already holds a previous build; choose a NEW --output directory. The existing \
                         artifact is preserved unchanged.",
                        destination.display()
                    ),
                )
            } else {
                DiagnosticError::Io(err)
            }
        })?;
        Ok(())
    })();

    drop(staging_file);
    remove_own_staging(&staging_path, staging_dev, staging_ino);
    result
}

fn copy_runtime_notice_tree(
    runtime_dir: &Path,
    source_dir: &Path,
    destination: &Path,
    destination_root: &Path,
) -> Result<usize, DiagnosticError> {
    let mut copied = 0usize;
    for entry in fs::read_dir(source_dir).map_err(DiagnosticError::Io)? {
        let entry = entry.map_err(DiagnosticError::Io)?;
        let path = entry.path();
        if path.is_symlink() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_SYMLINK_ESCAPE,
                format!(
                    "Symlink '{}' is prohibited in runtime notice input",
                    path.display()
                ),
            ));
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name == "__pycache__" {
                continue;
            }
            copied += copy_runtime_notice_tree(runtime_dir, &path, destination, destination_root)?;
        } else if path.is_file() {
            if name.ends_with(".pyc") {
                continue;
            }
            let rel = path.strip_prefix(runtime_dir).map_err(|_| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                    "Runtime notice file is outside its runtime directory",
                )
            })?;
            let dst = rel
                .components()
                .fold(destination.to_path_buf(), |acc, part| acc.join(part.as_os_str()));
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(DiagnosticError::Io)?;
                // Destination containment: a symlinked subdirectory inside the
                // destination must not redirect the write outside it.
                let canon_parent = parent.canonicalize().map_err(DiagnosticError::Io)?;
                if !canon_parent.starts_with(destination_root) {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::ING_SYMLINK_ESCAPE,
                        format!(
                            "Notice destination '{}' escapes '{}'",
                            parent.display(),
                            destination_root.display()
                        ),
                    ));
                }
            }
            copy_new_file(&path, &dst)?;
            copied += 1;        }
    }
    Ok(copied)
}

/// Resolves the notice directory for a runtime and copies it, together with the
/// runtime's `manifest.json`, into `destination`.
///
/// Resolution rules (engine-agnostic, data-driven):
/// 1. `expected_bundle` — the registry-declared `notice_bundle` — must agree with
///    the runtime's own `manifest.json` when both are present; disagreement is a
///    hard error so the two descriptors cannot drift silently.
/// 2. The declared bundle is resolved inside `runtime_dir` and must be a real
///    directory, not a symlink, and canonically contained in `runtime_dir`.
/// 3. Legacy descriptors that point at a non-existent `<dir>/<engine>` path fall
///    back to the sibling `licenses/` directory under the *same* checks.
/// 4. Only regular files are copied; symlinks, `__pycache__` and `.pyc` are
///    rejected or skipped.
///
/// Returns the number of files copied. Zero means the runtime has no notice
/// bundle in this resource root; callers that require notices must fail closed
/// rather than treating zero as success.
pub fn copy_runtime_notice_bundle(
    runtime_dir: &Path,
    destination: &Path,
    expected_bundle: Option<&str>,
) -> Result<usize, DiagnosticError> {
    if !runtime_dir.is_dir() {
        return Ok(0);
    }

    let manifest_path = runtime_dir.join("manifest.json");
    let mut declared_bundle: Option<String> = None;
    if manifest_path.is_file() {
        let text = fs::read_to_string(&manifest_path).map_err(DiagnosticError::Io)?;
        let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!(
                    "Runtime manifest '{}' is not valid JSON: {}",
                    manifest_path.display(),
                    e
                ),
            )
        })?;
        if let Some(bundle) = value.get("notice_bundle").and_then(|v| v.as_str()) {
            declared_bundle = Some(bundle.to_string());
        }
    }

    if let (Some(expected), Some(declared)) = (expected_bundle, declared_bundle.as_deref()) {
        let normalize = |value: &str| value.trim_end_matches('/').to_string();
        if normalize(expected) != normalize(declared) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!(
                    "Runtime notice descriptors disagree: registry '{}' vs manifest '{}' in '{}'",
                    expected,
                    declared,
                    runtime_dir.display()
                ),
            ));
        }
    }

    let mut notice_dir = None;
    if let Some(rel) = declared_bundle.as_deref() {
        let candidate = safe_runtime_relative_path(runtime_dir, rel)?;
        if candidate.is_dir() {
            if candidate.is_symlink() || !is_contained_in_root(&candidate, runtime_dir) {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_SYMLINK_ESCAPE,
                    format!(
                        "Declared notice bundle '{}' is not a contained real directory",
                        candidate.display()
                    ),
                ));
            }
            notice_dir = Some(candidate);
        }
    }
    if notice_dir.is_none() {
        // Legacy fallback: same containment and symlink checks as a declared path.
        let fallback = safe_runtime_relative_path(runtime_dir, "licenses")?;
        if fallback.is_dir() {
            if fallback.is_symlink() || !is_contained_in_root(&fallback, runtime_dir) {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_SYMLINK_ESCAPE,
                    format!(
                        "Legacy notice directory '{}' is not a contained real directory",
                        fallback.display()
                    ),
                ));
            }
            notice_dir = Some(fallback);
        }
    }

    let Some(notice_dir) = notice_dir else {
        return Ok(0);
    };

    fs::create_dir_all(destination).map_err(DiagnosticError::Io)?;
    reject_symlink_components(destination)?;
    let destination_root = destination.canonicalize().map_err(DiagnosticError::Io)?;

    let mut copied = 0usize;
    if manifest_path.is_file() {
        let dst = destination.join("manifest.json");
        copy_new_file(&manifest_path, &dst)?;
        copied += 1;
    }
    copied += copy_runtime_notice_tree(
        runtime_dir,
        &notice_dir,
        destination,
        &destination_root,
    )?;
    Ok(copied)
}

/// Registry-declared notice contract for a runtime id:
/// `(notice_bundle, required_notice_files)`.
///
/// The runtime id is authoritative: the CLI only ever selects runtime ids that
/// came from this registry, so an unregistered id has no notice metadata (and is
/// rejected earlier during runtime resolution, not here).
pub fn runtime_notice_requirements(runtime_id: &str) -> (Option<String>, Vec<String>) {
    crate::runtime_registry::RuntimeRegistry::new()
        .get(runtime_id)
        .and_then(|runtime| runtime.licenses.clone())
        .map(|licenses| (licenses.notice_bundle, licenses.required_notice_files))
        .unwrap_or((None, Vec::new()))
}

/// Resolves the runtime directory declared by the runtime registry
/// (`artifacts.android_module`), confined to `resource_root`.
pub fn resolve_runtime_module_dir(resource_root: &Path, runtime_id: &str) -> Option<PathBuf> {
    let rel = crate::runtime_registry::RuntimeRegistry::new()
        .get(runtime_id)
        .and_then(|runtime| runtime.artifacts.as_ref()?.android_module.clone())?;
    let path = safe_runtime_relative_path(resource_root, &rel).ok()?;
    if path.is_dir() {
        Some(path)
    } else {
        None
    }
}

/// Writes the selected runtime's license/provenance files into the durable
/// build output at `third-party-notices/<runtime_id>/`.
///
/// This is the same source and the same copy routine the generated Android
/// project uses for its APK assets, so durable notices and shipped notices
/// cannot drift. Returns the number of files written; `0` means the selected
/// runtime has no notice bundle in this resource root and nothing was created.
/// A runtime with registry-declared `required_notice_files` fails closed when
/// the bundle or any required file is missing.
pub fn write_durable_runtime_notices(
    output_dir: &Path,
    runtime_id: &str,
    resource_root: &Path,
) -> Result<usize, DiagnosticError> {
    if runtime_id.is_empty()
        || !runtime_id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!("Invalid runtime id '{}' for durable notices", runtime_id),
        ));
    }

    let (declared_bundle, required_files) = runtime_notice_requirements(runtime_id);
    let runtime_dir = resolve_runtime_module_dir(resource_root, runtime_id);

    let Some(runtime_dir) = runtime_dir else {
        if required_files.is_empty() {
            return Ok(0);
        }
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TEMPLATE_MISSING,
            format!(
                "Runtime '{}' requires notice files but its runtime module was not found in '{}'",
                runtime_id,
                resource_root.display()
            ),
        ));
    };

    let notices_root = output_dir.join("third-party-notices");
    fs::create_dir_all(&notices_root).map_err(DiagnosticError::Io)?;
    reject_symlink_components(&notices_root)?;
    let destination = notices_root.join(runtime_id);
    if destination.exists() && destination.is_symlink() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::ING_SYMLINK_ESCAPE,
            format!(
                "Durable notice destination '{}' is a symlink",
                destination.display()
            ),
        ));
    }

    let copied = copy_runtime_notice_bundle(&runtime_dir, &destination, declared_bundle.as_deref())?;

    for required in &required_files {
        let path = safe_runtime_relative_path(&destination, required)?;
        if !path.is_file() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_TEMPLATE_MISSING,
                format!(
                    "Runtime '{}' is missing required notice file '{}'",
                    runtime_id, required
                ),
            ));
        }
    }

    Ok(copied)
}

/// Checksum lines (`<sha256>  <path>`, relative to `output_dir`) for every file
/// under `<output_dir>/third-party-notices`, sorted.
///
/// Every failure is returned as an error: an unreadable directory, an
/// unlistable entry, a symlink, or an unreadable file must never be silently
/// omitted from an integrity list.
pub fn notice_checksum_lines(output_dir: &Path) -> Result<Vec<String>, DiagnosticError> {
    fn walk(base: &Path, current: &Path, lines: &mut Vec<String>) -> Result<(), DiagnosticError> {
        let entries = fs::read_dir(current).map_err(|err| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Cannot read notice directory '{}' while building checksums: {}",
                    current.display(),
                    err
                ),
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!(
                        "Cannot list an entry of '{}' while building checksums: {}",
                        current.display(),
                        err
                    ),
                )
            })?;
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|err| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!(
                        "Cannot inspect '{}' while building checksums: {}",
                        path.display(),
                        err
                    ),
                )
            })?;
            if meta.file_type().is_symlink() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::ING_SYMLINK_ESCAPE,
                    format!(
                        "Symlink '{}' is prohibited in the durable notice tree",
                        path.display()
                    ),
                ));
            }
            if meta.is_dir() {
                walk(base, &path, lines)?;
            } else if meta.is_file() {
                let bytes = fs::read(&path).map_err(|err| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                        format!(
                            "Cannot read '{}' while building checksums: {}",
                            path.display(),
                            err
                        ),
                    )
                })?;
                let rel = path.strip_prefix(base).map_err(|_| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                        format!("Notice file '{}' is outside '{}'", path.display(), base.display()),
                    )
                })?;
                let rel = rel.to_string_lossy().replace('\\', "/");
                lines.push(format!("{:x}  {}", Sha256::digest(&bytes), rel));
            }
        }
        Ok(())
    }

    let mut lines = Vec::new();
    let root = output_dir.join("third-party-notices");
    if root.is_dir() {
        walk(output_dir, &root, &mut lines)?;
        lines.sort();
    }
    // An experimental build's notice is a shipped artifact: its bytes are what the
    // manifest digest and the APK asset describe, so the integrity list covers it.
    // Default builds have no such file and therefore no entry.
    let experimental_notice = output_dir.join("EXPERIMENTAL-NOTICE.txt");
    if experimental_notice.is_file() {
        let bytes = fs::read(&experimental_notice).map_err(DiagnosticError::Io)?;
        lines.push(format!("{:x}  EXPERIMENTAL-NOTICE.txt", Sha256::digest(&bytes)));
    }
    Ok(lines)
}

/// Merges the durable notice checksums into the existing `checksums.txt` body.
///
/// Fails closed when the baseline list is missing, unreadable or empty: an
/// integrity list that does not cover the published artifact must never be
/// written and reported as success.
pub fn merge_notice_checksums(output_dir: &Path) -> Result<String, DiagnosticError> {
    let path = output_dir.join("checksums.txt");
    let existing = fs::read_to_string(&path).map_err(|err| {
        DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!(
                "Baseline checksums.txt is missing or unreadable at '{}': {}",
                path.display(),
                err
            ),
        )
    })?;

    let mut lines: Vec<String> = existing
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.to_string())
        .collect();
    if lines.is_empty() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!(
                "Baseline checksums.txt at '{}' is empty; refusing to publish an incomplete integrity list",
                path.display()
            ),
        ));
    }

    for line in notice_checksum_lines(output_dir)? {
        if !lines.contains(&line) {
            lines.push(line);
        }
    }
    lines.sort();
    let mut text = lines.join("\n");
    text.push('\n');
    Ok(text)
}

fn collect_allowlisted_files(
    workspace_root: &Path,
    current_dir: &Path,
    _canon_root: &Path,
    collected: &mut Vec<(String, PathBuf)>,
    options: &BundlePackageOptions,
) -> Result<(), DiagnosticError> {
    let entries = fs::read_dir(current_dir).map_err(DiagnosticError::Io)?;
    for entry in entries {
        let entry = entry.map_err(DiagnosticError::Io)?;
        let path = entry.path();

        // Strict Symlink Rejection: ALL symlinks are prohibited in distribution resource input
        if path.is_symlink() || entry.file_type().map_or(false, |ft| ft.is_symlink()) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_SYMLINK_ESCAPE,
                format!(
                    "Symlink '{}' is strictly prohibited in distribution resource input",
                    path.display()
                ),
            ));
        }

        let rel = path
            .strip_prefix(workspace_root)
            .map_err(|_| DiagnosticError::from_code(DiagnosticCode::CNT_TRAVERSAL_REJECTED, "Path outside workspace"))?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        // Skip ignored directories early
        if path.is_dir() {
            if rel_str == ".git" || rel_str.starts_with(".git/")
                || rel_str == ".worktrees" || rel_str.starts_with(".worktrees/")
                || rel_str == ".cargo" || rel_str.starts_with(".cargo/")
                || rel_str == "target" || rel_str.starts_with("target/")
                || rel_str == "build" || rel_str.starts_with("build/")
                || rel_str.starts_with("dist")
                || rel_str == "RawGames" || rel_str == "Raw-Games"
                || rel_str == "android/shell/app/src/main/jniLibs"
                || rel_str.contains("/.gradle")
                || rel_str.contains("/app/build")
                || rel_str.contains("/src/test")
            {
                continue;
            }
            collect_allowlisted_files(workspace_root, &path, _canon_root, collected, options)?;
        } else if path.is_file() {
            if is_allowlisted_relative_path_with(&rel_str, options.include_local_diagnostic_native) {
                collected.push((rel_str, path));
            }
        }
    }
    Ok(())
}

/// Verifies SHA-256 integrity of all files listed in checksums.txt within root.
pub fn verify_bundle_checksums(root: &Path) -> Result<usize, DiagnosticError> {
    let checksums_path = root.join("checksums.txt");
    if !checksums_path.is_file() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            "checksums.txt missing from bundle",
        ));
    }

    let content = fs::read_to_string(&checksums_path).map_err(DiagnosticError::Io)?;
    let mut verified_count = 0;
    let mut seen_paths = HashSet::new();

    for (line_no, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let expected_sha = match parts.next() {
            Some(s) => s,
            None => continue,
        };
        let rel_path = parts.collect::<Vec<_>>().join(" ");
        if rel_path.is_empty() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!("Malformed checksum entry at line {}", line_no + 1),
            ));
        }

        // Strict 64 hex characters
        if expected_sha.len() != 64 || !expected_sha.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!("Invalid SHA-256 digest format at line {}: '{}'", line_no + 1, expected_sha),
            ));
        }

        // Strict normalized relative path check
        if rel_path.starts_with('/') || rel_path.starts_with('\\')
            || rel_path.contains("..") || rel_path.contains('\0')
            || (rel_path.len() >= 2 && rel_path.chars().nth(1) == Some(':'))
            || rel_path.starts_with("//") || rel_path.starts_with("\\\\")
        {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!("Path traversal or non-relative path in checksums: '{}'", rel_path),
            ));
        }

        // No self-hash
        if rel_path == "checksums.txt" {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                "Cyclic self-hash of checksums.txt is not permitted",
            ));
        }

        // Case-folded collision check
        let path_key = rel_path.to_ascii_lowercase();
        if !seen_paths.insert(path_key) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
                format!("Duplicate or case-colliding entry in checksums: '{}'", rel_path),
            ));
        }

        let target_file = rel_path.split('/').fold(root.to_path_buf(), |acc, p| acc.join(p));
        if !target_file.is_file() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_INTEGRITY_MISMATCH,
                format!("Checksummed file missing: '{}'", rel_path),
            ));
        }

        if target_file.is_symlink() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::ING_SYMLINK_ESCAPE,
                format!("Checksummed file is a symlink: '{}'", rel_path),
            ));
        }

        let mut data = Vec::new();
        fs::File::open(&target_file)
            .and_then(|mut f| f.read_to_end(&mut data))
            .map_err(DiagnosticError::Io)?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let computed_sha = to_hex_string(&hasher.finalize());

        if !computed_sha.eq_ignore_ascii_case(expected_sha) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_INTEGRITY_MISMATCH,
                format!(
                    "Integrity mismatch for '{}': expected {}, computed {}",
                    rel_path, expected_sha, computed_sha
                ),
            ));
        }

        verified_count += 1;
    }

    if verified_count == 0 {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            "checksums.txt contains 0 valid file entries",
        ));
    }

    Ok(verified_count)
}

/// Packages a portable RuntimePack distribution bundle according to the positive allowlist specification.
pub fn package_distribution_bundle(
    workspace_root: &Path,
    output_dir: &Path,
    bin_path: Option<&Path>,
) -> Result<BundlePackagingReport, DiagnosticError> {
    package_distribution_bundle_with_options(
        workspace_root,
        output_dir,
        bin_path,
        &BundlePackageOptions::default(),
    )
}

/// Packages a bundle with explicit options. See `BundlePackageOptions`.
pub fn package_distribution_bundle_with_options(
    workspace_root: &Path,
    output_dir: &Path,
    bin_path: Option<&Path>,
    options: &BundlePackageOptions,
) -> Result<BundlePackagingReport, DiagnosticError> {
    if !is_valid_resource_root(workspace_root) {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            format!(
                "Workspace root '{}' is missing mandatory RuntimePack distribution markers",
                workspace_root.display()
            ),
        ));
    }

    // 1. Pre-validate disjointness BEFORE creating any directory
    validate_source_output_disjoint(workspace_root, output_dir)?;

    // 2. Validate binary payload framing if provided
    if let Some(src_bin) = bin_path {
        validate_binary_payload(src_bin)?;
    }

    let canon_root = workspace_root.canonicalize().map_err(DiagnosticError::Io)?;

    let mut files_to_copy = Vec::new();
    collect_allowlisted_files(workspace_root, workspace_root, &canon_root, &mut files_to_copy, options)?;

    fs::create_dir_all(output_dir).map_err(DiagnosticError::Io)?;

    let mut copied_rel_paths = HashSet::new();
    let mut manifest_entries = Vec::new();
    let mut checksum_lines = Vec::new();
    let mut total_bytes: u64 = 0;

    // Optional binary packaging into bin/
    if let Some(src_bin) = bin_path {
        let bin_name = src_bin.file_name().unwrap().to_string_lossy().to_string();
        let rel_bin_path = format!("bin/{}", bin_name);
        let dst_bin = output_dir.join("bin").join(&bin_name);

        if let Some(parent) = dst_bin.parent() {
            fs::create_dir_all(parent).map_err(DiagnosticError::Io)?;
        }

        let mut data = Vec::new();
        fs::File::open(src_bin)
            .and_then(|mut f| f.read_to_end(&mut data))
            .map_err(DiagnosticError::Io)?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256_hex = to_hex_string(&hasher.finalize());
        let size = data.len() as u64;

        fs::write(&dst_bin, &data).map_err(DiagnosticError::Io)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dst_bin).map_err(DiagnosticError::Io)?.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(&dst_bin, perms);
        }

        copied_rel_paths.insert(rel_bin_path.clone());
        total_bytes += size;
        checksum_lines.push((rel_bin_path.clone(), sha256_hex.clone()));
        manifest_entries.push(BundleManifestFileEntry {
            path: rel_bin_path,
            sha256: sha256_hex,
            size_bytes: size,
        });
    }

    // Copy allowlisted files
    for (rel_path, src_path) in files_to_copy {
        if copied_rel_paths.contains(&rel_path) {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::CNT_CORRUPTED_PACK,
                format!("Copy collision detected for relative bundle path '{}'", rel_path),
            ));
        }

        let dst_path = rel_path
            .split('/')
            .fold(output_dir.to_path_buf(), |acc, part| acc.join(part));

        if let Some(parent) = dst_path.parent() {
            fs::create_dir_all(parent).map_err(DiagnosticError::Io)?;
        }

        let mut data = Vec::new();
        fs::File::open(&src_path)
            .and_then(|mut f| f.read_to_end(&mut data))
            .map_err(DiagnosticError::Io)?;

        let mut hasher = Sha256::new();
        hasher.update(&data);
        let sha256_hex = to_hex_string(&hasher.finalize());
        let size = data.len() as u64;

        fs::write(&dst_path, &data).map_err(DiagnosticError::Io)?;

        #[cfg(unix)]
        {
            if rel_path == "android/shell/gradlew" {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(meta) = fs::metadata(&dst_path) {
                    let mut perms = meta.permissions();
                    perms.set_mode(0o755);
                    let _ = fs::set_permissions(&dst_path, perms);
                }
            }
        }

        copied_rel_paths.insert(rel_path.clone());
        total_bytes += size;
        checksum_lines.push((rel_path.clone(), sha256_hex.clone()));
        manifest_entries.push(BundleManifestFileEntry {
            path: rel_path,
            sha256: sha256_hex,
            size_bytes: size,
        });
    }

    // Scope marker for explicitly local-diagnostic bundles. Written before the
    // manifest/checksums so it is covered by the bundle's own integrity data.
    if options.include_local_diagnostic_native {
        let marker_path = output_dir.join("LOCAL_DIAGNOSTIC_ONLY.txt");
        fs::write(&marker_path, LOCAL_DIAGNOSTIC_MARKER.as_bytes())
            .map_err(DiagnosticError::Io)?;
        let marker_bytes = LOCAL_DIAGNOSTIC_MARKER.as_bytes();
        let mut marker_hasher = Sha256::new();
        marker_hasher.update(marker_bytes);
        let marker_sha = to_hex_string(&marker_hasher.finalize());
        let marker_size = marker_bytes.len() as u64;
        total_bytes += marker_size;
        checksum_lines.push(("LOCAL_DIAGNOSTIC_ONLY.txt".to_string(), marker_sha.clone()));
        manifest_entries.push(BundleManifestFileEntry {
            path: "LOCAL_DIAGNOSTIC_ONLY.txt".to_string(),
            sha256: marker_sha,
            size_bytes: marker_size,
        });
    }

    // Sort entries deterministically
    checksum_lines.sort_by(|a, b| a.0.cmp(&b.0));
    manifest_entries.sort_by(|a, b| a.path.cmp(&b.path));

    // Write ASSETS_CHECKSUM_MANIFEST.json FIRST so checksums.txt can cover it deterministically
    let total_files = manifest_entries.len();
    let report = BundlePackagingReport {
        schema_version: 1,
        bundle_version: "0.1.0".to_string(),
        total_files,
        total_bytes,
        checksums_path: output_dir.join("checksums.txt"),
        manifest_path: output_dir.join("ASSETS_CHECKSUM_MANIFEST.json"),
        licensing: ComponentLicensing {
            web: "MIT OR Apache-2.0".to_string(),
            renpy: "MIT / LGPL (RAPT 8.3.7 pinned in S09)".to_string(),
            rgss: "RESTRICTED_OMITTED (GPL source provenance unverified for public redistribution)".to_string(),
            product: "UNKNOWN (pending canonical license audit)".to_string(),
        },
        entries: manifest_entries,
    };

    let manifest_json = serde_json::to_string_pretty(&report)
        .map_err(|e| DiagnosticError::Serialization(e.to_string()))?;
    fs::write(&report.manifest_path, manifest_json).map_err(DiagnosticError::Io)?;

    // Append ASSETS_CHECKSUM_MANIFEST.json to checksum_lines
    let manifest_bytes = fs::read(&report.manifest_path).map_err(DiagnosticError::Io)?;
    let mut m_hasher = Sha256::new();
    m_hasher.update(&manifest_bytes);
    let m_sha = to_hex_string(&m_hasher.finalize());
    checksum_lines.push(("ASSETS_CHECKSUM_MANIFEST.json".to_string(), m_sha));
    checksum_lines.sort_by(|a, b| a.0.cmp(&b.0));

    // Write checksums.txt
    let mut checksums_content = String::new();
    for (rel_path, sha) in &checksum_lines {
        checksums_content.push_str(&format!("{}  {}\n", sha, rel_path));
    }
    fs::write(&report.checksums_path, checksums_content).map_err(DiagnosticError::Io)?;

    // Verify self-consistency using verify_bundle_checksums
    verify_bundle_checksums(output_dir)?;

    // Validate emitted bundle
    if !is_valid_resource_root(output_dir) {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_RESOURCE_ROOT_INVALID,
            "Emitted bundle failed post-packaging resource root validation",
        ));
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static TEST_COUNTER: AtomicUsize = AtomicUsize::new(1);

    fn create_mock_bundle_root(prefix: &str) -> PathBuf {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("rtp_test_{}_{}_{}", prefix, std::process::id(), n));
        let _ = fs::create_dir_all(&dir);

        // Populate required markers
        let shell_dir = dir.join("android/shell");
        let runtimes_dir = dir.join("runtimes/manifests");
        let schemas_dir = dir.join("schemas");
        fs::create_dir_all(&shell_dir).unwrap();
        fs::create_dir_all(&runtimes_dir).unwrap();
        fs::create_dir_all(&schemas_dir).unwrap();

        fs::write(shell_dir.join("build.gradle.kts"), b"// shell").unwrap();
        fs::write(runtimes_dir.join("web-rpgmaker.toml"), b"# manifest").unwrap();
        fs::write(schemas_dir.join("build-manifest.schema.json"), b"{}").unwrap();
        fs::write(dir.join("toolchain.lock"), b"# lock").unwrap();

        dir
    }

    #[test]
    fn test_explicit_override_valid() {
        let bundle = create_mock_bundle_root("explicit_ok");
        let res = resolve_resource_root_with_context(Some(&bundle), None, None);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), bundle);
    }

    #[test]
    fn test_explicit_override_invalid_fails_closed() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let invalid_dir = std::env::temp_dir().join(format!("rtp_invalid_bundle_{}_{}", std::process::id(), n));
        let _ = fs::create_dir_all(&invalid_dir);

        let res = resolve_resource_root_with_context(Some(&invalid_dir), None, None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_INVALID));
    }

    #[test]
    fn test_executable_relative_standard_layout() {
        let bundle = create_mock_bundle_root("exe_standard");
        let bin_dir = bundle.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let exe_path = bin_dir.join("runtimepack");
        fs::write(&exe_path, b"dummy").unwrap();

        let res = resolve_resource_root_with_context(None, Some(&exe_path), None);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), bundle);
    }

    #[test]
    fn test_executable_relative_flat_layout() {
        let bundle = create_mock_bundle_root("exe_flat");
        let exe_path = bundle.join("runtimepack.exe");
        fs::write(&exe_path, b"dummy").unwrap();

        let res = resolve_resource_root_with_context(None, Some(&exe_path), None);
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), bundle);
    }

    #[test]
    fn test_cwd_spoof_isolation() {
        // If explicit, exe, and dev are None, resolution MUST fail even if markers might exist in CWD
        let res = resolve_resource_root_with_context(None, None, None);
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_MISSING));
    }

    #[test]
    fn test_dev_manifest_fallback_valid() {
        let bundle = create_mock_bundle_root("dev_valid");
        fs::write(bundle.join("Cargo.toml"), b"[workspace]\nmembers = [\"core\"]\n").unwrap();
        let core_crate = bundle.join("core");
        fs::create_dir_all(&core_crate).unwrap();
        fs::write(core_crate.join("Cargo.toml"), b"[package]\nname = \"runtimepack-core\"\n").unwrap();

        // Pass core_crate as CARGO_MANIFEST_DIR: should resolve to workspace parent bundle
        let res = resolve_resource_root_with_context(None, None, Some(&core_crate));
        assert!(res.is_ok());
        assert_eq!(res.unwrap(), bundle);
    }

    #[test]
    fn test_dev_manifest_fallback_without_cargo_toml_fails() {
        let bundle = create_mock_bundle_root("dev_no_cargo");
        // No Cargo.toml written
        let res = resolve_resource_root_with_context(None, None, Some(&bundle));
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_MISSING));
    }

    #[test]
    fn test_symlink_path_containment() {
        let bundle = create_mock_bundle_root("containment");
        let inside = bundle.join("schemas/build-manifest.schema.json");
        assert!(is_contained_in_root(&inside, &bundle));

        let traversal = bundle.join("../escaping_secret.txt");
        assert!(!is_contained_in_root(&traversal, &bundle));

        let deep_traversal = bundle.join("schemas/../../escaping_secret.txt");
        assert!(!is_contained_in_root(&deep_traversal, &bundle));
    }

    #[test]
    fn test_package_distribution_bundle_positive_allowlist() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let src_root = create_mock_bundle_root(&format!("src_pack_{}", n));
        let out_root = std::env::temp_dir().join(format!("rtp_out_pack_{}_{}", std::process::id(), n));

        // Create allowed files
        let app_dir = src_root.join("android/shell/app/src/main/java/io/runtimepack/host");
        fs::create_dir_all(&app_dir).unwrap();
        fs::write(app_dir.join("WebActivity.kt"), b"// kotlin activity").unwrap();

        let themes_dir = src_root.join("runtimes/themes/matte");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::write(themes_dir.join("theme.json"), b"{}").unwrap();

        // Create disallowed files that must NOT be packaged
        let jni_dir = src_root.join("android/shell/app/src/main/jniLibs/arm64-v8a");
        fs::create_dir_all(&jni_dir).unwrap();
        fs::write(jni_dir.join("libforbidden.so"), b"forbidden").unwrap();

        let test_dir = src_root.join("android/shell/app/src/test/java");
        fs::create_dir_all(&test_dir).unwrap();
        fs::write(test_dir.join("MockTest.kt"), b"test").unwrap();

        fs::write(src_root.join("local.properties"), b"sdk.dir=/custom").unwrap();

        // Mock ELF binary
        let bin_path = src_root.join("dummy_bin");
        create_mock_elf_binary(&bin_path);

        let report = package_distribution_bundle(&src_root, &out_root, Some(&bin_path))
            .expect("Packaging must succeed");

        assert!(report.total_files >= 6);
        assert!(out_root.join("bin/dummy_bin").is_file());
        assert!(out_root.join("android/shell/app/src/main/java/io/runtimepack/host/WebActivity.kt").is_file());
        assert!(out_root.join("runtimes/themes/matte/theme.json").is_file());
        assert!(out_root.join("checksums.txt").is_file());
        assert!(out_root.join("ASSETS_CHECKSUM_MANIFEST.json").is_file());

        // Assert excluded files are NOT present
        assert!(!out_root.join("android/shell/app/src/main/jniLibs").exists());
        assert!(!out_root.join("android/shell/app/src/test").exists());
        assert!(!out_root.join("local.properties").exists());

        // Assert emitted output passes validation
        assert!(is_valid_resource_root(&out_root));
    }

    fn create_mock_elf_binary(path: &Path) {
        let mut elf = vec![0u8; 64];
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2; // 64-bit
        elf[5] = 1; // little endian
        elf[6] = 1; // version 1
        fs::write(path, elf).unwrap();
    }

    fn create_mock_pe_binary(path: &Path) {
        let mut pe = vec![0u8; 128];
        pe[0..2].copy_from_slice(b"MZ");
        // e_lfanew at 0x3C = 64
        pe[0x3C] = 64;
        pe[64..68].copy_from_slice(b"PE\0\0");
        // machine at 68 = 0x8664 (AMD64)
        pe[68] = 0x64;
        pe[69] = 0x86;
        fs::write(path, pe).unwrap();
    }

    #[test]
    fn test_validate_binary_payload_framing() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp = std::env::temp_dir().join(format!("rtp_bin_val_{}_{}", std::process::id(), n));
        fs::create_dir_all(&temp).unwrap();

        // 1. Valid ELF
        let elf_path = temp.join("valid_elf");
        create_mock_elf_binary(&elf_path);
        assert!(validate_binary_payload(&elf_path).is_ok());

        // 2. Valid PE
        let pe_path = temp.join("valid_pe.exe");
        create_mock_pe_binary(&pe_path);
        assert!(validate_binary_payload(&pe_path).is_ok());

        // 3. Plain text / script
        let txt_path = temp.join("script.sh");
        fs::write(&txt_path, b"#!/bin/bash\necho hello\n").unwrap();
        let res = validate_binary_payload(&txt_path);
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_INVALID));

        // 4. Truncated MZ (missing e_lfanew or PE header)
        let mz_path = temp.join("truncated_mz.exe");
        fs::write(&mz_path, b"MZ\x90\x00truncated without PE header").unwrap();
        let res_mz = validate_binary_payload(&mz_path);
        assert!(res_mz.is_err());
        assert_eq!(res_mz.unwrap_err().code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_INVALID));
    }

    #[test]
    fn test_source_output_disjoint_prevalidation() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let src_root = create_mock_bundle_root(&format!("src_disjoint_{}", n));

        // 1. Output inside source root
        let nested_out = src_root.join("dist/bundle");
        let res_nested = validate_source_output_disjoint(&src_root, &nested_out);
        assert!(res_nested.is_err());
        assert_eq!(res_nested.unwrap_err().code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_INVALID));

        // 2. Source root inside output (ancestor output)
        if let Some(parent) = src_root.parent() {
            let res_parent = validate_source_output_disjoint(&src_root, parent);
            assert!(res_parent.is_err());
            assert_eq!(res_parent.unwrap_err().code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_INVALID));
        }

        // 3. Disjoint output
        let valid_out = std::env::temp_dir().join(format!("rtp_disjoint_ok_{}_{}", std::process::id(), n));
        assert!(validate_source_output_disjoint(&src_root, &valid_out).is_ok());
    }

    #[test]
    fn test_symlink_rejection_in_distribution_input() {
        #[cfg(unix)]
        {
            let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let src_root = create_mock_bundle_root(&format!("src_symlink_{}", n));
            let out_root = std::env::temp_dir().join(format!("rtp_out_symlink_{}_{}", std::process::id(), n));

            // Create a symlink inside the source root pointing to toolchain.lock
            let symlink_dir = src_root.join("android/shell/app/src/main/assets/web_shims");
            fs::create_dir_all(&symlink_dir).unwrap();
            let symlink_path = symlink_dir.join("link.js");
            let target_path = src_root.join("toolchain.lock");
            std::os::unix::fs::symlink(&target_path, &symlink_path).unwrap();

            let res = package_distribution_bundle(&src_root, &out_root, None);
            assert!(res.is_err());
            let err = res.unwrap_err();
            assert_eq!(err.code(), Some(DiagnosticCode::ING_SYMLINK_ESCAPE));
        }
    }

    #[test]
    fn test_package_distribution_refuses_nonempty_output_before_writing() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let src_root = create_mock_bundle_root(&format!("src_nonempty_{}", n));
        let out_root = std::env::temp_dir().join(format!("rtp_out_nonempty_{}_{}", std::process::id(), n));
        fs::create_dir_all(&out_root).unwrap();

        // Pre-existing user content that must survive untouched.
        let sentinel = out_root.join("keep-me.txt");
        let sentinel_bytes = b"do-not-touch".to_vec();
        fs::write(&sentinel, &sentinel_bytes).unwrap();
        let before_entries = fs::read_dir(&out_root).unwrap().count();

        let res = package_distribution_bundle(&src_root, &out_root, None);
        let err = res.expect_err("non-empty output directory must be refused");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_RESOURCE_ROOT_INVALID));
        assert!(
            err.to_string().contains("not empty"),
            "diagnostic must explain the refusal, got: {err}"
        );

        // Destination must be unchanged: same entries, same bytes, nothing added.
        let after_entries = fs::read_dir(&out_root).unwrap().count();
        assert_eq!(
            before_entries, after_entries,
            "refusal must happen before any write into the destination"
        );
        assert_eq!(fs::read(&sentinel).unwrap(), sentinel_bytes);
        assert!(!out_root.join("checksums.txt").exists());
        assert!(!out_root.join("ASSETS_CHECKSUM_MANIFEST.json").exists());
    }

    #[test]
    fn test_local_diagnostic_bundle_includes_rgss_native_with_marker() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let src_root = create_mock_bundle_root(&format!("src_localdiag_{}", n));
        let jni = src_root.join("runtimes/rgss/jni/arm64-v8a");
        fs::create_dir_all(&jni).unwrap();
        fs::write(jni.join("libmkxp-z.so"), b"\x7fELF-local-diag").unwrap();
        let lic = src_root.join("runtimes/rgss/licenses");
        fs::create_dir_all(&lic).unwrap();
        fs::write(lic.join("NOTICE.txt"), b"local diagnostic notice").unwrap();

        // Default (distribution) packaging must still exclude RGSS native libs.
        let dist_out = std::env::temp_dir().join(format!("rtp_out_localdiag_dist_{}_{}", std::process::id(), n));
        package_distribution_bundle(&src_root, &dist_out, None).expect("default packaging must succeed");
        assert!(
            !dist_out.join("runtimes/rgss/jni/arm64-v8a/libmkxp-z.so").exists(),
            "default bundle must keep excluding RGSS native libraries"
        );
        assert!(!dist_out.join("LOCAL_DIAGNOSTIC_ONLY.txt").exists());

        // Explicit local-diagnostic packaging includes them plus a scope marker.
        let local_out = std::env::temp_dir().join(format!("rtp_out_localdiag_local_{}_{}", std::process::id(), n));
        let opts = BundlePackageOptions { include_local_diagnostic_native: true };
        package_distribution_bundle_with_options(&src_root, &local_out, None, &opts)
            .expect("local diagnostic packaging must succeed");
        assert!(local_out.join("runtimes/rgss/jni/arm64-v8a/libmkxp-z.so").is_file());
        assert!(local_out.join("runtimes/rgss/licenses/NOTICE.txt").is_file());
        let marker = fs::read_to_string(local_out.join("LOCAL_DIAGNOSTIC_ONLY.txt"))
            .expect("local diagnostic bundle must carry a scope marker");
        assert!(marker.contains("LOCAL_DIAGNOSTIC_ONLY"));
        assert!(marker.contains("SOURCE_UNVERIFIED"));
        assert!(marker.contains("NO_REMOTE_DISTRIBUTION"));
        // The marker is covered by the bundle's own checksums.
        let checksums = fs::read_to_string(local_out.join("checksums.txt")).unwrap();
        assert!(checksums.contains("LOCAL_DIAGNOSTIC_ONLY.txt"));
        assert!(checksums.contains("runtimes/rgss/jni/arm64-v8a/libmkxp-z.so"));
        assert!(verify_bundle_checksums(&local_out).is_ok());
    }

    #[test]
    fn test_verify_bundle_checksums_mutation_detection() {
        let n = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let src_root = create_mock_bundle_root(&format!("src_chk_{}", n));
        let out_root = std::env::temp_dir().join(format!("rtp_out_chk_{}_{}", std::process::id(), n));

        let bin_path = src_root.join("mock_bin");
        create_mock_elf_binary(&bin_path);

        let report = package_distribution_bundle(&src_root, &out_root, Some(&bin_path))
            .expect("Packaging must succeed");
        assert!(report.total_files >= 4);

        // Verification of intact bundle passes
        assert!(verify_bundle_checksums(&out_root).is_ok());

        // Mutate a file in out_root to simulate corruption/tampering
        let mutated_file = out_root.join("android/shell/build.gradle.kts");
        fs::write(&mutated_file, b"// corrupted content").unwrap();

        let res_mutated = verify_bundle_checksums(&out_root);
        assert!(res_mutated.is_err());
        let err = res_mutated.unwrap_err();
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_INTEGRITY_MISMATCH));
    }
}
