# Capability Policy — Tahap 1 (Fondasi) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menyatukan penulisan engine/family, menggagalkan manifest rusak secara eksplisit, dan menghapus duplikasi daftar detektor — tanpa mengubah perilaku build yang sudah ada.

**Architecture:** Tiga perbaikan terpisah dan masing-masing dapat diuji sendiri. Task 1 memaku bentuk wire kanonik pada enum `EngineId`/`RuntimeFamily` lewat atribut serde eksplisit plus alias untuk ejaan lama. Task 2 mengubah `RuntimeRegistry::load_dir` agar mengembalikan error bertipe alih-alih menelan kegagalan parse. Task 3 memindahkan daftar detektor ke satu fungsi di `cli` (composition root), lalu memakai fungsi itu dari semua pemanggil CLI.

**Tech Stack:** Rust 2021, serde/serde_json/toml, crate `runtimepack-core`, `runtimepack` (CLI), `anyhow`/`thiserror` sesuai pola yang ada.

**Spec:** `Docs/superpowers/specs/2026-10-23-capability-compatibility-policy-design.md` (§4, §6, §9, §10, §12 Tahap 1)

## Global Constraints

- Tidak boleh melemahkan gate keras yang ada: integritas input, keamanan path, signing, toolchain, `audit_policy` hard-block.
- Tidak ada pengecualian berdasarkan judul game, hash, atau nama folder.
- Tidak menambah crate dependensi baru.
- Tidak mengubah perilaku build yang sudah lulus tes; setiap perubahan perilaku harus eksplisit di tes.
- Wajib TDD: tes ditulis lebih dulu, dijalankan, dilihat gagal, lalu implementasi minimal.
- Semua perintah build/test dijalankan di WSL Ubuntu-24.04 dengan `CARGO_TARGET_DIR=/home/mhsymr/linux-remediation-20261022/cargo-target`.
- Build APK wajib `--flavor beta` bila menyentuh build (tidak ada di plan ini).
- Format Rust mengikuti `cargo fmt`; jangan menambah `#[allow(...)]` baru.

## Review Focus

1. **Manifest dengan ejaan lama** (`ren-py`, `rpg-maker-xp`, `web-rpg-maker`, `tyrano-web`) harus tetap terbaca lewat alias, dan setelah round-trip ditulis dalam ejaan kanonik — bukan error.
2. **Manifest dengan nilai enum asing** harus gagal dengan pesan yang menyebut path berkas dan alasan, bukan dilewati diam-diam. (Penolakan field tak dikenal pada profil runtime adalah cakupan Tahap 3; jangan menambahkan `deny_unknown_fields` di sini karena akan menolak manifest lama yang sah.)
3. **Direktori manifest yang tidak ada** harus tetap `Ok(0)` (bukan error), karena runtime builtin tetap dipakai.
4. **Registry detektor tunggal**: `runtimepack audit` dan `runtimepack build` harus mendeteksi engine yang sama; menambah detektor di satu tempat tidak boleh meninggalkan yang lain tertinggal.
5. **`EngineId::from_str` untuk nilai asing** tetap menghasilkan `Err`, bukan `Unknown` implisit — dipaku oleh tes di Task 1 agar refactor serde tidak mengubah perilaku parsing CLI.

---

### Task 1: Ejaan wire kanonik untuk `EngineId` dan `RuntimeFamily`

**Files:**
- Modify: `core/src/domain.rs:4-16` (`EngineId`), `core/src/domain.rs:73-82` (`RuntimeFamily`)
- Test: `core/src/domain.rs` (modul `#[cfg(test)]` di akhir berkas; buat bila belum ada)

**Interfaces:**
- Consumes: tidak ada (task pertama).
- Produces: `EngineId` dan `RuntimeFamily` yang menyerialkan ejaan kanonik dan menerima alias lama.
  - Kanonik `EngineId`: `rpgmaker-xp`, `rpgmaker-vx`, `rpgmaker-vxace`, `rpgmaker-mv`, `rpgmaker-mz`, `renpy`, `tyrano`, `visual-novel-maker`, `unknown`.
  - Kanonik `RuntimeFamily`: `rgss`, `web`, `renpy`, `tyrano`, `vnmaker-web`.
  - Alias diterima (baca saja, tidak pernah ditulis): `rpg-maker-xp`, `rpg-maker-vx`, `rpg-maker-vx-ace`, `rpg-maker-vxace`, `rpg-maker-mv`, `rpg-maker-mz`, `ren-py`, `web-rpg-maker`, `tyrano-web`.

