use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};
use std::fs;
use std::path::{Path, PathBuf};

pub const MIPMAP_TARGETS: &[(&str, u32)] = &[
    ("mipmap-mdpi", 48),
    ("mipmap-hdpi", 72),
    ("mipmap-xhdpi", 96),
    ("mipmap-xxhdpi", 144),
    ("mipmap-xxxhdpi", 192),
];

const NEUTRAL_MDPI: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-mdpi/ic_launcher.png");
const NEUTRAL_MDPI_ROUND: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-mdpi/ic_launcher_round.png");
const NEUTRAL_HDPI: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-hdpi/ic_launcher.png");
const NEUTRAL_HDPI_ROUND: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-hdpi/ic_launcher_round.png");
const NEUTRAL_XHDPI: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-xhdpi/ic_launcher.png");
const NEUTRAL_XHDPI_ROUND: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-xhdpi/ic_launcher_round.png");
const NEUTRAL_XXHDPI: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-xxhdpi/ic_launcher.png");
const NEUTRAL_XXHDPI_ROUND: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-xxhdpi/ic_launcher_round.png");
const NEUTRAL_XXXHDPI: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-xxxhdpi/ic_launcher.png");
const NEUTRAL_XXXHDPI_ROUND: &[u8] =
    include_bytes!("../../../android/shell/app/src/main/res/mipmap-xxxhdpi/ic_launcher_round.png");

#[derive(Debug, Clone)]
struct IconDirEntry {
    b_width: u8,
    b_height: u8,
    b_color_count: u8,
    b_reserved: u8,
    w_planes: u16,
    w_bit_count: u16,
    _dw_bytes_in_res: u32,
    n_id: u16,
}

