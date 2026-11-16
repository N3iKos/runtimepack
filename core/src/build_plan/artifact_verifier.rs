use crate::diagnostics::{DiagnosticCode, DiagnosticError};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Verification summary after executing all artifact gate stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactVerification {
    pub sha256: String,
    pub size_bytes: u64,
    pub zip_entries: usize,
    pub signature_verified: bool,
}

/// Structural and resource details from ZIP inspection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZipAuditDetails {
    pub total_entries: usize,
    pub total_uncompressed_bytes: u64,
}

/// Signature details extracted from apksigner output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureDetails {
    pub verified: bool,
    pub scheme_v2: bool,
    pub scheme_v3: bool,
    pub signer_certificate_sha256: Option<String>,
    pub raw_output: String,
}

/// Resource budgets to bound ZIP parsing and decompression against zip-bombs.
#[derive(Debug, Clone)]
pub struct ZipLimits {
    pub max_archive_bytes: u64,
    pub max_entries: usize,
    pub max_expanded_bytes: u64,
    pub timeout: Duration,
}

impl Default for ZipLimits {
    fn default() -> Self {
        Self {
            // 8 GiB maximum physical archive size before ZipArchive construction
            max_archive_bytes: 8 * 1024 * 1024 * 1024,
            // Accommodates up to 100,000 files inside production APKs
            max_entries: 100_000,
            // 8 GiB maximum aggregate uncompressed payload (supports large games up to 2 GB source)
            max_expanded_bytes: 8 * 1024 * 1024 * 1024,
            // 120 seconds verification timeout budget
            timeout: Duration::from_secs(120),
        }
    }
}

/// Trait for APK cryptographic signature verifier.
pub trait ApkVerifier: Send + Sync {
    fn verify_signature(&self, apk_path: &Path) -> Result<SignatureDetails, DiagnosticError>;
}

/// Trait for ZIP structural and streaming CRC validator.
pub trait ZipValidator: Send + Sync {
    fn validate_zip(
        &self,
        apk_path: &Path,
        limits: &ZipLimits,
    ) -> Result<ZipAuditDetails, DiagnosticError>;
}

/// Default production implementation of ZipValidator using zip 8.6.0.
pub struct DefaultZipValidator;

impl ZipValidator for DefaultZipValidator {
    fn validate_zip(
        &self,
        apk_path: &Path,
        limits: &ZipLimits,
    ) -> Result<ZipAuditDetails, DiagnosticError> {
        let meta = apk_path.symlink_metadata().map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("Failed to query APK metadata for ZIP validation: {}", e),
            )
        })?;

        // Guard against physical archive exceeding allocated archive size before central directory allocation
        if meta.len() > limits.max_archive_bytes {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Physical archive size ({} bytes) exceeds maximum allowable limit ({} bytes)",
                    meta.len(),
                    limits.max_archive_bytes
                ),
            ));
        }

        let file = File::open(apk_path).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("Failed to open APK file for ZIP validation: {}", e),
            )
        })?;

        let start_time = Instant::now();
        let reader = BufReader::new(file);

        let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("APK is not a structurally valid ZIP archive: {}", e),
            )
        })?;

        let entry_count = archive.len();
        if entry_count > limits.max_entries {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "APK entry count ({}) exceeds maximum limit ({})",
                    entry_count, limits.max_entries
                ),
            ));
        }

        let mut total_uncompressed: u64 = 0;
        let mut buffer = [0u8; 64 * 1024];

        for i in 0..entry_count {
            if start_time.elapsed() > limits.timeout {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!(
                        "ZIP validation exceeded timeout budget of {:?}",
                        limits.timeout
                    ),
                ));
            }

            let mut zip_file = archive.by_index(i).map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!("Failed to read ZIP entry at index {}: {}", i, e),
                )
            })?;

            if zip_file.encrypted() {
                return Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!(
                        "Encrypted ZIP entries are not supported in production APKs: {}",
                        zip_file.name()
                    ),
                ));
            }

            // Stream entry payload to EOF to trigger Crc32Reader verification
            loop {
                let bytes_read = zip_file.read(&mut buffer).map_err(|e| {
                    DiagnosticError::from_code(
                        DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                        format!(
                            "CRC or decompression failure on entry '{}': {}",
                            zip_file.name(),
                            e
                        ),
                    )
                })?;

                if bytes_read == 0 {
                    break;
                }

                total_uncompressed = total_uncompressed
                    .checked_add(bytes_read as u64)
                    .ok_or_else(|| {
                        DiagnosticError::from_code(
                            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                            "Aggregate uncompressed byte count overflowed u64",
                        )
                    })?;

                if total_uncompressed > limits.max_expanded_bytes {
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                        format!(
                            "APK uncompressed size exceeded maximum limit of {} bytes",
                            limits.max_expanded_bytes
                        ),
                    ));
                }
            }
        }

        Ok(ZipAuditDetails {
            total_entries: entry_count,
            total_uncompressed_bytes: total_uncompressed,
        })
    }
}