- [ ] **Step 1: Tulis tes yang gagal**

Tambahkan di `core/src/domain.rs`:

```rust
#[cfg(test)]
mod canonical_wire_tests {
    use super::{EngineId, RuntimeFamily};

    #[test]
    fn engine_id_serializes_canonical_spelling() {
        let cases = [
            (EngineId::RpgMakerXp, "\"rpgmaker-xp\""),
            (EngineId::RpgMakerVx, "\"rpgmaker-vx\""),
            (EngineId::RpgMakerVxAce, "\"rpgmaker-vxace\""),
            (EngineId::RpgMakerMv, "\"rpgmaker-mv\""),
            (EngineId::RpgMakerMz, "\"rpgmaker-mz\""),
            (EngineId::RenPy, "\"renpy\""),
            (EngineId::Tyrano, "\"tyrano\""),
            (EngineId::VisualNovelMaker, "\"visual-novel-maker\""),
            (EngineId::Unknown, "\"unknown\""),
        ];
        for (value, expected) in cases {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
        }
    }

    #[test]
    fn engine_id_accepts_legacy_kebab_alias() {
        let cases = [
            ("\"rpg-maker-xp\"", EngineId::RpgMakerXp),
            ("\"rpg-maker-vx\"", EngineId::RpgMakerVx),
            ("\"rpg-maker-vx-ace\"", EngineId::RpgMakerVxAce),
            ("\"rpg-maker-mv\"", EngineId::RpgMakerMv),
            ("\"rpg-maker-mz\"", EngineId::RpgMakerMz),
            ("\"ren-py\"", EngineId::RenPy),
        ];
        for (wire, expected) in cases {
            let parsed: EngineId = serde_json::from_str(wire).unwrap();
            assert_eq!(parsed, expected, "failed to read alias {wire}");
        }
    }

    #[test]
    fn runtime_family_serializes_canonical_spelling() {
        let cases = [
            (RuntimeFamily::Rgss, "\"rgss\""),
            (RuntimeFamily::WebRpgMaker, "\"web\""),
            (RuntimeFamily::RenPy, "\"renpy\""),
            (RuntimeFamily::TyranoWeb, "\"tyrano\""),
            (RuntimeFamily::VnMakerWeb, "\"vnmaker-web\""),
        ];
        for (value, expected) in cases {
            assert_eq!(serde_json::to_string(&value).unwrap(), expected);
        }
    }

    #[test]
    fn runtime_family_accepts_legacy_kebab_alias() {
        let cases = [
            ("\"web-rpg-maker\"", RuntimeFamily::WebRpgMaker),
            ("\"ren-py\"", RuntimeFamily::RenPy),
            ("\"tyrano-web\"", RuntimeFamily::TyranoWeb),
        ];
        for (wire, expected) in cases {
            let parsed: RuntimeFamily = serde_json::from_str(wire).unwrap();
            assert_eq!(parsed, expected, "failed to read alias {wire}");
        }
    }
    #[test]
    fn engine_id_from_str_still_rejects_unknown_values() {
        use std::str::FromStr;
        assert!(EngineId::from_str("not-an-engine").is_err());
        assert_eq!(EngineId::from_str("renpy").unwrap(), EngineId::RenPy);
        assert_eq!(EngineId::from_str("ren-py").unwrap(), EngineId::RenPy);
    }
}
```

- [ ] **Step 2: Jalankan tes, pastikan gagal**

Run:
```bash
cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/runtime-v2
CARGO_TARGET_DIR=/home/mhsymr/linux-remediation-20261022/cargo-target \
  cargo test -p runtimepack-core canonical_wire_tests
```
Expected: FAIL — serialisasi menghasilkan `"rpg-maker-xp"` dan `"ren-py"`, bukan ejaan kanonik.

- [ ] **Step 3: Implementasi atribut serde eksplisit**

