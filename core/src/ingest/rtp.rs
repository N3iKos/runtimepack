use std::fs;
use std::path::PathBuf;

/// Check if two RTP names match, accounting for case and standard aliases.
pub fn matches_rtp_name(candidate: &str, target: &str) -> bool {
    let c = candidate.trim().to_ascii_lowercase();
    let t = target.trim().to_ascii_lowercase();
    if c == t {
        return true;
    }
    let is_vxace = |s: &str| s == "rpgvxace" || s == "vxace" || s == "vx_ace";
    if is_vxace(&c) && is_vxace(&t) {
        return true;
    }
    let is_vx = |s: &str| s == "rpgvx" || s == "vx";
    if is_vx(&c) && is_vx(&t) {
        return true;
    }
    let is_xp = |s: &str| s == "rpgxp" || s == "xp" || s == "standard";
    if is_xp(&c) && is_xp(&t) {
        return true;
    }
    false
}

/// Parse the `[Game]` section of a Game.ini content string and return the first non-empty RTP name if declared.
pub fn parse_ini_rtp_name(ini_content: &str) -> Option<String> {
    let content = ini_content.strip_prefix('\u{feff}').unwrap_or(ini_content);
    let mut in_game_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let section = trimmed[1..trimmed.len() - 1].trim();
            if section.eq_ignore_ascii_case("Game") {
                in_game_section = true;
            } else if in_game_section {
                break;
            }
            continue;
        }
        if in_game_section {
            if let Some((key, val)) = trimmed.split_once('=') {
                let key = key.trim();
                let val = val.trim();
                if (key.eq_ignore_ascii_case("RTP")
                    || key.eq_ignore_ascii_case("RTP1")
                    || key.eq_ignore_ascii_case("RTP2")
                    || key.eq_ignore_ascii_case("RTP3"))
                    && !val.is_empty()
                {
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

/// Find system or repository RTP path for a given RTP name (e.g. "RPGVXAce", "RPGVX", "Standard", "RPGXP").
///
/// Order of precedence:
/// 1. Environment variable `{RTP_NAME}_RTP_PATH` (e.g. `RPGVXACE_RTP_PATH`)
/// 2. Local repository directory `runtimes/rtp/{rtp_name_lowercase}/` (e.g. `runtimes/rtp/vxace` or `runtimes/rtp/rpgvxace`)
/// 3. Standard Windows Common Files:
///    - `CommonProgramFiles(x86)\Enterbrain\RGSS3\{rtp_name}`
///    - `CommonProgramFiles\Enterbrain\RGSS3\{rtp_name}`
///    - `C:\Program Files (x86)\Common Files\Enterbrain\RGSS3\{rtp_name}`
///    - `C:\Program Files\Common Files\Enterbrain\RGSS3\{rtp_name}`
///    (for RGSS2 check `RGSS2`, for RGSS1 check `RGSS`)
/// 4. Windows Registry:
///    - `SOFTWARE\WOW6432Node\Enterbrain\RGSS3\RTP` or `SOFTWARE\Enterbrain\RGSS3\RTP`
pub fn find_system_rtp_path(rtp_name: &str) -> Option<PathBuf> {
    let clean = rtp_name.trim();
    if clean.is_empty() {
        return None;
    }

    // a. Environment variable: {RTP_NAME}_RTP_PATH
    if let Some(p) = find_rtp_env_var(clean) {
        return Some(p);
    }

    // b. Local repository directory: runtimes/rtp/{rtp_name_lowercase}/
    if let Some(p) = find_rtp_local_repo(clean) {
        return Some(p);
    }

    // c. Standard Windows Common Files
    if let Some(p) = find_rtp_windows_common_files(clean) {
        return Some(p);
    }

    // d. Windows Registry
    #[cfg(windows)]
    if let Some(p) = find_rtp_windows_registry(clean) {
        return Some(p);
    }

    None
}

fn find_rtp_env_var(rtp_name: &str) -> Option<PathBuf> {
    let upper = rtp_name.replace(' ', "_").to_ascii_uppercase();
    let candidates = [
        format!("{upper}_RTP_PATH"),
        format!("{}_RTP_PATH", rtp_name.trim()),
    ];

    for var in &candidates {
        if let Ok(val) = std::env::var(var) {
            let p = PathBuf::from(val.trim());
            if p.is_dir() {
                return fs::canonicalize(&p).ok().or(Some(p));
            }
        }
    }
    None
}

fn find_rtp_local_repo(rtp_name: &str) -> Option<PathBuf> {
    let lower = rtp_name.to_ascii_lowercase();
    let mut aliases = vec![lower.clone()];
    if lower == "rpgvxace" || lower == "vxace" || lower == "vx_ace" {
        for a in ["vxace", "rpgvxace", "vx_ace"] {
            if !aliases.contains(&a.to_string()) {
                aliases.push(a.to_string());
            }
        }
    } else if lower == "rpgvx" || lower == "vx" {
        for a in ["vx", "rpgvx"] {
            if !aliases.contains(&a.to_string()) {
                aliases.push(a.to_string());
            }
        }
    } else if lower == "rpgxp" || lower == "xp" || lower == "standard" {
        for a in ["xp", "rpgxp", "standard"] {
            if !aliases.contains(&a.to_string()) {
                aliases.push(a.to_string());
            }
        }
    }

    let mut check_roots = Vec::new();
    if let Ok(curr) = std::env::current_dir() {
        let mut dir = curr;
        for _ in 0..5 {
            check_roots.push(dir.clone());
            if !dir.pop() {
                break;
            }
        }
    }

    for root in check_roots {
        let rtp_base = root.join("runtimes").join("rtp");
        if rtp_base.is_dir() {
            for alias in &aliases {
                let candidate = rtp_base.join(alias);
                if candidate.is_dir() {
                    return fs::canonicalize(&candidate).ok().or(Some(candidate));
                }
            }
        }
    }

    None
}

fn find_rtp_windows_common_files(rtp_name: &str) -> Option<PathBuf> {
    let mut common_bases = Vec::new();
    if let Ok(val) = std::env::var("CommonProgramFiles(x86)") {
        common_bases.push(PathBuf::from(val));
    }
    if let Ok(val) = std::env::var("CommonProgramFiles") {
        common_bases.push(PathBuf::from(val));
    }
    #[cfg(windows)]
    {
        common_bases.push(PathBuf::from(r"C:\Program Files (x86)\Common Files"));
        common_bases.push(PathBuf::from(r"C:\Program Files\Common Files"));
    }

    let lower = rtp_name.to_ascii_lowercase();
    let rgss_dirs: &[&str] = if lower.contains("vxace") || lower.contains("rgss3") {
        &["RGSS3", "RGSS2", "RGSS"]
    } else if lower.contains("vx") || lower.contains("rgss2") {
        &["RGSS2", "RGSS3", "RGSS"]
    } else if lower.contains("xp") || lower.contains("rgss1") || lower.contains("standard") {
        &["RGSS", "RGSS3", "RGSS2"]
    } else {
        &["RGSS3", "RGSS2", "RGSS"]
    };

    for base in common_bases {
        for rgss_sub in rgss_dirs {
            let enterbrain_rgss = base.join("Enterbrain").join(rgss_sub);
            if enterbrain_rgss.is_dir() {
                let direct = enterbrain_rgss.join(rtp_name);
                if direct.is_dir() {
                    return fs::canonicalize(&direct).ok().or(Some(direct));
                }
                if let Ok(entries) = fs::read_dir(&enterbrain_rgss) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if p.is_dir() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            if matches_rtp_name(&name, rtp_name) {
                                return fs::canonicalize(&p).ok().or(Some(p));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn find_rtp_windows_registry(rtp_name: &str) -> Option<PathBuf> {
    let lower = rtp_name.to_ascii_lowercase();
    let registry_keys: &[&str] = if lower.contains("vxace") || lower.contains("rgss3") {
        &[
            r"HKLM\SOFTWARE\WOW6432Node\Enterbrain\RGSS3\RTP",
            r"HKLM\SOFTWARE\Enterbrain\RGSS3\RTP",
            r"HKCU\SOFTWARE\WOW6432Node\Enterbrain\RGSS3\RTP",
            r"HKCU\SOFTWARE\Enterbrain\RGSS3\RTP",
        ]
    } else if lower.contains("vx") || lower.contains("rgss2") {
        &[
            r"HKLM\SOFTWARE\WOW6432Node\Enterbrain\RGSS2\RTP",
            r"HKLM\SOFTWARE\Enterbrain\RGSS2\RTP",
            r"HKCU\SOFTWARE\WOW6432Node\Enterbrain\RGSS2\RTP",
            r"HKCU\SOFTWARE\Enterbrain\RGSS2\RTP",
        ]
    } else if lower.contains("xp") || lower.contains("rgss1") || lower.contains("standard") {
        &[
            r"HKLM\SOFTWARE\WOW6432Node\Enterbrain\RGSS\RTP",
            r"HKLM\SOFTWARE\Enterbrain\RGSS\RTP",
            r"HKCU\SOFTWARE\WOW6432Node\Enterbrain\RGSS\RTP",
            r"HKCU\SOFTWARE\Enterbrain\RGSS\RTP",
        ]
    } else {
        &[
            r"HKLM\SOFTWARE\WOW6432Node\Enterbrain\RGSS3\RTP",
            r"HKLM\SOFTWARE\Enterbrain\RGSS3\RTP",
            r"HKLM\SOFTWARE\WOW6432Node\Enterbrain\RGSS2\RTP",
            r"HKLM\SOFTWARE\Enterbrain\RGSS2\RTP",
            r"HKLM\SOFTWARE\WOW6432Node\Enterbrain\RGSS\RTP",
            r"HKLM\SOFTWARE\Enterbrain\RGSS\RTP",
        ]
    };

    for key in registry_keys {
        if let Ok(output) = std::process::Command::new("reg")
            .args(["query", key])
            .output()
        {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    let trimmed = line.trim();
                    if let Some((val_name, rest)) = trimmed.split_once("REG_SZ") {
                        if matches_rtp_name(val_name.trim(), rtp_name) {
                            let path_str = rest.trim();
                            let p = PathBuf::from(path_str);
                            if p.is_dir() {
                                return fs::canonicalize(&p).ok().or(Some(p));
                            }
                        }
                    } else if let Some((val_name, rest)) = trimmed.split_once("REG_EXPAND_SZ") {
                        if matches_rtp_name(val_name.trim(), rtp_name) {
                            let path_str = rest.trim();
                            let p = PathBuf::from(path_str);
                            if p.is_dir() {
                                return fs::canonicalize(&p).ok().or(Some(p));
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matches_rtp_name() {
        assert!(matches_rtp_name("RPGVXAce", "rpgvxace"));
        assert!(matches_rtp_name("vxace", "RPGVXAce"));
        assert!(matches_rtp_name("RPGVX", "vx"));
        assert!(matches_rtp_name("RPGXP", "standard"));
        assert!(matches_rtp_name("Standard", "xp"));
        assert!(!matches_rtp_name("RPGVXAce", "RPGVX"));
    }

    #[test]
    fn test_parse_ini_rtp_name() {
        let ini = "[Game]\r\nLibrary=System\\RGSS301.dll\r\nScripts=Data\\Scripts.rvdata2\r\nTitle=Test\r\nRTP=RPGVXAce\r\n";
        assert_eq!(parse_ini_rtp_name(ini), Some("RPGVXAce".to_string()));

        let ini_space = "[Game]\nrtp = RPGVXAce\n";
        assert_eq!(parse_ini_rtp_name(ini_space), Some("RPGVXAce".to_string()));

        let ini_xp = "[Game]\nLibrary=RGSS104E.dll\nScripts=Data\\Scripts.rxdata\nRTP1=Standard\nRTP2=\n";
        assert_eq!(parse_ini_rtp_name(ini_xp), Some("Standard".to_string()));

        let ini_no_rtp = "[Game]\nTitle=No RTP\n";
        assert_eq!(parse_ini_rtp_name(ini_no_rtp), None);

        let ini_other_section = "[Other]\nRTP=RPGVXAce\n[Game]\nTitle=None\n";
        assert_eq!(parse_ini_rtp_name(ini_other_section), None);
    }
}
