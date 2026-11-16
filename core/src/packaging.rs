//! Shipped-payload exclusion decision — the single source of truth.
//!
//! The packer (`runtimepack-pack`), the web analyzer (`scanner.rs`) and the web
//! auditor (`asset_audit.rs`) must agree on **which files the artifact
//! contains**. Before this module each surface carried its own opinion: the
//! packer dropped desktop runtime DLLs while the audit judged the source tree,
//! so a build could be blocked over a file that would never be shipped.
//!
//! Two exclusion classes exist, both structural facts about the target platform,
//! never capability claims:
//!
//! 1. **Known desktop runtime binaries** — a reviewed filename list of NW.js
//!    runner components, Windows redistributables and desktop-only SDKs, plus the
//!    directory-scoped desktop trees (`_commonredist/`, `swiftshader/`,
//!    `lib/py*-windows*`, ...). Reason: [`REASON_PLATFORM_BINARY`].
//! 2. **Windows PE images** with a `.dll` or `.node` extension — a PE image
//!    cannot be loaded by an Android WebView/ART process. Reason:
//!    [`REASON_WINDOWS_PE_BINARY`]. This check is *structural*: an ELF or
//!    Mach-O `.node` addon, or a `.dll` that is not a PE image, is not excluded
//!    by this rule.
//!
//! `.so` is never excluded by either rule (a shared object *can* run on the
//! target), and no JavaScript or asset file is ever excluded here.
//!
//! Every exclusion carries a reason string that lands in
//! `packaging-exclusions.json`, so a dropped path is always attributable.

use std::path::Path;

/// Reason recorded for a reviewed desktop runtime / desktop-only binary name.
pub const REASON_PLATFORM_BINARY: &str = "EXCLUDED_PLATFORM_BINARY";
/// Reason recorded for a structural Windows PE image (`.dll` / `.node`).
pub const REASON_WINDOWS_PE_BINARY: &str = "EXCLUDED_WINDOWS_PE_BINARY";

/// Bounded header read for structural detection. A PE `e_lfanew` beyond this
/// window is not treated as a PE image (the file then ships, which is the
/// conservative direction for an exclusion decision).
pub const MAX_HEADER_BYTES: usize = 4096;

fn lower(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn basename(lower_path: &str) -> &str {
    match lower_path.rfind('/') {
        Some(i) => &lower_path[i + 1..],
        None => lower_path,
    }
}

fn extension_is_native_image(lower_path: &str) -> bool {
    lower_path.ends_with(".dll") || lower_path.ends_with(".node")
}

/// True when `header` starts with a DOS header and a PE signature at `e_lfanew`.
///
/// Only the bounded prefix is inspected; no filesystem access happens here.
pub fn is_windows_pe_image(header: &[u8]) -> bool {
    if header.len() < 0x40 {
        return false;
    }
    if &header[0..2] != b"MZ" {
        return false;
    }
    let lfanew = u32::from_le_bytes([header[0x3C], header[0x3D], header[0x3E], header[0x3F]]) as usize;
    match lfanew.checked_add(4) {
        Some(end) if end <= header.len() => &header[lfanew..end] == b"PE\0\0",
        _ => false,
    }
}

/// Path-only exclusion decision (the reviewed name and directory list).
///
/// This is the historical packer predicate. It needs no file bytes, so it is the
/// fallback when a header cannot be read.
pub fn path_exclusion_reason(path: &str) -> Option<&'static str> {
    let lower = lower(path);
    if lower.contains("_commonredist/")
        || lower.starts_with("swiftshader/")
        || lower.contains("/swiftshader/")
        || lower.starts_with("lib/py3-windows")
        || lower.starts_with("lib/py2-windows")
        || lower.starts_with("lib/windows")
        || lower.contains("/py3-windows")
        || lower.contains("/py2-windows")
        || lower.contains("/windows-")
        || lower.starts_with("lib/py3-linux")
        || lower.starts_with("lib/py2-linux")
        || lower.starts_with("lib/linux")
        || lower.contains("/py3-linux")
        || lower.contains("/py2-linux")
        || lower.contains("/linux-")
        || (lower.starts_with("lib/") && (lower.contains("windows") || lower.contains("linux")))
    {
        return Some(REASON_PLATFORM_BINARY);
    }
    let filename = basename(&lower);
    if filename.ends_with(".dll") {
        if filename.starts_with("msvcr")
            || filename.starts_with("msvcp")
            || filename.starts_with("vcruntime")
            || filename.starts_with("ucrtbase")
            || filename.starts_with("d3d")
            || filename.starts_with("xinput")
            || filename == "nw.dll"
            || filename == "node.dll"
            || filename == "ffmpeg.dll"
            || filename == "nw_elf.dll"
            || filename == "libegl.dll"
            || filename == "libglesv2.dll"
            || filename.starts_with("d3dcompiler")
            || filename.starts_with("vk_swiftshader")
            || filename.starts_with("vulkan")
            || filename.starts_with("steam_api")
            || filename.starts_with("fluidsynth")
            || filename.starts_with("libfluidsynth")
            || filename.starts_with("utils")
            || filename == "discord_game_sdk.dll"
        {
            return Some(REASON_PLATFORM_BINARY);
        }
    }
    None
}