Pada `EngineId`, ganti `#[serde(rename_all = "kebab-case")]` dengan rename per-varian:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EngineId {
    #[serde(rename = "rpgmaker-xp", alias = "rpg-maker-xp")]
    RpgMakerXp,
    #[serde(rename = "rpgmaker-vx", alias = "rpg-maker-vx")]
    RpgMakerVx,
    #[serde(rename = "rpgmaker-vxace", alias = "rpg-maker-vx-ace", alias = "rpg-maker-vxace")]
    RpgMakerVxAce,
    #[serde(rename = "rpgmaker-mv", alias = "rpg-maker-mv")]
    RpgMakerMv,
    #[serde(rename = "rpgmaker-mz", alias = "rpg-maker-mz")]
    RpgMakerMz,
    #[serde(rename = "renpy", alias = "ren-py")]
    RenPy,
    #[serde(rename = "tyrano")]
    Tyrano,
    #[serde(rename = "visual-novel-maker")]
    VisualNovelMaker,
    #[serde(rename = "unknown")]
    Unknown,
}
```

Pada `RuntimeFamily`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuntimeFamily {
    #[serde(rename = "rgss")]
    Rgss,
    #[serde(rename = "web", alias = "web-rpg-maker")]
    WebRpgMaker,
    #[serde(rename = "renpy", alias = "ren-py")]
    RenPy,
    #[serde(rename = "tyrano", alias = "tyrano-web")]
    TyranoWeb,
    #[serde(rename = "vnmaker-web")]
    VnMakerWeb,
}
```

Jangan mengubah `Display` maupun `FromStr`.

- [ ] **Step 4: Jalankan tes, pastikan lulus**

Run:
```bash
cargo test -p runtimepack-core canonical_wire_tests
```
Expected: PASS — 4 tes lulus.

- [ ] **Step 5: Jalankan seluruh tes core untuk regresi**

Run:
```bash
cargo test -p runtimepack-core
```
Expected: PASS. Bila ada tes lama yang mengunci ejaan kebab, perbarui **hanya** ekspektasi wire-nya dan catat di laporan; jangan mengubah logika.

- [ ] **Step 6: Commit**

```bash
git add core/src/domain.rs
git commit -m "refactor(core): pin canonical engine/family wire spelling with legacy aliases"
```

---

### Task 2: Manifest runtime rusak harus gagal eksplisit

**Files:**
- Modify: `core/src/runtime_registry/mod.rs:139-160` (`load_dir`)
- Modify: `core/src/runtime_registry/mod.rs:82-89` (tambah konstruktor eksplisit yang memuat direktori)
- Test: `core/src/runtime_registry/mod.rs` (modul `#[cfg(test)]`)

**Interfaces:**
- Consumes: ejaan kanonik dari Task 1.
- Produces:
  - `pub fn load_dir(&mut self, dir: &Path) -> Result<usize, RuntimeManifestError>`
  - `pub struct RuntimeManifestError { pub path: PathBuf, pub reason: String }` dengan `impl std::fmt::Display` dan `impl std::error::Error`.
  - `pub fn load_dir_with_builtins(dir: &Path) -> Result<RuntimeRegistry, RuntimeManifestError>` — membuat registry builtin lalu memuat direktori.

- [ ] **Step 1: Tulis tes yang gagal**

Tambahkan di `core/src/runtime_registry/mod.rs`:

```rust
#[cfg(test)]
mod manifest_loading_tests {
    use super::RuntimeRegistry;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rp-manifest-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn malformed_manifest_is_an_error_not_a_silent_skip() {
        let dir = scratch("malformed");
        fs::write(dir.join("bad.toml"), "this is not toml {{{").unwrap();
        let mut registry = RuntimeRegistry::new();
        let err = registry.load_dir(&dir).unwrap_err();
        assert!(
            err.path.ends_with("bad.toml"),
            "error must name the offending file: {err}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_enum_value_is_an_error() {
        let dir = scratch("unknown-enum");
        fs::write(
            dir.join("bad-family.toml"),
            r#"
manifest_version = 1
id = "x"
version = "1.0.0"
runtime_api = 1
family = "not-a-family"
engines = ["renpy"]
abis = ["arm64-v8a"]
capabilities = []
[android]
min_sdk = 26
"#,
        )
        .unwrap();
        let mut registry = RuntimeRegistry::new();
        let err = registry.load_dir(&dir).unwrap_err();
        assert!(
            err.reason.contains("not-a-family") || err.reason.contains("unknown variant"),
            "error must explain the bad value: {}",
            err.reason
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_directory_is_not_an_error() {
        let dir = std::env::temp_dir().join("rp-manifest-test-does-not-exist-xyz");
        let _ = fs::remove_dir_all(&dir);
        let mut registry = RuntimeRegistry::new();
        assert_eq!(registry.load_dir(&dir).unwrap(), 0);
    }

    #[test]
    fn all_shipped_manifests_parse() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("runtimes")
            .join("manifests");
        let mut registry = RuntimeRegistry::new();
        let count = registry.load_dir(&manifest_dir).unwrap();
        assert_eq!(count, 6, "all six shipped manifests must parse");
        let ids: Vec<&str> = registry.all().iter().map(|m| m.id.as_str()).collect();
        for expected in [
            "renpy-android-7",
            "renpy-android-8",
            "rgss-android",
            "web-rpgmaker",
            "vnmaker-web",
            "tyrano-web",
        ] {
            assert!(ids.contains(&expected), "missing {expected} in {ids:?}");
        }
    }
}
```