pub fn extract_best_icon_from_pe(bytes: &[u8]) -> Option<DynamicImage> {
    if bytes.len() < 0x40 || &bytes[0..2] != b"MZ" {
        return None;
    }
    let e_lfanew = u32::from_le_bytes(bytes[0x3C..0x40].try_into().ok()?) as usize;
    if e_lfanew + 24 > bytes.len() || &bytes[e_lfanew..e_lfanew + 4] != b"PE\0\0" {
        return None;
    }
    let num_sections =
        u16::from_le_bytes(bytes[e_lfanew + 6..e_lfanew + 8].try_into().ok()?) as usize;
    let size_of_optional_header =
        u16::from_le_bytes(bytes[e_lfanew + 20..e_lfanew + 22].try_into().ok()?) as usize;

    let opt_header_offset = e_lfanew + 24;
    if opt_header_offset + size_of_optional_header > bytes.len() {
        return None;
    }
    let magic =
        u16::from_le_bytes(bytes[opt_header_offset..opt_header_offset + 2].try_into().ok()?);
    let data_dir_offset = match magic {
        0x10B => opt_header_offset + 96,
        0x20B => opt_header_offset + 112,
        _ => return None,
    };

    // Entry 2 is Resource Directory
    let rsrc_dir_entry_offset = data_dir_offset + 16;
    if rsrc_dir_entry_offset + 8 > bytes.len() {
        return None;
    }
    let rsrc_rva = u32::from_le_bytes(
        bytes[rsrc_dir_entry_offset..rsrc_dir_entry_offset + 4]
            .try_into()
            .ok()?,
    );
    let rsrc_size = u32::from_le_bytes(
        bytes[rsrc_dir_entry_offset + 4..rsrc_dir_entry_offset + 8]
            .try_into()
            .ok()?,
    );
    if rsrc_rva == 0 || rsrc_size == 0 {
        return None;
    }

    let sec_table_offset = opt_header_offset + size_of_optional_header;
    if sec_table_offset + num_sections * 40 > bytes.len() {
        return None;
    }

    let mut rsrc_sec = None;
    for i in 0..num_sections {
        let sec_off = sec_table_offset + i * 40;
        let va = u32::from_le_bytes(bytes[sec_off + 12..sec_off + 16].try_into().ok()?);
        let raw_size = u32::from_le_bytes(bytes[sec_off + 16..sec_off + 20].try_into().ok()?);
        let raw_ptr = u32::from_le_bytes(bytes[sec_off + 20..sec_off + 24].try_into().ok()?);
        if rsrc_rva >= va && rsrc_rva < va + raw_size {
            rsrc_sec = Some((va, raw_size, raw_ptr));
            break;
        }
    }
    let (va_base, _, raw_base) = rsrc_sec?;
    let rva_to_offset = |rva: u32| -> Option<usize> {
        if rva >= va_base {
            let off = (rva - va_base + raw_base) as usize;
            if off < bytes.len() {
                Some(off)
            } else {
                None
            }
        } else {
            None
        }
    };

    let rsrc_root_offset = rva_to_offset(rsrc_rva)?;

    let parse_resource_dir = |dir_offset: usize| -> Option<Vec<(u32, u32)>> {
        if dir_offset + 16 > bytes.len() {
            return None;
        }
        let named_entries =
            u16::from_le_bytes(bytes[dir_offset + 12..dir_offset + 14].try_into().ok()?) as usize;
        let id_entries =
            u16::from_le_bytes(bytes[dir_offset + 14..dir_offset + 16].try_into().ok()?) as usize;
        let total = named_entries + id_entries;
        let entries_start = dir_offset + 16;
        if entries_start + total * 8 > bytes.len() {
            return None;
        }
        let mut result = Vec::with_capacity(total);
        for i in 0..total {
            let e_off = entries_start + i * 8;
            let id = u32::from_le_bytes(bytes[e_off..e_off + 4].try_into().ok()?);
            let data_off = u32::from_le_bytes(bytes[e_off + 4..e_off + 8].try_into().ok()?);
            result.push((id, data_off));
        }
        Some(result)
    };

    let root_entries = parse_resource_dir(rsrc_root_offset)?;
    // RT_ICON = 3, RT_GROUP_ICON = 14
    let grp_entry = root_entries.iter().find(|(id, _)| *id == 14)?;
    let icon_entry = root_entries.iter().find(|(id, _)| *id == 3)?;

    let grp_dir_offset = rsrc_root_offset + ((grp_entry.1 & 0x7FFF_FFFF) as usize);
    let grp_dir = parse_resource_dir(grp_dir_offset)?;
    let first_grp = grp_dir.first()?;

    let lang_dir_offset = rsrc_root_offset + ((first_grp.1 & 0x7FFF_FFFF) as usize);
    let lang_dir = parse_resource_dir(lang_dir_offset)?;
    let first_lang = lang_dir.first()?;

    let data_entry_off = rsrc_root_offset + (first_lang.1 as usize);
    if data_entry_off + 8 > bytes.len() {
        return None;
    }
    let grp_data_rva =
        u32::from_le_bytes(bytes[data_entry_off..data_entry_off + 4].try_into().ok()?);
    let grp_data_sz = u32::from_le_bytes(
        bytes[data_entry_off + 4..data_entry_off + 8]
            .try_into()
            .ok()?,
    ) as usize;
    let grp_file_off = rva_to_offset(grp_data_rva)?;
    if grp_file_off + grp_data_sz > bytes.len() || grp_data_sz < 6 {
        return None;
    }

    let grp_slice = &bytes[grp_file_off..grp_file_off + grp_data_sz];
    let icon_count = u16::from_le_bytes(grp_slice[4..6].try_into().ok()?) as usize;
    if grp_slice.len() < 6 + icon_count * 14 {
        return None;
    }

    let mut best_entry: Option<IconDirEntry> = None;
    let mut best_score: u64 = 0;

    for i in 0..icon_count {
        let e_slice = &grp_slice[6 + i * 14..6 + (i + 1) * 14];
        let w = e_slice[0];
        let h = e_slice[1];
        let color_count = e_slice[2];
        let reserved = e_slice[3];
        let planes = u16::from_le_bytes(e_slice[4..6].try_into().ok()?);
        let bit_count = u16::from_le_bytes(e_slice[6..8].try_into().ok()?);
        let bytes_in_res = u32::from_le_bytes(e_slice[8..12].try_into().ok()?);
        let icon_id = u16::from_le_bytes(e_slice[12..14].try_into().ok()?);

        let rw = if w == 0 { 256u64 } else { w as u64 };
        let rh = if h == 0 { 256u64 } else { h as u64 };
        let bpp = if bit_count > 0 { bit_count as u64 } else { 1 };
        let score = rw * rh * bpp;

        if score > best_score {
            best_score = score;
            best_entry = Some(IconDirEntry {
                b_width: w,
                b_height: h,
                b_color_count: color_count,
                b_reserved: reserved,
                w_planes: planes,
                w_bit_count: bit_count,
                _dw_bytes_in_res: bytes_in_res,
                n_id: icon_id,
            });
        }
    }

    let best = best_entry?;

    // Now find best.n_id in RT_ICON (type 3)
    let icon_type_dir_off = rsrc_root_offset + ((icon_entry.1 & 0x7FFF_FFFF) as usize);
    let icon_type_dir = parse_resource_dir(icon_type_dir_off)?;
    let matching_icon = icon_type_dir.iter().find(|(id, _)| *id == best.n_id as u32)?;

    let icon_lang_dir_off = rsrc_root_offset + ((matching_icon.1 & 0x7FFF_FFFF) as usize);
    let icon_lang_dir = parse_resource_dir(icon_lang_dir_off)?;
    let icon_lang = icon_lang_dir.first()?;

    let icon_data_entry_off = rsrc_root_offset + (icon_lang.1 as usize);
    if icon_data_entry_off + 8 > bytes.len() {
        return None;
    }
    let raw_icon_rva = u32::from_le_bytes(
        bytes[icon_data_entry_off..icon_data_entry_off + 4]
            .try_into()
            .ok()?,
    );
    let raw_icon_sz = u32::from_le_bytes(
        bytes[icon_data_entry_off + 4..icon_data_entry_off + 8]
            .try_into()
            .ok()?,
    ) as usize;
    let raw_icon_off = rva_to_offset(raw_icon_rva)?;
    if raw_icon_off + raw_icon_sz > bytes.len() {
        return None;
    }
    let raw_icon_bytes = &bytes[raw_icon_off..raw_icon_off + raw_icon_sz];

    // Check if raw icon is PNG
    if raw_icon_bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        if let Ok(img) = image::load_from_memory_with_format(raw_icon_bytes, ImageFormat::Png) {
            return Some(img);
        }
    }

    // Wrap into standard ICO format
    let mut ico_buf = Vec::with_capacity(22 + raw_icon_bytes.len());
    ico_buf.extend_from_slice(&0u16.to_le_bytes()); // idReserved
    ico_buf.extend_from_slice(&1u16.to_le_bytes()); // idType = 1 (icon)
    ico_buf.extend_from_slice(&1u16.to_le_bytes()); // idCount = 1
    ico_buf.push(best.b_width);
    ico_buf.push(best.b_height);
    ico_buf.push(best.b_color_count);
    ico_buf.push(best.b_reserved);
    ico_buf.extend_from_slice(&best.w_planes.to_le_bytes());
    ico_buf.extend_from_slice(&best.w_bit_count.to_le_bytes());
    ico_buf.extend_from_slice(&(raw_icon_bytes.len() as u32).to_le_bytes());
    ico_buf.extend_from_slice(&22u32.to_le_bytes()); // offset 22
    ico_buf.extend_from_slice(raw_icon_bytes);

    if let Ok(img) = image::load_from_memory_with_format(&ico_buf, ImageFormat::Ico) {
        return Some(img);
    }

    // Fallback: try Python PIL helper if available
    decode_ico_with_python(&ico_buf)
}

