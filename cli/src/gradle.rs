use runtimepack_core::diagnostics::{DiagnosticCode, DiagnosticError};
use runtimepack_core::provenance::ToolchainLock;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Where the pinned toolchain requirements came from.
///
/// Diagnostics must report this honestly: claiming `toolchain.lock` when the
/// built-in defaults were used is a truthfulness defect, and silently using
/// built-in values while the shipped lock says otherwise is drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinSource {
    /// Values parsed from the `toolchain.lock` in the resolved resource root.
    LockFile,
    /// Built-in defaults, because no readable `toolchain.lock` was available.
    BuiltInDefault,
}

impl PinSource {
    pub fn label(&self) -> &'static str {
        match self {
            PinSource::LockFile => "toolchain.lock",
            PinSource::BuiltInDefault => "built-in defaults (pin file not readable)",
        }
    }
}

/// The pinned requirements the preflight enforces.
#[derive(Debug, Clone)]
pub struct ToolchainPins {
    pub lock: ToolchainLock,
    pub source: PinSource,
}

impl ToolchainPins {
    pub fn jdk_min_major(&self) -> u32 {
        self.lock
            .jdk_min_version
            .split('.')
            .next()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(17)
    }

    pub fn platform_rel(&self) -> String {
        format!("platforms/android-{}", self.lock.android_compile_sdk)
    }

    pub fn build_tools_rel(&self) -> String {
        format!("build-tools/{}", self.lock.build_tools_version)
    }
}

/// Reads `toolchain.lock` from `root`, falling back to built-in defaults.
///
/// `root == None` always yields the built-in defaults (used by library-style
/// callers and tests). A missing or malformed file is not fatal: the caller
/// still gets usable pins, but `source` records that the lock file was not read
/// so no diagnostic can claim otherwise.
pub fn resolve_toolchain_pins(root: Option<&Path>) -> ToolchainPins {
    let lock_path = root.map(|r| r.join("toolchain.lock"));
    if let Some(path) = lock_path {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(lock) = ToolchainLock::from_json_str(&text) {
                return ToolchainPins {
                    lock,
                    source: PinSource::LockFile,
                };
            }
        }
    }
    ToolchainPins {
        lock: ToolchainLock::default(),
        source: PinSource::BuiltInDefault,
    }
}

/// Resolves pins from the active RuntimePack resource root, if one is resolvable.
pub fn resolve_toolchain_pins_from_resource_root() -> ToolchainPins {
    let root = runtimepack_core::resource_root::resolve_resource_root().ok();
    resolve_toolchain_pins(root.as_deref())
}

fn java_binary_name() -> &'static str {
    if cfg!(windows) {
        "bin/java.exe"
    } else {
        "bin/java"
    }
}

/// Where the Android SDK root came from. Reported in diagnostics so an operator
/// can tell which of several candidate sources actually won.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkSource {
    AndroidHome,
    AndroidSdkRoot,
    LocalProperties,
    Conventional,
    Missing,
}

impl SdkSource {
    pub fn label(&self) -> &'static str {
        match self {
            SdkSource::AndroidHome => "ANDROID_HOME",
            SdkSource::AndroidSdkRoot => "ANDROID_SDK_ROOT",
            SdkSource::LocalProperties => "local.properties sdk.dir",
            SdkSource::Conventional => "conventional install path",
            SdkSource::Missing => "not found",
        }
    }
}

/// Where `JAVA_HOME` came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaSource {
    JavaHome,
    BundledToolchain,
    Path,
    Missing,
}

impl JavaSource {
    pub fn label(&self) -> &'static str {
        match self {
            JavaSource::JavaHome => "JAVA_HOME",
            JavaSource::BundledToolchain => "bundled toolchain",
            JavaSource::Path => "java on PATH",
            JavaSource::Missing => "not found",
        }
    }
}

/// Single resolved view of the Android build toolchain.
///
/// This is the one contract shared by the preflight check, the Gradle command
/// environment and the artifact verifier, so a host that passes preflight is
/// the same host Gradle is invoked on.
#[derive(Debug, Clone)]
pub struct AndroidToolchain {
    pub sdk_root: Option<PathBuf>,
    pub sdk_source: SdkSource,
    pub java_home: Option<PathBuf>,
    pub java_source: JavaSource,
    /// Detected JDK major version. `None` means it could not be determined,
    /// which fails closed rather than passing silently.
    pub java_major: Option<u32>,
}