- [ ] **Step 2: Jalankan tes, pastikan gagal**

Run:
```bash
cargo test -p runtimepack-core manifest_loading_tests
```
Expected: FAIL — `load_dir` mengembalikan `io::Result` dan menelan kegagalan parse, sehingga tes pertama dan `all_shipped_manifests_parse` gagal.

- [ ] **Step 3: Implementasi error bertipe**

Ganti signature dan badan `load_dir`:

```rust
#[derive(Debug)]
pub struct RuntimeManifestError {
    pub path: std::path::PathBuf,
    pub reason: String,
}

impl std::fmt::Display for RuntimeManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid runtime manifest '{}': {}", self.path.display(), self.reason)
    }
}

impl std::error::Error for RuntimeManifestError {}
```

Perilaku `load_dir`:

- direktori tidak ada atau bukan direktori → `Ok(0)`;
- berkas `.toml` yang gagal dibaca → `Err(RuntimeManifestError { path, reason })`;
- berkas `.toml` yang gagal di-parse → `Err(RuntimeManifestError { path, reason: e.to_string() })`;
- selain `.toml` → dilewati;
- sukses → `Ok(count)`.

Tambahkan konstruktor:

```rust
pub fn load_dir_with_builtins(dir: &Path) -> Result<RuntimeRegistry, RuntimeManifestError> {
    let mut registry = RuntimeRegistry::new();
    registry.load_dir(dir)?;
    Ok(registry)
}
```

- [ ] **Step 4: Perbarui pemanggil agar tetap kompilasi**

Cari seluruh pemanggil `load_dir`:

```bash
grep -rn "load_dir" core/ cli/ tests/
```

Untuk setiap pemanggil, teruskan error ke jalur error yang sudah ada (`?` bila fungsi mengembalikan `Result`, atau `.expect(...)` hanya di kode tes). Jangan menambah `unwrap_or_default()` baru.

- [ ] **Step 5: Jalankan tes, pastikan lulus**

Run:
```bash
cargo test -p runtimepack-core manifest_loading_tests
cargo test -p runtimepack-core
```
Expected: PASS, termasuk `all_shipped_manifests_parse` dengan 6 manifest.

- [ ] **Step 6: Commit**

```bash
git add core/src/runtime_registry/mod.rs
git commit -m "fix(core): fail on invalid runtime manifests instead of silently skipping"
```

---

### Task 3: Satu daftar detektor untuk `build` dan `audit`

**Files:**
- Create: `cli/src/detection_registry.rs`
- Modify: `cli/src/lib.rs` (tambah `pub mod detection_registry;`)
- Modify: `core/src/detection/mod.rs` (tambah accessor baca-saja `len()`, `is_empty()`, `names()`)
- Modify: `cli/src/main.rs:265-275`
- Modify: `cli/src/audit_command.rs:320-331`
- Test: `cli/src/detection_registry.rs` (modul `#[cfg(test)]`)

**Interfaces:**
- Consumes: `DetectorRegistry` yang sudah ada di `core/src/detection/mod.rs`.
- Produces:
  - `pub fn default_detector_registry() -> DetectorRegistry` di `runtimepack_cli::detection_registry`.
  - Menjamin urutan registrasi identik dengan daftar saat ini: XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano, VN Maker.