/// Production implementation of ApkVerifier invoking official Android SDK apksigner.
pub struct SystemApkVerifier {
    pub sdk_root: Option<PathBuf>,
    pub java_home: Option<PathBuf>,
    pub process_timeout: Duration,
    pub max_output_bytes: usize,
}

impl SystemApkVerifier {
    pub fn new() -> Self {
        let sdk_root = resolve_android_sdk_root();
        let java_home = resolve_java_home();
        Self {
            sdk_root,
            java_home,
            process_timeout: Duration::from_secs(120),
            max_output_bytes: 4 * 1024 * 1024, // 4 MiB output buffer limit
        }
    }

    pub fn with_roots(sdk_root: Option<PathBuf>, java_home: Option<PathBuf>) -> Self {
        Self {
            sdk_root,
            java_home,
            process_timeout: Duration::from_secs(120),
            max_output_bytes: 4 * 1024 * 1024,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.process_timeout = timeout;
        self
    }
}

impl Default for SystemApkVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl ApkVerifier for SystemApkVerifier {
    fn verify_signature(&self, apk_path: &Path) -> Result<SignatureDetails, DiagnosticError> {
        let sdk_root = self.sdk_root.as_ref().ok_or_else(|| {
            DiagnosticError::from_code(
                DiagnosticCode::SIG_TOOLCHAIN_MISSING,
                "Android SDK root not found; cannot verify APK signature via apksigner",
            )
        })?;

        // Locate official apksigner.jar or direct executable under build-tools
        let (java_bin, jar_path, direct_binary) =
            locate_apksigner_tool(sdk_root, self.java_home.as_deref())?;

        let mut cmd = if let Some(ref jar) = jar_path {
            let java_exe = java_bin.ok_or_else(|| {
                DiagnosticError::from_code(
                    DiagnosticCode::SIG_TOOLCHAIN_MISSING,
                    "Java runtime (java.exe / java) not found to execute apksigner.jar",
                )
            })?;
            let mut c = Command::new(java_exe);
            c.arg("-jar").arg(jar);
            c
        } else if let Some(ref bin) = direct_binary {
            Command::new(bin)
        } else {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::SIG_TOOLCHAIN_MISSING,
                "Neither apksigner.jar nor direct apksigner executable found in Android build-tools",
            ));
        };