fn non_empty_trimmed(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Parses `sdk.dir` out of a `local.properties` file body.
///
/// Java properties escaping is honoured: `\:`, `\=`, `\#`, `\!`, `\ `, `\\`
/// and `\t`/`\n`/`\r`/`\uXXXX`. Windows paths written as
/// `sdk.dir=C\:\\Users\\me\\Android\\Sdk` therefore resolve correctly.
pub fn parse_local_properties_sdk_dir(content: &str) -> Option<PathBuf> {
    for raw_line in content.lines() {
        let line = raw_line.trim_start();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        // Split on the first unescaped '=' or ':'.
        let mut key = String::new();
        let mut rest: Option<&str> = None;
        let mut chars = line.char_indices().peekable();
        while let Some((idx, ch)) = chars.next() {
            match ch {
                '\\' => {
                    key.push(ch);
                    if let Some((_, next)) = chars.next() {
                        key.push(next);
                    }
                }
                '=' | ':' => {
                    rest = Some(&line[idx + ch.len_utf8()..]);
                    break;
                }
                _ => key.push(ch),
            }
        }
        if unescape_properties(key.trim()).trim() != "sdk.dir" {
            continue;
        }
        let value = unescape_properties(rest.unwrap_or("").trim());
        let value = value.trim().to_string();
        if value.is_empty() {
            return None;
        }
        return Some(PathBuf::from(value));
    }
    None
}

fn unescape_properties(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('t') => out.push('\t'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(c) => out.push(c),
                    None => {
                        out.push('\\');
                        out.push('u');
                        out.push_str(&hex);
                    }
                }
            }
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Pure SDK-root precedence: `ANDROID_HOME` -> `ANDROID_SDK_ROOT` ->
/// `local.properties` -> conventional per-OS path. Blank values are ignored.
pub fn resolve_sdk_root_from_values(
    android_home: Option<&str>,
    android_sdk_root: Option<&str>,
    local_properties_sdk: Option<&Path>,
    conventional: Option<&Path>,
) -> (Option<PathBuf>, SdkSource) {
    if let Some(v) = non_empty_trimmed(android_home.map(str::to_string)) {
        return (Some(PathBuf::from(v)), SdkSource::AndroidHome);
    }
    if let Some(v) = non_empty_trimmed(android_sdk_root.map(str::to_string)) {
        return (Some(PathBuf::from(v)), SdkSource::AndroidSdkRoot);
    }
    if let Some(p) = local_properties_sdk {
        return (Some(p.to_path_buf()), SdkSource::LocalProperties);
    }
    if let Some(p) = conventional {
        return (Some(p.to_path_buf()), SdkSource::Conventional);
    }
    (None, SdkSource::Missing)
}

/// Pure JDK precedence: `JAVA_HOME` -> bundled toolchain dir.
///
/// PATH-only Java is handled by the live probe, which cannot be expressed as a
/// pure value.
pub fn resolve_java_home_from_values(
    java_home: Option<&str>,
    bundled: Option<&Path>,
) -> (Option<PathBuf>, JavaSource) {
    if let Some(v) = non_empty_trimmed(java_home.map(str::to_string)) {
        return (Some(PathBuf::from(v)), JavaSource::JavaHome);
    }
    if let Some(p) = bundled {
        if p.join(java_binary_name()).is_file() {
            return (Some(p.to_path_buf()), JavaSource::BundledToolchain);
        }
    }
    (None, JavaSource::Missing)
}

/// Conventional per-OS SDK install path, if it exists.
pub fn conventional_sdk_dir(base: Option<&Path>) -> Option<PathBuf> {
    let candidate = base?.join("Android").join("Sdk");
    candidate.is_dir().then_some(candidate)
}

/// Documented legacy bundled JDK location (`<home>/.runtimepack/toolchains/jdk-17`).
pub fn bundled_jdk_dir(base: Option<&Path>) -> Option<PathBuf> {
    let dir = base?.join(".runtimepack").join("toolchains").join("jdk-17");
    dir.is_dir().then_some(dir)
}

/// Extracts the JDK major version from a `release` file or `java -version` text.
pub fn parse_java_major_from_version_text(text: &str) -> Option<u32> {
    let bytes: Vec<char> = text.chars().collect();
    // Find the first quoted version token, e.g. JAVA_VERSION="17.0.1" or
    // openjdk version "17.0.1".
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == '"' {
            let start = i + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end] != '"' {
                end += 1;
            }
            if end > start {
                let token: String = bytes[start..end].iter().collect();
                if let Some(major) = parse_major_token(&token) {
                    return Some(major);
                }
            }
            i = end + 1;
            continue;
        }
        i += 1;
    }
    None
}

fn parse_major_token(token: &str) -> Option<u32> {
    let token = token.trim();
    if !token.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        return None;
    }
    let parts: Vec<&str> = token.split(['.', '_', '-', '+']).collect();
    let first: u32 = parts.first()?.parse().ok()?;
    if first == 1 {
        // Legacy 1.x scheme: 1.8.0_402 -> 8
        return parts.get(1).and_then(|s| s.parse::<u32>().ok());
    }
    Some(first)
}

/// Reads the JDK major version from `<java_home>/release`.
pub fn read_java_major_from_release(java_home: &Path) -> Option<u32> {
    let content = std::fs::read_to_string(java_home.join("release")).ok()?;
    parse_java_major_from_version_text(&content)
}

fn home_base() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var("USERPROFILE")
            .ok()
            .map(PathBuf::from)
            .or_else(|| std::env::var("HOME").ok().map(PathBuf::from))
    } else {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

fn local_app_data() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var("LOCALAPPDATA").ok().map(PathBuf::from)
    } else {
        std::env::var("HOME").ok().map(PathBuf::from)
    }
}

/// Probes `java` on PATH for its major version.
fn probe_path_java_major() -> Option<u32> {
    let out = Command::new("java").arg("-version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&out.stdout).into_owned();
    }
    parse_java_major_from_version_text(&text)
}