**Mengapa `cli`, bukan `core`:** struct detektor tinggal di crate `engines/*` (`engines/rgss`, `engines/rpgmaker_web`, `engines/renpy`, `engines/tyrano`, `engines/vnmaker`), dan setiap crate engine bergantung pada `core`. Menempatkan fungsi perakit ini di `core` akan membuat siklus dependensi `core -> engines/* -> core` yang ditolak Cargo. `cli` adalah composition root: satu-satunya crate yang sudah menautkan kelima crate engine, dan satu-satunya tempat `AnalyzerRegistry` serta `AssetAuditorRegistry` dirakit. Rumah jangka panjang yang benar adalah crate assembly khusus (mis. `engines/registry`); itu di luar scope Tahap 1.

- [ ] **Step 1: Tulis tes yang gagal**

Buat `cli/src/detection_registry.rs` berisi hanya tes:

```rust
#[cfg(test)]
mod default_registry_tests {
    use super::default_detector_registry;

    #[test]
    fn registry_contains_every_supported_engine_once() {
        let registry = default_detector_registry();
        assert_eq!(
            registry.len(),
            8,
            "expected exactly the eight v1 detectors"
        );
    }

    #[test]
    fn registry_is_deterministic_across_calls() {
        let a: Vec<String> = default_detector_registry()
            .names()
            .into_iter()
            .map(str::to_string)
            .collect();
        let b: Vec<String> = default_detector_registry()
            .names()
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(a, b, "registration order must be stable");
    }
}
```

Bila `DetectorRegistry` belum punya `len()` atau `names()`, tambahkan sebagai metode inheren kecil di `core/src/detection/mod.rs` (hanya pembacaan, tanpa mengubah perilaku analisis).

Daftarkan modul baru di `cli/src/lib.rs` (`pub mod detection_registry;`) agar tes lib dapat menjangkaunya.

- [ ] **Step 2: Jalankan tes, pastikan gagal**

Run:
```bash
cargo test -p runtimepack --lib default_registry_tests
```
Expected: FAIL — `default_detector_registry` belum ada (`cannot find function`).

Catatan: fungsi ini tinggal di crate `runtimepack` (CLI), bukan `runtimepack-core`. Perintah `cargo test -p runtimepack-core default_registry_tests` akan melaporkan `0 passed; ... filtered out` dengan exit 0 — gate hampa tanpa satu assertion pun, jadi jangan dipakai.

- [ ] **Step 3: Implementasi fungsi tunggal**

Isi `cli/src/detection_registry.rs`:

```rust
//! Single source of truth for the v1 detector set.

use runtimepack_core::detection::DetectorRegistry;
use runtimepack_engine_renpy::RenPyDetector;
use runtimepack_engine_rgss::{RpgMakerVxAceDetector, RpgMakerVxDetector, RpgMakerXpDetector};
use runtimepack_engine_rpgmaker_web::{RpgMakerMvDetector, RpgMakerMzDetector};
use runtimepack_engine_tyrano::TyranoDetector;
use runtimepack_engine_vnmaker::VnMakerDetector;

/// The complete v1 detector set, in deterministic registration order.
pub fn default_detector_registry() -> DetectorRegistry {
    let mut registry = DetectorRegistry::new();
    registry.register(Box::new(RpgMakerXpDetector));
    registry.register(Box::new(RpgMakerVxDetector));
    registry.register(Box::new(RpgMakerVxAceDetector));
    registry.register(Box::new(RpgMakerMvDetector));
    registry.register(Box::new(RpgMakerMzDetector));
    registry.register(Box::new(RenPyDetector));
    registry.register(Box::new(TyranoDetector));
    registry.register(Box::new(VnMakerDetector));
    registry
}
```

Nama crate dan modul detektor di atas adalah yang sebenarnya dipakai (`runtimepack_engine_*`); jangan memindahkan modul detektor ke `core`.

- [ ] **Step 4: Ganti kedua pemanggil**

Di `cli/src/main.rs`, ganti badan fungsi di sekitar baris 265 agar memanggil `runtimepack_cli::detection_registry::default_detector_registry()`.

Di `cli/src/audit_command.rs`, hapus `fn create_detector_registry()` beserta isinya dan ganti setiap pemakaiannya dengan `runtimepack_cli::detection_registry::default_detector_registry()`.

Hapus `use` detektor yang menjadi tidak terpakai di kedua berkas.