        cmd.arg("verify")
            .arg("--verbose")
            .arg("--print-certs")
            .arg(apk_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::SIG_TOOLCHAIN_MISSING,
                format!("Failed to execute apksigner process: {}", e),
            )
        })?;

        let mut stdout_pipe = child.stdout.take();
        let mut stderr_pipe = child.stderr.take();
        let max_output = self.max_output_bytes;

        let (stdout_tx, stdout_rx) = mpsc::channel();
        let (stderr_tx, stderr_rx) = mpsc::channel();

        let stdout_handle = thread::spawn(move || {
            let mut out = Vec::new();
            if let Some(ref mut pipe) = stdout_pipe {
                let mut buf = [0u8; 4096];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    if out.len() + n <= max_output {
                        out.extend_from_slice(&buf[..n]);
                    }
                }
            }
            let _ = stdout_tx.send(out);
        });

        let stderr_handle = thread::spawn(move || {
            let mut err = Vec::new();
            if let Some(ref mut pipe) = stderr_pipe {
                let mut buf = [0u8; 4096];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    if err.len() + n <= max_output {
                        err.extend_from_slice(&buf[..n]);
                    }
                }
            }
            let _ = stderr_tx.send(err);
        });

        let start = Instant::now();
        let timeout = self.process_timeout;
        let mut exit_status = None;

        while start.elapsed() < timeout {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_status = Some(status);
                    break;
                }
                Ok(None) => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(e) => {
                    let _ = child.kill();
                    return Err(DiagnosticError::from_code(
                        DiagnosticCode::SIG_VERIFICATION_FAILED,
                        format!("Error polling apksigner child status: {}", e),
                    ));
                }
            }
        }

        if exit_status.is_none() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(DiagnosticError::from_code(
                DiagnosticCode::SIG_VERIFICATION_FAILED,
                format!("apksigner process timed out after {:?}", timeout),
            ));
        }

        let status = exit_status.unwrap();
        let _ = stdout_handle.join();
        let _ = stderr_handle.join();

        let stdout_bytes = stdout_rx.recv().unwrap_or_default();
        let stderr_bytes = stderr_rx.recv().unwrap_or_default();

        let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
        let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();
        let combined = format!("{}\n{}", stdout, stderr);

        if !status.success() {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::SIG_VERIFICATION_FAILED,
                format!(
                    "APK signature verification failed (exit code {:?}):\n{}",
                    status.code(),
                    combined.trim()
                ),
            ));
        }

        // Official verification criteria: stdout contains "Verifies"
        if !stdout.contains("Verifies") {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::SIG_VERIFICATION_FAILED,
                format!("apksigner did not confirm verification:\n{}", stdout.trim()),
            ));
        }

        let scheme_v2 = stdout.contains("Verified using v2 scheme (APK Signature Scheme v2): true");
        let scheme_v3 = stdout.contains("Verified using v3 scheme (APK Signature Scheme v3): true");
        let mut cert_sha256 = None;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("Signer #1 certificate SHA-256 digest:") {
                let digest = trimmed
                    .trim_start_matches("Signer #1 certificate SHA-256 digest:")
                    .trim();
                cert_sha256 = Some(digest.to_string());
                break;
            }
        }

        Ok(SignatureDetails {
            verified: true,
            scheme_v2,
            scheme_v3,
            signer_certificate_sha256: cert_sha256,
            raw_output: stdout,
        })
    }
}

/// Proves one packaged APK entry exists with exactly `expected_sha256`.
///
/// Shared by the artifact gate and the finalizer so a build can never publish an
/// APK whose packaged notice differs from the bytes its manifest and checksum
/// list describe.
pub fn verify_required_asset_in_apk(
    apk: &Path,
    entry: &str,
    expected_sha256: &str,
) -> Result<(), DiagnosticError> {
    let file = File::open(apk).map_err(|e| {
        DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!("Failed to open APK for asset verification: {}", e),
        )
    })?;
    let mut archive = zip::ZipArchive::new(BufReader::new(file)).map_err(|e| {
        DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!("APK is not a structurally valid ZIP archive: {}", e),
        )
    })?;
    let mut zip_file = archive.by_name(entry).map_err(|_| {
        DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!("Published APK is missing the required asset '{entry}'"),
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = zip_file.read(&mut buffer).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("Failed to read required asset '{entry}' from the APK: {e}"),
            )
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let actual = format!("{:x}", hasher.finalize());
    if actual != expected_sha256 {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
            format!(
                "Required asset '{entry}' in the published APK has hash {actual}, expected {expected_sha256}"
            ),
        ));
    }
    Ok(())
}

/// Core ArtifactVerifier coordinating the five validation stages.
pub struct ArtifactVerifier<'a> {
    verifier: &'a dyn ApkVerifier,
    zip_validator: &'a dyn ZipValidator,
    limits: ZipLimits,
    /// APK asset entries that must exist with an exact SHA-256 digest. Used to
    /// prove the published artifact really carries the notice the manifest and
    /// checksum list describe, which no other stage inspects.
    required_assets: Vec<(String, String)>,
}

impl<'a> ArtifactVerifier<'a> {
    pub fn new(verifier: &'a dyn ApkVerifier, zip_validator: &'a dyn ZipValidator) -> Self {
        Self::with_verifiers(verifier, zip_validator)
    }

    pub fn with_verifiers(
        verifier: &'a dyn ApkVerifier,
        zip_validator: &'a dyn ZipValidator,
    ) -> Self {
        Self {
            verifier,
            zip_validator,
            limits: ZipLimits::default(),
            required_assets: Vec::new(),
        }
    }