/// Resolves the toolchain from the live host environment.
///
/// `project_dir` is the actual generated Android project directory. Its
/// `local.properties` is consulted only when no SDK environment variable is
/// set, which is the only case where that file can add information.
pub fn resolve_android_toolchain(project_dir: Option<&Path>) -> AndroidToolchain {
    let local_props_sdk = project_dir
        .and_then(|dir| std::fs::read_to_string(dir.join("local.properties")).ok())
        .and_then(|content| parse_local_properties_sdk_dir(&content));
    let conventional = conventional_sdk_dir(local_app_data().as_deref());

    let (sdk_root, sdk_source) = resolve_sdk_root_from_values(
        std::env::var("ANDROID_HOME").ok().as_deref(),
        std::env::var("ANDROID_SDK_ROOT").ok().as_deref(),
        local_props_sdk.as_deref(),
        conventional.as_deref(),
    );

    let bundled = bundled_jdk_dir(home_base().as_deref());
    let (java_home, mut java_source) = resolve_java_home_from_values(
        std::env::var("JAVA_HOME").ok().as_deref(),
        bundled.as_deref(),
    );

    let mut java_major = java_home.as_deref().and_then(read_java_major_from_release);

    if java_home.is_none() {
        if let Some(major) = probe_path_java_major() {
            java_source = JavaSource::Path;
            java_major = Some(major);
        }
    } else if java_major.is_none() {
        // JAVA_HOME set but no readable `release`: ask that JDK directly.
        if let Some(home) = java_home.as_deref() {
            let java_bin = home.join(java_binary_name());
            if let Ok(out) = Command::new(java_bin).arg("-version").output() {
                if out.status.success() {
                    let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
                    if text.trim().is_empty() {
                        text = String::from_utf8_lossy(&out.stdout).into_owned();
                    }
                    java_major = parse_java_major_from_version_text(&text);
                }
            }
        }
    }

    if java_home.is_none() && java_source == JavaSource::Missing {
        // keep Missing
    } else if java_source == JavaSource::Missing {
        java_source = JavaSource::Path;
    }

    AndroidToolchain {
        sdk_root,
        sdk_source,
        java_home,
        java_source,
        java_major,
    }
}

/// Combines a freshly resolved toolchain with an earlier resolution.
///
/// A build resolves the toolchain twice: once before any output exists, and once
/// after the Android project is generated. The second pass sees the generated
/// project, but a retained project's `local.properties` was only visible to the
/// first. Missing SDK/JDK values are therefore carried over from the earlier
/// resolution instead of being silently lost.
pub fn merge_toolchain(
    fresh: AndroidToolchain,
    earlier: Option<&AndroidToolchain>,
) -> AndroidToolchain {
    let mut merged = fresh;
    if let Some(prev) = earlier {
        if merged.sdk_root.is_none() && prev.sdk_root.is_some() {
            merged.sdk_root = prev.sdk_root.clone();
            merged.sdk_source = prev.sdk_source;
        }
        if merged.java_home.is_none() && prev.java_home.is_some() {
            merged.java_home = prev.java_home.clone();
            merged.java_source = prev.java_source;
            merged.java_major = prev.java_major;
        }
        if merged.java_major.is_none() {
            merged.java_major = prev.java_major;
        }
    }
    merged
}

/// Validates a resolved toolchain against the pinned contract in `toolchain.lock`.
///
/// Checks, in order: SDK present, pinned platform present, pinned build-tools
/// present, JDK present, JDK major detectable and >= pinned minimum.
///
/// `project_dir` is the generated Android project, when one exists. It only
/// affects the remediation text: `sdk.dir` is mentioned as an option only when a
/// project file that Gradle would actually read exists, so the diagnostic never
/// suggests an edit that the next invocation would discard.
pub fn validate_android_toolchain(
    tc: &AndroidToolchain,
    project_dir: Option<&Path>,
) -> Result<(), DiagnosticError> {
    let pins = resolve_toolchain_pins_from_resource_root();
    validate_android_toolchain_with_pins(tc, project_dir, &pins)
}

/// Validates against explicit pins. This is the form the build pipeline uses so
/// the checked components and the reported provenance come from the same read.
pub fn validate_android_toolchain_with_pins(
    tc: &AndroidToolchain,
    project_dir: Option<&Path>,
    pins: &ToolchainPins,
) -> Result<(), DiagnosticError> {
    let platform_rel = pins.platform_rel();
    let build_tools_rel = pins.build_tools_rel();
    let jdk_min = pins.jdk_min_major();
    let pin_note = match pins.source {
        PinSource::LockFile => format!(" (from {})", pins.source.label()),
        PinSource::BuiltInDefault => format!(" ({})", pins.source.label()),
    };
    let project_hint = match project_dir {
        Some(dir) => format!(
            " Or set 'sdk.dir' in '{}' (the generated project Gradle reads).",
            dir.join("local.properties").display()
        ),
        None => String::new(),
    };

    let sdk_dir = match tc.sdk_root.as_deref() {
        Some(p) if p.is_dir() => p,
        other => {
            return Err(DiagnosticError::from_code(
                DiagnosticCode::BLD_TOOLCHAIN_MISSING,
                format!(
                    "Android SDK not found. Set ANDROID_HOME or ANDROID_SDK_ROOT to a valid SDK directory (compileSdk {}, build-tools {}{}). Checked: {} (source: {}).{}",
                    pins.lock.android_compile_sdk,
                    pins.lock.build_tools_version,
                    pin_note,
                    other
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "<unset>".to_string()),
                    tc.sdk_source.label(),
                    project_hint
                ),
            ));
        }
    };

    if !sdk_dir.join(&platform_rel).is_dir() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "Android SDK at '{}' is missing the required platform component '{}'{} (offline-first: RuntimePack does not auto-download SDK components). Install it with: sdkmanager \"platforms;android-{}\"",
                sdk_dir.display(),
                platform_rel,
                pin_note,
                pins.lock.android_compile_sdk
            ),
        ));
    }

    if !sdk_dir.join(&build_tools_rel).is_dir() {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "Android SDK at '{}' is missing the required build-tools component '{}'{} (offline-first: RuntimePack does not auto-download SDK components). Install it with: sdkmanager \"build-tools;{}\"",
                sdk_dir.display(),
                build_tools_rel,
                pin_note,
                pins.lock.build_tools_version
            ),
        ));
    }

    let java_present = match tc.java_home.as_deref() {
        Some(home) => home.join(java_binary_name()).is_file(),
        None => false,
    } || tc.java_source == JavaSource::Path;

    if !java_present {
        return Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "JDK {} not found. Set JAVA_HOME to a JDK {} installation, or put 'java' on PATH. Checked: {} (source: {}).",
                jdk_min,
                jdk_min,
                tc.java_home
                    .as_deref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<unset>".to_string()),
                tc.java_source.label()
            ),
        ));
    }

    match tc.java_major {
        Some(major) if major >= jdk_min => Ok(()),
        Some(major) => Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "JDK {} or newer required by AGP/Gradle{}, but the resolved JDK reports major version {}. Set JAVA_HOME to a JDK {} installation.",
                jdk_min, pin_note, major, jdk_min
            ),
        )),
        None => Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "Could not determine the JDK major version, so the JDK {} requirement cannot be verified. Set JAVA_HOME to a JDK {} installation that contains a readable 'release' file.",
                jdk_min, jdk_min
            ),
        )),
    }
}