fn decode_ico_with_python(ico_data: &[u8]) -> Option<DynamicImage> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new("python")
        .arg("-c")
        .arg("import sys, io; from PIL import Image; img = Image.open(io.BytesIO(sys.stdin.buffer.read())).convert('RGBA'); buf = io.BytesIO(); img.save(buf, format='PNG'); sys.stdout.buffer.write(buf.getvalue())")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(ico_data);
    }

    let output = child.wait_with_output().ok()?;
    if output.status.success() && !output.stdout.is_empty() {
        image::load_from_memory_with_format(&output.stdout, ImageFormat::Png).ok()
    } else {
        None
    }
}

pub fn create_round_icon(square: &RgbaImage) -> RgbaImage {
    let mut round = square.clone();
    let w = round.width();
    let h = round.height();
    let cx = (w as f32) / 2.0;
    let cy = (h as f32) / 2.0;
    let r = (w.min(h) as f32) / 2.0;

    for y in 0..h {
        for x in 0..w {
            let dx = (x as f32 + 0.5) - cx;
            let dy = (y as f32 + 0.5) - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let p = round.get_pixel_mut(x, y);
            if dist >= r {
                p[3] = 0;
            } else if dist > r - 1.0 {
                let factor = r - dist;
                p[3] = ((p[3] as f32) * factor).round() as u8;
            }
        }
    }
    round
}