    /// Require the published APK to contain `entry` with exactly `sha256`.
    ///
    /// An experimental build claims, in its manifest and checksum list, that the
    /// APK ships the same notice bytes. Without this check no stage ever looks
    /// inside the archive, so a build could publish a notice it does not carry.
    pub fn with_required_asset(mut self, entry: &str, sha256: &str) -> Self {
        self.required_assets
            .push((entry.to_string(), sha256.to_string()));
        self
    }

    fn verify_required_assets(&self, apk: &Path) -> Result<(), DiagnosticError> {
        for (entry, expected) in &self.required_assets {
            verify_required_asset_in_apk(apk, entry, expected)?;
        }
        Ok(())
    }

    pub fn with_limits(mut self, limits: ZipLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Computes streaming SHA-256 hash using 64 KiB chunks without reading the whole file into memory.
    pub fn compute_streaming_sha256(path: &Path) -> Result<String, DiagnosticError> {
        let file = File::open(path).map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("Failed to open file for streaming SHA-256: {}", e),
            )
        })?;

        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let bytes_read = reader.read(&mut buffer).map_err(|e| {
                DiagnosticError::from_code(
                    DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                    format!("Failed reading streaming buffer: {}", e),
                )
            })?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        let result = hasher.finalize();
        Ok(format!("{:x}", result))
    }

    /// Executes the full five-stage verification gate on the source and destination APKs.
    pub fn run_full_gate(
        &self,
        source_apk: &Path,
        dest_apk: &Path,
    ) -> Result<ArtifactVerification, DiagnosticError> {
        // Stage 1: Non-empty regular file checks
        let src_meta = source_apk.symlink_metadata().map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("Source APK metadata error: {}", e),
            )
        })?;

        let dest_meta = dest_apk.symlink_metadata().map_err(|e| {
            DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!("Destination APK metadata error: {}", e),
            )
        })?;

        if !src_meta.file_type().is_file() || src_meta.len() == 0 {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Source APK is empty or not a regular file (size: {} bytes)",
                    src_meta.len()
                ),
            ));
        }

        if !dest_meta.file_type().is_file() || dest_meta.len() == 0 {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Destination APK is empty or not a regular file (size: {} bytes)",
                    dest_meta.len()
                ),
            ));
        }

        // Stage 2: Streaming SHA-256 and post-copy byte identity
        let src_hash = Self::compute_streaming_sha256(source_apk)?;
        let dest_hash = Self::compute_streaming_sha256(dest_apk)?;

        if src_hash != dest_hash {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_ARTIFACT_INTEGRITY_FAILED,
                format!(
                    "Byte identity verification failed between source ({}) and destination ({})",
                    src_hash, dest_hash
                ),
            ));
        }

        // Stage 3: Bounded ZIP structural and CRC-32 validation
        let zip_audit = self
            .zip_validator
            .validate_zip(dest_apk, &self.limits)?;

        // Stage 3b: Required packaged assets must match their recorded digests.
        self.verify_required_assets(dest_apk)?;

        // Stage 4: Cryptographic signature verification via apksigner
        let sig_details = self.verifier.verify_signature(dest_apk)?;
        if !sig_details.verified {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::SIG_VERIFICATION_FAILED,
                "APK signature could not be verified",
            ));
        }

        Ok(ArtifactVerification {
            sha256: dest_hash,
            size_bytes: dest_meta.len(),
            zip_entries: zip_audit.total_entries,
            signature_verified: sig_details.verified,
        })
    }
}

/// Owned container for ArtifactVerifier with dynamic toolchain resolution.
pub struct OwnedArtifactVerifier {
    pub verifier: SystemApkVerifier,
    pub zip_validator: DefaultZipValidator,
    pub limits: ZipLimits,
}

impl OwnedArtifactVerifier {
    /// Constructs an owned verifier dynamically resolving the SDK and Java environment per instance.
    pub fn from_env() -> Self {
        Self {
            verifier: SystemApkVerifier::new(),
            zip_validator: DefaultZipValidator,
            limits: ZipLimits::default(),
        }
    }