/// Live preflight used before Gradle starts.
///
/// `project_dir` is the generated Android project, so a retained project's
/// `local.properties` is honoured; a freshly generated project derives it from
/// the environment, so the two agree.
pub fn preflight_android_toolchain(project_dir: &Path) -> Result<(), DiagnosticError> {
    let tc = resolve_android_toolchain(Some(project_dir));
    validate_android_toolchain(&tc, Some(project_dir))
}

/// Early preflight, run before the output directory or project is created.
///
/// `retained_project_dir` is an existing project directory if one is present
/// (for example from `--keep-build-files`), so a `local.properties` that Gradle
/// would actually read still counts. When it is absent, the diagnostic names only
/// the environment variables.
pub fn preflight_android_toolchain_early(
    retained_project_dir: Option<&Path>,
) -> Result<(), DiagnosticError> {
    let tc = resolve_android_toolchain(retained_project_dir);
    validate_android_toolchain(&tc, retained_project_dir)
}

/// Applies the resolved toolchain to a Gradle command.
pub fn apply_gradle_env_with(cmd: &mut Command, tc: &AndroidToolchain) {
    if let Some(home) = tc.java_home.as_deref() {
        cmd.env("JAVA_HOME", home);
    }
    if let Some(sdk) = tc.sdk_root.as_deref() {
        cmd.env("ANDROID_HOME", sdk);
        cmd.env("ANDROID_SDK_ROOT", sdk);
    }

    if let Ok(guh) = std::env::var("GRADLE_USER_HOME") {
        if !guh.trim().is_empty() {
            cmd.env("GRADLE_USER_HOME", guh);
        }
    } else if let Some(base) = home_base() {
        let p = base.join(".gradle");
        if p.exists() {
            cmd.env("GRADLE_USER_HOME", p);
        }
    }
}

/// Applies standard Android/Gradle environment variables to the command.
/// Dynamically resolves variables from the host environment without hardcoded machine paths
/// and strictly partitions OS-specific fallback lookups.
pub fn apply_gradle_env(cmd: &mut Command) {
    let tc = resolve_android_toolchain(None);
    apply_gradle_env_with(cmd, &tc);
}

/// Resolves the optimal Gradle invocation command for the target Android project directory.
///
/// Priority:
/// 1. Pinned Project Wrapper (`gradlew` on Unix via POSIX `sh ./gradlew`, `gradlew.bat` on Windows).
/// 2. Legacy Toolchain Fallback (`USERPROFILE/.runtimepack/toolchains/gradle-8.7/bin/gradle.bat` on Windows).
/// 3. System PATH Fallback (`gradle`) verified against pinned Gradle 8.7.
///
/// Fails closed with `BLD-101` if wrapper is missing and no verified Gradle 8.7 toolchain is present.
pub fn resolve_gradle_command(
    android_project_dir: &Path,
) -> Result<(Command, String), DiagnosticError> {
    let tc = resolve_android_toolchain(Some(android_project_dir));
    resolve_gradle_command_with(android_project_dir, &tc)
}