/// Exclusion decision from a path plus an optional file header.
///
/// The reviewed name/directory list wins (it is the reviewed set, and it is the
/// only rule available without bytes). Otherwise a PE `.dll`/`.node` image is
/// excluded structurally. Everything else — including `.so`, ELF/Mach-O `.node`
/// addons and non-PE `.dll` files — ships.
pub fn exclusion_reason(path: &str, header: Option<&[u8]>) -> Option<&'static str> {
    if let Some(reason) = path_exclusion_reason(path) {
        return Some(reason);
    }
    let header = header?;
    if !extension_is_native_image(&lower(path)) {
        return None;
    }
    if is_windows_pe_image(header) {
        return Some(REASON_WINDOWS_PE_BINARY);
    }
    None
}

/// Exclusion decision for a real file, reading a bounded header when needed.
///
/// An unreadable file is *not* excluded: an exclusion must be attributable, and a
/// read failure is not evidence about the bytes.
pub fn exclusion_reason_for_real_file(path: &str, real_path: &Path) -> Option<&'static str> {
    if let Some(reason) = path_exclusion_reason(path) {
        return Some(reason);
    }
    if !extension_is_native_image(&lower(path)) {
        return None;
    }
    let header = read_header(real_path)?;
    exclusion_reason(path, Some(&header))
}

fn read_header(real_path: &Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(real_path).ok()?;
    let mut header = Vec::with_capacity(MAX_HEADER_BYTES);
    file.take(MAX_HEADER_BYTES as u64)
        .read_to_end(&mut header)
        .ok()?;
    Some(header)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pe_bytes() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x80];
        bytes[0] = b'M';
        bytes[1] = b'Z';
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes
    }

    #[test]
    fn pe_signature_is_detected_structurally() {
        assert!(is_windows_pe_image(&pe_bytes()));
        assert!(!is_windows_pe_image(b"MZ"));
        assert!(!is_windows_pe_image(b"\x7fELF\x02\x01\x01\x00"));
        // A DOS header whose e_lfanew points outside the bounded window is not
        // treated as a PE image.
        let mut truncated = pe_bytes();
        truncated[0x3C..0x40].copy_from_slice(&0xFFFFu32.to_le_bytes());
        assert!(!is_windows_pe_image(&truncated));
    }

    #[test]
    fn reviewed_names_win_and_record_the_platform_reason() {
        assert_eq!(
            exclusion_reason("lib/steam_api64.dll", None),
            Some(REASON_PLATFORM_BINARY)
        );
        assert_eq!(
            exclusion_reason("lib/steam_api64.dll", Some(&pe_bytes())),
            Some(REASON_PLATFORM_BINARY),
            "the reviewed name is the reason even for a PE image"
        );
        assert_eq!(
            exclusion_reason("swiftshader/libEGL.dll", None),
            Some(REASON_PLATFORM_BINARY)
        );
    }

    #[test]
    fn structural_rule_is_pe_only_for_dll_and_node() {
        assert_eq!(
            exclusion_reason("lib/sdkencryptedappticket.dll", Some(&pe_bytes())),
            Some(REASON_WINDOWS_PE_BINARY)
        );
        assert_eq!(
            exclusion_reason("lib/greenworks-win64.node", Some(&pe_bytes())),
            Some(REASON_WINDOWS_PE_BINARY)
        );
        // A reviewed Ren'Py desktop directory is excluded by path, and the path
        // reason is recorded even for a non-PE image.
        assert_eq!(
            exclusion_reason("lib/greenworks-linux.node", Some(b"\x7fELF")),
            Some(REASON_PLATFORM_BINARY)
        );
        // Outside such a directory a non-Windows native image ships: it can
        // execute on the target.
        assert_eq!(
            exclusion_reason("addons/greenworks-linux.node", Some(b"\x7fELF")),
            None
        );
        assert_eq!(
            exclusion_reason(
                "addons/greenworks-macos.node",
                Some(&[0xcf, 0xfa, 0xed, 0xfe])
            ),
            None
        );
        assert_eq!(
            exclusion_reason("addons/libacme.so", Some(&pe_bytes())),
            None
        );
        // A `.dll` that is not a PE image is not excluded by this rule alone.
        assert_eq!(
            exclusion_reason("addons/notpe.dll", Some(b"MZ")),
            None
        );
        // Bytes are required for the structural rule; without them nothing is
        // dropped on structural grounds.
        assert_eq!(
            exclusion_reason("addons/sdkencryptedappticket.dll", None),
            None
        );
    }

    #[test]
    fn js_and_assets_are_never_excluded() {
        for path in ["js/plugins/OrangeMapshot.js", "data/System.json", "img/x.png"] {
            assert_eq!(exclusion_reason(path, Some(&pe_bytes())), None, "{path}");
        }
    }
}