    pub fn with_limits(mut self, limits: ZipLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn run_full_gate(
        &self,
        source_apk: &Path,
        dest_apk: &Path,
    ) -> Result<ArtifactVerification, DiagnosticError> {
        let v = ArtifactVerifier::with_verifiers(&self.verifier, &self.zip_validator)
            .with_limits(self.limits.clone());
        v.run_full_gate(source_apk, dest_apk)
    }

    pub fn compute_streaming_sha256(path: &Path) -> Result<String, DiagnosticError> {
        ArtifactVerifier::compute_streaming_sha256(path)
    }
}

impl Default for OwnedArtifactVerifier {
    fn default() -> Self {
        Self::from_env()
    }
}

pub static DEFAULT_ZIP_VALIDATOR: DefaultZipValidator = DefaultZipValidator;

fn resolve_android_sdk_root() -> Option<PathBuf> {
    if let Ok(ah) = std::env::var("ANDROID_HOME") {
        let p = PathBuf::from(ah);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(asr) = std::env::var("ANDROID_SDK_ROOT") {
        let p = PathBuf::from(asr);
        if p.exists() {
            return Some(p);
        }
    }
    if cfg!(windows) {
        if let Ok(la) = std::env::var("LOCALAPPDATA") {
            let p = PathBuf::from(&la).join("Android").join("Sdk");
            if p.exists() {
                return Some(p);
            }
        }
    } else if cfg!(unix) {
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(&home).join("Android").join("Sdk");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

fn resolve_java_home() -> Option<PathBuf> {
    if let Ok(jh) = std::env::var("JAVA_HOME") {
        let p = PathBuf::from(jh);
        if p.exists() {
            return Some(p);
        }
    }
    if cfg!(windows) {
        if let Ok(profile) = std::env::var("USERPROFILE") {
            let p = PathBuf::from(&profile)
                .join(".runtimepack")
                .join("toolchains")
                .join("jdk-17");
            if p.exists() {
                return Some(p);
            }
        }
    } else if cfg!(unix) {
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(&home)
                .join(".runtimepack")
                .join("toolchains")
                .join("jdk-17");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

fn locate_apksigner_tool(
    sdk_root: &Path,
    java_home: Option<&Path>,
) -> Result<(Option<PathBuf>, Option<PathBuf>, Option<PathBuf>), DiagnosticError> {
    let build_tools_dir = sdk_root.join("build-tools");
    if !build_tools_dir.is_dir() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::SIG_TOOLCHAIN_MISSING,
            format!(
                "Android SDK build-tools directory not found at {}",
                build_tools_dir.display()
            ),
        ));
    }

    // Read entries in build-tools and pick pinned 34.0.0 if present, or latest available version
    let mut versions: Vec<PathBuf> = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(&build_tools_dir) {
        for entry in read_dir.flatten() {
            if entry.path().is_dir() {
                versions.push(entry.path());
            }
        }
    }

    if versions.is_empty() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::SIG_TOOLCHAIN_MISSING,
            "No build-tools versions found in Android SDK",
        ));
    }

    // Prefer exact version 34.0.0
    versions.sort();
    let selected_dir = versions
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("34.0.0"))
        .unwrap_or_else(|| versions.last().unwrap());

    let apksigner_jar = selected_dir.join("lib").join("apksigner.jar");
    if apksigner_jar.is_file() {
        // Resolve java executable
        let java_exe_name = if cfg!(windows) { "java.exe" } else { "java" };
        let java_bin = if let Some(jh) = java_home {
            let bin = jh.join("bin").join(java_exe_name);
            if bin.is_file() {
                Some(bin)
            } else {
                None
            }
        } else {
            None
        }
        .or_else(|| {
            // Check PATH entries
            if let Ok(path_var) = std::env::var("PATH") {
                for dir in std::env::split_paths(&path_var) {
                    let candidate = dir.join(java_exe_name);
                    if candidate.is_file() {
                        return Some(candidate);
                    }
                }
            }
            None
        });

        if let Some(jb) = java_bin {
            return Ok((Some(jb), Some(apksigner_jar), None));
        }
    }

    // Direct binary fallback: on Unix only (trusted official POSIX shell script).
    // On Windows, .bat scripts are rejected to prevent cmd.exe command injection and quoting defects.
    if cfg!(unix) {
        let direct_bin = selected_dir.join("apksigner");
        if direct_bin.is_file() {
            return Ok((None, None, Some(direct_bin)));
        }
    } else if cfg!(windows) {
        let direct_exe = selected_dir.join("apksigner.exe");
        if direct_exe.is_file() {
            return Ok((None, None, Some(direct_exe)));
        }
    }

    Err(DiagnosticError::from_code(
        DiagnosticCode::SIG_TOOLCHAIN_MISSING,
        format!(
            "Official apksigner.jar (or native executable) not found or cannot execute in {}",
            selected_dir.display()
        ),
    ))
}