/// Resolves the Gradle invocation using an already-validated toolchain.
///
/// The build pipeline must use this form: re-resolving here would spawn Gradle
/// with a different SDK/JDK than the one the preflight approved (for example
/// when the SDK is known only through `local.properties`).
pub fn resolve_gradle_command_with(
    android_project_dir: &Path,
    tc: &AndroidToolchain,
) -> Result<(Command, String), DiagnosticError> {
    if cfg!(unix) {
        let gradlew_path = android_project_dir.join("gradlew");
        if gradlew_path.is_file() {
            let mut cmd = Command::new("sh");
            cmd.arg("./gradlew");
            cmd.current_dir(android_project_dir);
            apply_gradle_env_with(&mut cmd, tc);
            return Ok((cmd, "project-wrapper-unix".to_string()));
        }
    } else if cfg!(windows) {
        let gradlew_bat = android_project_dir.join("gradlew.bat");
        if gradlew_bat.is_file() {
            let mut cmd = Command::new(gradlew_bat);
            cmd.current_dir(android_project_dir);
            apply_gradle_env_with(&mut cmd, tc);
            return Ok((cmd, "project-wrapper-windows".to_string()));
        }

        if let Ok(profile) = std::env::var("USERPROFILE") {
            let gradle_bin =
                PathBuf::from(&profile).join(".runtimepack/toolchains/gradle-8.7/bin/gradle.bat");
            if gradle_bin.is_file() {
                let mut cmd = Command::new(gradle_bin);
                cmd.current_dir(android_project_dir);
                apply_gradle_env_with(&mut cmd, tc);
                return Ok((cmd, "legacy-toolchain-windows".to_string()));
            }
        }
    }

    // Priority 3: Fall back to system PATH `gradle`, but enforce exact version pin (8.7)
    let version_check = Command::new("gradle").arg("--version").output();
    match version_check {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("Gradle 8.7") {
                let mut cmd = Command::new("gradle");
                cmd.current_dir(android_project_dir);
                apply_gradle_env_with(&mut cmd, tc);
                Ok((cmd, "system-path".to_string()))
            } else {
                let detected = stdout
                    .lines()
                    .find(|l| l.contains("Gradle"))
                    .unwrap_or("unknown");
                Err(DiagnosticError::from_code(
                    DiagnosticCode::BLD_TOOLCHAIN_MISSING,
                    format!(
                        "No project Gradle wrapper found and system 'gradle' is unpinned or incompatible: expected Gradle 8.7, got {}",
                        detected.trim()
                    ),
                ))
            }
        }
        Ok(out) => Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "No project Gradle wrapper found and system 'gradle --version' exited with status {:?}: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        )),
        Err(e) => Err(DiagnosticError::from_code(
            DiagnosticCode::BLD_TOOLCHAIN_MISSING,
            format!(
                "No project Gradle wrapper found and system 'gradle' is missing or not executable: {}",
                e
            ),
        )),
    }
}

#[cfg(test)]
mod validated_toolchain_flow_tests {
    use super::*;
    use std::fs;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rtp_flow_{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_sdk(platform: &str, build_tools: &str) -> PathBuf {
        let sdk = temp_dir("sdk");
        fs::create_dir_all(sdk.join("platforms").join(platform)).unwrap();
        fs::create_dir_all(sdk.join("build-tools").join(build_tools)).unwrap();
        sdk
    }

    fn env_of(cmd: &Command, key: &str) -> Option<String> {
        cmd.get_envs()
            .find(|(k, _)| k.to_string_lossy() == key)
            .and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
    }

    /// I-1: the *validated* toolchain must be what the Gradle child process gets,
    /// not a fresh re-resolution from the ambient environment.
    #[test]
    fn gradle_command_uses_validated_sdk_when_env_is_unset() {
        let sdk = make_sdk("android-34", "34.0.0");
        let project = temp_dir("project_with_local_props");
        fs::create_dir_all(project.join("app")).unwrap();
        // The project carries the only SDK reference.
        fs::write(
            project.join("local.properties"),
            format!("sdk.dir={}\n", sdk.display()),
        )
        .unwrap();
        // A wrapper so resolution succeeds without needing a real Gradle install.
        fs::write(project.join("gradlew"), b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(project.join("gradlew")).unwrap().permissions();
            p.set_mode(0o755);
            let _ = fs::set_permissions(project.join("gradlew"), p);
        }

        let tc = AndroidToolchain {
            sdk_root: Some(sdk.clone()),
            sdk_source: SdkSource::LocalProperties,
            java_home: None,
            java_source: JavaSource::Path,
            java_major: Some(17),
        };

        let (cmd, _strategy) = resolve_gradle_command_with(&project, &tc)
            .expect("resolution must succeed with a wrapper present");

        assert_eq!(
            env_of(&cmd, "ANDROID_HOME").as_deref(),
            Some(sdk.to_string_lossy().as_ref()),
            "ANDROID_HOME must come from the validated toolchain"
        );
        assert_eq!(
            env_of(&cmd, "ANDROID_SDK_ROOT").as_deref(),
            Some(sdk.to_string_lossy().as_ref()),
            "ANDROID_SDK_ROOT must come from the validated toolchain"
        );
    }

    /// I-1: the same holds for the JDK, including the bundled-toolchain route.
    #[test]
    fn gradle_command_uses_validated_java_home() {
        let sdk = make_sdk("android-34", "34.0.0");
        let jdk = temp_dir("jdk");
        fs::create_dir_all(jdk.join("bin")).unwrap();
        let project = temp_dir("project_java");
        fs::write(project.join("gradlew"), b"#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut p = fs::metadata(project.join("gradlew")).unwrap().permissions();
            p.set_mode(0o755);
            let _ = fs::set_permissions(project.join("gradlew"), p);
        }

        let tc = AndroidToolchain {
            sdk_root: Some(sdk),
            sdk_source: SdkSource::AndroidHome,
            java_home: Some(jdk.clone()),
            java_source: JavaSource::BundledToolchain,
            java_major: Some(17),
        };
        let (cmd, _) = resolve_gradle_command_with(&project, &tc).unwrap();
        assert_eq!(
            env_of(&cmd, "JAVA_HOME").as_deref(),
            Some(jdk.to_string_lossy().as_ref()),
            "JAVA_HOME must come from the validated toolchain"
        );
    }

    /// I-2: pins come from the shipped `toolchain.lock` when it is readable.
    #[test]
    fn pins_are_read_from_toolchain_lock_file() {
        let root = temp_dir("root_with_lock");
        fs::write(
            root.join("toolchain.lock"),
            r#"{
  "lock_version": 1,
  "gradle_version": "8.7",
  "agp_version": "8.3.0",
  "jdk_min_version": "21",
  "android_compile_sdk": 35,
  "android_min_sdk": 26,
  "android_target_sdk": 35,
  "build_tools_version": "35.0.0"
}"#,
        )
        .unwrap();

        let pins = resolve_toolchain_pins(Some(&root));
        assert_eq!(pins.source, PinSource::LockFile);
        assert_eq!(pins.lock.android_compile_sdk, 35);
        assert_eq!(pins.lock.build_tools_version, "35.0.0");
        assert_eq!(pins.jdk_min_major(), 21);
        assert_eq!(pins.platform_rel(), "platforms/android-35");
        assert_eq!(pins.build_tools_rel(), "build-tools/35.0.0");
    }