pub fn find_candidate_executables(game_dir: &Path, app_name: Option<&str>) -> Vec<PathBuf> {
    let mut direct_exes = Vec::new();
    if !game_dir.is_dir() {
        return direct_exes;
    }

    if let Ok(entries) = fs::read_dir(game_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.eq_ignore_ascii_case("exe") {
                        direct_exes.push(path);
                    }
                }
            }
        }
    }

    // If no direct exes, search 1 level deep (e.g. www/ or game/)
    if direct_exes.is_empty() {
        if let Ok(entries) = fs::read_dir(game_dir) {
            for entry in entries.flatten() {
                let sub = entry.path();
                if sub.is_dir() {
                    if let Ok(sub_entries) = fs::read_dir(&sub) {
                        for sub_entry in sub_entries.flatten() {
                            let p = sub_entry.path();
                            if p.is_file() {
                                if let Some(ext) = p.extension() {
                                    if ext.eq_ignore_ascii_case("exe") {
                                        direct_exes.push(p);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let app_clean = app_name.map(|n| format!("{}.exe", n.to_lowercase().replace(' ', "")));
    let app_raw = app_name.map(|n| format!("{}.exe", n.to_lowercase()));

    direct_exes.sort_by(|a, b| {
        let name_a = a
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let name_b = b
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        let rank = |name: &str| -> u8 {
            if name == "game.exe" {
                0
            } else if let Some(ref ac) = app_clean {
                if name == ac.as_str() {
                    1
                } else if let Some(ref ar) = app_raw {
                    if name == ar.as_str() {
                        1
                    } else {
                        2
                    }
                } else {
                    2
                }
            } else {
                2
            }
        };

        rank(&name_a).cmp(&rank(&name_b)).then_with(|| name_a.cmp(&name_b))
    });

    direct_exes
}

pub fn ensure_neutral_icons(res_dir: &Path) {
    let mapping: &[(&str, &[u8], &[u8])] = &[
        ("mipmap-mdpi", NEUTRAL_MDPI, NEUTRAL_MDPI_ROUND),
        ("mipmap-hdpi", NEUTRAL_HDPI, NEUTRAL_HDPI_ROUND),
        ("mipmap-xhdpi", NEUTRAL_XHDPI, NEUTRAL_XHDPI_ROUND),
        ("mipmap-xxhdpi", NEUTRAL_XXHDPI, NEUTRAL_XXHDPI_ROUND),
        ("mipmap-xxxhdpi", NEUTRAL_XXXHDPI, NEUTRAL_XXXHDPI_ROUND),
    ];

    for (folder, sq, rd) in mapping {
        let out_dir = res_dir.join(folder);
        let _ = fs::create_dir_all(&out_dir);
        let sq_path = out_dir.join("ic_launcher.png");
        let rd_path = out_dir.join("ic_launcher_round.png");
        if !sq_path.exists() {
            let _ = fs::write(sq_path, sq);
        }
        if !rd_path.exists() {
            let _ = fs::write(rd_path, rd);
        }
    }
}

pub fn extract_and_generate_icons(
    game_source_dir: &Path,
    app_name: Option<&str>,
    res_dir: &Path,
) -> bool {
    let candidate_exes = find_candidate_executables(game_source_dir, app_name);
    let mut extracted_img = None;

    for exe_path in candidate_exes {
        if let Ok(bytes) = fs::read(&exe_path) {
            if let Some(img) = extract_best_icon_from_pe(&bytes) {
                extracted_img = Some(img);
                break;
            }
        }
    }

    if let Some(img) = extracted_img {
        let rgba = img.to_rgba8();
        for (folder, size) in MIPMAP_TARGETS {
            let out_dir = res_dir.join(folder);
            let _ = fs::create_dir_all(&out_dir);

            let resized = image::imageops::resize(&rgba, *size, *size, FilterType::Lanczos3);
            let _ = resized.save_with_format(out_dir.join("ic_launcher.png"), ImageFormat::Png);

            let round = create_round_icon(&resized);
            let _ =
                round.save_with_format(out_dir.join("ic_launcher_round.png"), ImageFormat::Png);
        }
        true
    } else {
        ensure_neutral_icons(res_dir);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_icon_from_pe() {
        let sample_exe = Path::new("../Raw-Game/BLACK SOULS/Game.exe");
        if sample_exe.exists() {
            let bytes = fs::read(sample_exe).unwrap();
            let img = extract_best_icon_from_pe(&bytes);
            assert!(img.is_some(), "Expected icon extracted from BLACK SOULS/Game.exe");
            let img = img.unwrap();
            assert_eq!(img.width(), 256);
            assert_eq!(img.height(), 256);
        }
    }

    #[test]
    fn test_extract_and_generate_icons_flow() {
        let temp_dir = std::env::temp_dir().join(format!("rtp_icon_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let res_dir = temp_dir.join("res");
        let sample_dir = Path::new("../Raw-Game/The_Witchs_House_MV");
        if sample_dir.exists() {
            let extracted = extract_and_generate_icons(sample_dir, Some("The Witch's House"), &res_dir);
            assert!(extracted, "Should extract icon from The Witch's House MV");
            for (folder, size) in MIPMAP_TARGETS {
                let sq = res_dir.join(folder).join("ic_launcher.png");
                let rd = res_dir.join(folder).join("ic_launcher_round.png");
                assert!(sq.exists(), "Square icon should exist: {:?}", sq);
                assert!(rd.exists(), "Round icon should exist: {:?}", rd);
                let sq_img = image::open(&sq).unwrap();
                assert_eq!(sq_img.width(), *size);
                assert_eq!(sq_img.height(), *size);
            }
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_fallback_to_neutral_icons_when_no_exe() {
        let temp_dir = std::env::temp_dir().join(format!("rtp_no_exe_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let res_dir = temp_dir.join("res");
        let extracted = extract_and_generate_icons(&temp_dir, Some("WebGame"), &res_dir);
        assert!(!extracted, "Should return false when no exe is present");

        for (folder, _) in MIPMAP_TARGETS {
            let sq = res_dir.join(folder).join("ic_launcher.png");
            let rd = res_dir.join(folder).join("ic_launcher_round.png");
            assert!(sq.exists(), "Neutral square icon should exist: {:?}", sq);
            assert!(rd.exists(), "Neutral round icon should exist: {:?}", rd);
        }

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