- [ ] **Step 5: Jalankan tes, pastikan lulus**

Run:
```bash
cargo test -p runtimepack --lib default_registry_tests
cargo build -p runtimepack
```
Expected: PASS (3 tes) dan build sukses tanpa warning `unused import`.

Catatan gate: jangan memakai `cargo test -p runtimepack-core default_registry_tests`. Fungsi ini tidak ada di `runtimepack-core`, sehingga perintah itu hanya menyaring seluruh tes dan keluar dengan `0 passed; ... filtered out` + exit 0 — hijau palsu tanpa assertion.

- [ ] **Step 6: Verifikasi paritas deteksi**

Cakupan Step 6 kini **permanen** lewat `tests/integration/detector_parity_test.rs` (target `detector_parity_test` di `cli/Cargo.toml`). Tes itu menjalankan binary dua kali (`inspect` dan `audit --no-html -o <tmp>`) pada satu fixture web (`tests/fixtures/mv_min`) dan satu non-web (`tests/fixtures/renpy_v8_min`), lalu meng-assert engine yang dilaporkan identik.

Run:
```bash
cargo test -p runtimepack --test detector_parity_test
```
Expected: PASS, 2 tes. Gate ini gagal bila seseorang menambahkan daftar detektor kedua dengan urutan atau isi berbeda di `main.rs` atau `audit_command.rs`.

Pemeriksaan manual opsional (bukan gate) pada satu game MV dan satu game Ren'Py:

```bash
runtimepack inspect <game-mv>
runtimepack audit <game-mv>
runtimepack inspect <game-renpy>
runtimepack audit <game-renpy>
```
Expected: engine yang terdeteksi identik antara `inspect` dan `audit` untuk masing-masing game.

- [ ] **Step 7: Commit**

```bash
git add cli/src/detection_registry.rs cli/src/lib.rs cli/src/main.rs cli/src/audit_command.rs core/src/detection/mod.rs tests/integration/detector_parity_test.rs cli/Cargo.toml
git commit -m "refactor(cli): single detector registry shared by build and audit"
```

---

### Task 4: Regresi menyeluruh Tahap 1

**Files:**
- Test: seluruh workspace (tidak ada berkas baru)

**Interfaces:**
- Consumes: Task 1–3.
- Produces: bukti bahwa Tahap 1 tidak mengubah perilaku build yang sudah ada.

- [ ] **Step 1: Jalankan seluruh tes workspace**

Run:
```bash
cd /mnt/d/02_Development/Projects_1/runtimepack-vnm-reconstruction/runtime-v2
CARGO_TARGET_DIR=/home/mhsymr/linux-remediation-20261022/cargo-target \
CARGO_BUILD_JOBS=2 \
  cargo test --workspace 2>&1 | tee /mnt/d/02_Development/Raw-Games/Sample/port_apk/_reports/linux-remediation-20261022/TAHAP1-WORKSPACE.log
```
Expected: seluruh tes lulus, **kecuali** dua kegagalan yang sudah diketahui sebelum plan ini (`test_build_arbitrary_nested_payload_gpack_contains_root_entrypoint_and_excludes_outer_runners` dan `test_cli_build_with_compat_profile_and_staging`). Kegagalan itu milik Task berikutnya (Tahap 2), bukan regresi Tahap 1.

- [ ] **Step 2: Bandingkan dengan baseline**

Bandingkan daftar kegagalan dengan `RESUME-BASELINE-WORKSPACE-442335c.log`. Tidak boleh ada kegagalan **baru**.

- [ ] **Step 3: Catat hasil**

Tulis ringkasan ke `_reports/linux-remediation-20261022/TAHAP1-RESULT.md`: jumlah tes lulus/gagal, daftar kegagalan yang tersisa, dan bukti bahwa tidak ada kegagalan baru.

---

## Catatan penutup

Plan ini **hanya** Tahap 1. Tahap 2 (satu titik keputusan) dan Tahap 3 (Opsi C pada RPG Maker Web) memerlukan plan terpisah, karena keduanya mengubah semantik kebijakan dan harus disetujui sendiri.

Dua kegagalan tes yang sudah ada (`build_test.rs`) sengaja **tidak** diperbaiki di sini: keduanya berasal dari perbedaan pandangan scanner vs auditor, yang merupakan isi Tahap 2.