    /// I-2: a missing/unreadable lock falls back to built-in defaults *and says so*.
    #[test]
    fn pins_fall_back_to_defaults_and_report_the_source() {
        let empty_root = temp_dir("root_without_lock");
        let pins = resolve_toolchain_pins(Some(&empty_root));
        assert_eq!(pins.source, PinSource::BuiltInDefault);
        assert_eq!(pins.lock.android_compile_sdk, 34);
        assert_eq!(pins.source.label(), "built-in defaults (pin file not readable)");
    }

    /// I-2: the validation uses the resolved pins, and the message names the
    /// component that the *lock file* asked for.
    #[test]
    fn validation_uses_lock_file_pins_not_hardcoded_values() {
        let root = temp_dir("root_lock_35");
        fs::write(
            root.join("toolchain.lock"),
            r#"{
  "lock_version": 1,
  "gradle_version": "8.7",
  "agp_version": "8.3.0",
  "jdk_min_version": "17",
  "android_compile_sdk": 35,
  "android_min_sdk": 26,
  "android_target_sdk": 35,
  "build_tools_version": "35.0.0"
}"#,
        )
        .unwrap();
        let pins = resolve_toolchain_pins(Some(&root));
        let sdk = make_sdk("android-34", "34.0.0");
        let tc = AndroidToolchain {
            sdk_root: Some(sdk),
            sdk_source: SdkSource::AndroidHome,
            java_home: None,
            java_source: JavaSource::Path,
            java_major: Some(17),
        };

        let err = validate_android_toolchain_with_pins(&tc, None, &pins)
            .expect_err("SDK lacking the lock-file platform must fail");
        let text = err.to_string();
        assert!(
            text.contains("platforms/android-35"),
            "diagnostic must name the lock-file platform, got: {text}"
        );
        assert!(
            text.contains("toolchain.lock"),
            "diagnostic may reference toolchain.lock because it was actually read, got: {text}"
        );
    }

    /// I-2: when defaults were used, the message must NOT claim toolchain.lock.
    #[test]
    fn validation_does_not_claim_toolchain_lock_when_defaults_were_used() {
        let pins = resolve_toolchain_pins(None);
        let sdk = make_sdk("android-34", "33.0.1");
        let tc = AndroidToolchain {
            sdk_root: Some(sdk),
            sdk_source: SdkSource::AndroidHome,
            java_home: None,
            java_source: JavaSource::Path,
            java_major: Some(17),
        };
        let err = validate_android_toolchain_with_pins(&tc, None, &pins)
            .expect_err("missing build-tools must fail");
        let text = err.to_string();
        assert!(
            !text.contains("toolchain.lock"),
            "must not claim a lock file it did not read, got: {text}"
        );
        assert!(
            text.contains("built-in defaults"),
            "must name the actual pin source, got: {text}"
        );
    }

    /// I-5: the diagnostic states the offline-first contract (no auto-download).
    #[test]
    fn component_diagnostic_states_offline_first_policy() {
        let pins = resolve_toolchain_pins(None);
        let sdk = make_sdk("android-34", "34.0.0");
        fs::remove_dir_all(sdk.join("platforms/android-34")).unwrap();
        let tc = AndroidToolchain {
            sdk_root: Some(sdk),
            sdk_source: SdkSource::AndroidHome,
            java_home: None,
            java_source: JavaSource::Path,
            java_major: Some(17),
        };
        let err = validate_android_toolchain_with_pins(&tc, None, &pins)
            .expect_err("missing platform must fail");
        let text = err.to_string();
        assert!(
            text.contains("does not auto-download"),
            "diagnostic must state that no component is auto-downloaded, got: {text}"
        );
    }
}

#[cfg(test)]
mod toolchain_preflight_tests {
    use super::*;
    use std::fs;

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rtp_preflight_{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_fake_sdk() -> PathBuf {
        let sdk = temp_dir("sdk");
        fs::create_dir_all(sdk.join("platforms/android-34")).unwrap();
        fs::create_dir_all(sdk.join("build-tools/34.0.0")).unwrap();
        sdk
    }

    fn make_fake_jdk() -> PathBuf {
        let jdk = temp_dir("jdk");
        let bin = jdk.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let java = bin.join(if cfg!(windows) { "java.exe" } else { "java" });
        fs::write(java, b"stub").unwrap();
        fs::write(jdk.join("release"), b"JAVA_VERSION=\"17.0.20.1\"\n").unwrap();
        jdk
    }

    fn toolchain(sdk: Option<&Path>, jdk: Option<&Path>, java_major: Option<u32>) -> AndroidToolchain {
        AndroidToolchain {
            sdk_root: sdk.map(|p| p.to_path_buf()),
            sdk_source: if sdk.is_some() { SdkSource::AndroidHome } else { SdkSource::Missing },
            java_home: jdk.map(|p| p.to_path_buf()),
            java_source: if jdk.is_some() { JavaSource::JavaHome } else { JavaSource::Missing },
            java_major,
        }
    }

    #[test]
    fn preflight_passes_with_sdk_and_java_home() {
        let sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let res = validate_android_toolchain(&toolchain(Some(&sdk), Some(&jdk), Some(17)), None);
        assert!(res.is_ok(), "expected OK, got {res:?}");
    }

    #[test]
    fn preflight_passes_with_sdk_and_java_on_path_only() {
        let sdk = make_fake_sdk();
        let mut tc = toolchain(Some(&sdk), None, Some(17));
        tc.java_source = JavaSource::Path;
        let res = validate_android_toolchain(&tc, None);
        assert!(res.is_ok(), "expected OK, got {res:?}");
    }

    #[test]
    fn preflight_fails_closed_when_sdk_missing() {
        let jdk = make_fake_jdk();
        let res = validate_android_toolchain(&toolchain(None, Some(&jdk), Some(17)), None);
        let err = res.expect_err("missing SDK must fail closed");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING));
        let text = err.to_string();
        assert!(
            text.contains("ANDROID_HOME") && text.contains("ANDROID_SDK_ROOT"),
            "diagnostic must name the SDK environment variables, got: {text}"
        );
    }

    #[test]
    fn preflight_fails_closed_when_sdk_path_is_not_a_directory() {
        let jdk = make_fake_jdk();
        let bogus = temp_dir("bogus_sdk").join("not_a_dir");
        let res = validate_android_toolchain(&toolchain(Some(&bogus), Some(&jdk), Some(17)), None);
        assert_eq!(
            res.expect_err("non-directory SDK must fail closed").code(),
            Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING)
        );
    }

    #[test]
    fn preflight_fails_closed_when_jdk_missing() {
        let sdk = make_fake_sdk();
        let res = validate_android_toolchain(&toolchain(Some(&sdk), None, None), None);
        let err = res.expect_err("missing JDK must fail closed");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING));
        assert!(
            err.to_string().contains("JAVA_HOME"),
            "diagnostic must name JAVA_HOME, got: {err}"
        );
    }

    // --- M-3: pinned SDK components must actually be validated -------------

    #[test]
    fn preflight_fails_when_sdk_lacks_pinned_platform() {
        let sdk = temp_dir("sdk_no_platform");
        fs::create_dir_all(sdk.join("build-tools/34.0.0")).unwrap();
        let jdk = make_fake_jdk();
        let err = validate_android_toolchain(&toolchain(Some(&sdk), Some(&jdk), Some(17)), None)
            .expect_err("SDK without platforms/android-34 must fail");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING));
        let text = err.to_string();
        assert!(
            text.contains("platforms/android-34"),
            "diagnostic must name the missing platform component, got: {text}"
        );
    }

    #[test]
    fn preflight_fails_when_sdk_lacks_pinned_build_tools() {
        let sdk = temp_dir("sdk_no_build_tools");
        fs::create_dir_all(sdk.join("platforms/android-34")).unwrap();
        fs::create_dir_all(sdk.join("build-tools/33.0.1")).unwrap();
        let jdk = make_fake_jdk();
        let err = validate_android_toolchain(&toolchain(Some(&sdk), Some(&jdk), Some(17)), None)
            .expect_err("SDK without build-tools/34.0.0 must fail");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING));
        assert!(
            err.to_string().contains("build-tools/34.0.0"),
            "diagnostic must name the missing build-tools component, got: {err}"
        );
    }

    #[test]
    fn preflight_fails_when_jdk_major_below_17() {
        let sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let err = validate_android_toolchain(&toolchain(Some(&sdk), Some(&jdk), Some(11)), None)
            .expect_err("JDK 11 must fail the 17+ contract");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING));
        assert!(
            err.to_string().contains("11"),
            "diagnostic must report the detected major version, got: {err}"
        );
    }

    #[test]
    fn preflight_fails_when_jdk_major_undeterminable() {
        let sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let err = validate_android_toolchain(&toolchain(Some(&sdk), Some(&jdk), None), None)
            .expect_err("undeterminable JDK version must fail closed, not pass silently");
        assert_eq!(err.code(), Some(DiagnosticCode::BLD_TOOLCHAIN_MISSING));
    }

    // --- M-2: bundled JDK fallback must be accepted by preflight ----------

    #[test]
    fn preflight_passes_with_bundled_jdk_fallback() {
        let sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let (resolved, source) = resolve_java_home_from_values(None, Some(&jdk));
        assert_eq!(source, JavaSource::BundledToolchain);
        let tc = AndroidToolchain {
            sdk_root: Some(sdk.clone()),
            sdk_source: SdkSource::AndroidHome,
            java_home: resolved,
            java_source: source,
            java_major: read_java_major_from_release(&jdk),
        };
        let res = validate_android_toolchain(&tc, None);
        assert!(res.is_ok(), "bundled JDK must satisfy preflight, got {res:?}");
    }

    // --- M-1: local.properties precedence and Windows escaping ------------

    #[test]
    fn resolve_sdk_prefers_env_over_local_properties() {
        let env_sdk = temp_dir("env_sdk");
        let props_sdk = temp_dir("props_sdk");
        let (resolved, source) = resolve_sdk_root_from_values(
            Some(env_sdk.to_str().unwrap()),
            None,
            Some(&props_sdk),
            None,
        );
        assert_eq!(source, SdkSource::AndroidHome);
        assert_eq!(resolved.as_deref(), Some(env_sdk.as_path()));
    }

    #[test]
    fn resolve_sdk_falls_back_to_local_properties() {
        let props_sdk = temp_dir("props_only_sdk");
        let (resolved, source) =
            resolve_sdk_root_from_values(None, None, Some(&props_sdk), None);
        assert_eq!(source, SdkSource::LocalProperties);
        assert_eq!(resolved.as_deref(), Some(props_sdk.as_path()));
    }

    #[test]
    fn parse_local_properties_unescapes_windows_path() {
        let raw = "sdk.dir=C\\:\\\\Users\\\\dev\\\\AppData\\\\Local\\\\Android\\\\Sdk\n";
        let parsed = parse_local_properties_sdk_dir(raw).expect("must parse sdk.dir");
        assert_eq!(
            parsed.to_string_lossy().replace('\\', "/"),
            "C:/Users/dev/AppData/Local/Android/Sdk"
        );
    }

    #[test]
    fn parse_local_properties_handles_comments_and_forward_slashes() {
        let raw = "# generated\nsdk.dir=/home/dev/Android/Sdk\n";
        let parsed = parse_local_properties_sdk_dir(raw).expect("must parse sdk.dir");
        assert_eq!(
            parsed.to_string_lossy().replace('\\', "/"),
            "/home/dev/Android/Sdk"
        );
    }

    // --- L-2: empty / whitespace env values must not block fallbacks ------

    #[test]
    fn resolve_ignores_empty_and_whitespace_env() {
        let props_sdk = temp_dir("ws_props_sdk");
        let (resolved, source) = resolve_sdk_root_from_values(
            Some("   "),
            Some(""),
            Some(&props_sdk),
            None,
        );
        assert_eq!(
            source,
            SdkSource::LocalProperties,
            "blank ANDROID_HOME must not shadow local.properties"
        );
        assert_eq!(resolved.as_deref(), Some(props_sdk.as_path()));
    }

    // --- M-1: the remediation text must match what actually works ---------

    #[test]
    fn sdk_diagnostic_omits_local_properties_when_no_project_exists() {
        let jdk = make_fake_jdk();
        let err = validate_android_toolchain(&toolchain(None, Some(&jdk), Some(17)), None)
            .expect_err("missing SDK must fail");
        assert!(
            !err.to_string().contains("local.properties"),
            "must not suggest editing a project that does not exist yet, got: {err}"
        );
    }

    #[test]
    fn sdk_diagnostic_names_local_properties_for_a_real_project_dir() {
        let jdk = make_fake_jdk();
        let project = temp_dir("real_project");
        let err = validate_android_toolchain(&toolchain(None, Some(&jdk), Some(17)), Some(&project))
            .expect_err("missing SDK must fail");
        let text = err.to_string();
        assert!(
            text.contains("local.properties"),
            "a real project dir makes the sdk.dir hint actionable, got: {text}"
        );
        assert!(
            text.contains(&project.display().to_string()),
            "the hint must name the actual project path, got: {text}"
        );
    }

    // --- M-1/M-2: one resolver shared by both checks ----------------------

    #[test]
    fn merge_toolchain_carries_retained_local_properties_sdk_forward() {
        let sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let earlier = toolchain(Some(&sdk), Some(&jdk), Some(17));

        // Second pass: generated project has no local.properties, env still unset.
        let fresh = toolchain(None, Some(&jdk), Some(17));
        let merged = merge_toolchain(fresh, Some(&earlier));

        assert_eq!(merged.sdk_root.as_deref(), Some(sdk.as_path()));
        assert_eq!(merged.sdk_source, SdkSource::AndroidHome);
        assert!(
            validate_android_toolchain(&merged, None).is_ok(),
            "carried-forward SDK must satisfy the late check"
        );
    }

    #[test]
    fn merge_toolchain_prefers_fresh_values_when_present() {
        let old_sdk = make_fake_sdk();
        let new_sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let earlier = toolchain(Some(&old_sdk), Some(&jdk), Some(17));
        let fresh = toolchain(Some(&new_sdk), Some(&jdk), Some(17));

        let merged = merge_toolchain(fresh, Some(&earlier));
        assert_eq!(
            merged.sdk_root.as_deref(),
            Some(new_sdk.as_path()),
            "fresh resolution must win when it produced a value"
        );
    }

    #[test]
    fn merge_toolchain_without_earlier_is_identity() {
        let sdk = make_fake_sdk();
        let jdk = make_fake_jdk();
        let fresh = toolchain(Some(&sdk), Some(&jdk), Some(17));
        let merged = merge_toolchain(fresh.clone(), None);
        assert_eq!(merged.sdk_root, fresh.sdk_root);
        assert_eq!(merged.java_home, fresh.java_home);
    }

    // --- M-3 helpers ------------------------------------------------------

    #[test]
    fn parse_java_major_from_release_and_version_output() {
        assert_eq!(parse_java_major_from_version_text("JAVA_VERSION=\"17.0.20.1\""), Some(17));
        assert_eq!(
            parse_java_major_from_version_text("openjdk version \"17.0.20.1\" 2026-08-18"),
            Some(17)
        );
        assert_eq!(parse_java_major_from_version_text("java version \"1.8.0_402\""), Some(8));
        assert_eq!(parse_java_major_from_version_text("no version here"), None);
    }
}
