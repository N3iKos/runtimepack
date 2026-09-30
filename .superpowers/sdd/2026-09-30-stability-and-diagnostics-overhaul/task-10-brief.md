# Task 10 Brief: Probe CLI T1 — ProbeTarget trait + AdbTarget config-driven + scenario

**Plan:** `Docs/superpowers/plans/2026-09-30-stability-and-diagnostics-overhaul.md` (Task 10, versi revisi T1-only)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save` (branch `feature/native-mv-mz-save`, HEAD `97e9982`). Windows, PowerShell 5.1.

## Context
CLI RuntimePack adalah Rust binary (`cli/`, package `runtimepack`, subcommand `build` sudah ada). Anda menambah subcommand `probe` versi T1 (universal smoke test): install APK ke device ADB → boot → new game via D-pad → verifikasi masuk game → input test → error scan → laporan PASS/FAIL per langkah dengan kode RT-xxx.

PRINSIP WAJIB:
1. **Device TIDAK hardcode.** Implementasi tunggal `AdbTarget` generik (via `adb -s <address>`). Device didefinisikan di config file (`probe-profiles/devices.toml`) atau argumen CLI (`--device <id-atau-alamat>`). Tidak boleh ada string "mumu" di kode Rust kecuali di config default/example.
2. **TDD**: logika scenario diuji dengan `MockTarget` in-memory — tanpa device nyata. Semua verdict code harus teruji: `RT-BOOT-TIMEOUT`, `RT-NEWGAME-STALL`, `RT-CONSOLE-ERROR`, dan PASS path.
3. Ikuti struktur & gaya kode `cli/src/main.rs` yang ada (cek dulu: bagaimana subcommand di-parse — kemungkinan clap atau manual match; ikuti yang dipakai).

## Files
- Create: `cli/src/probe/mod.rs`, `cli/src/probe/target.rs` (trait + ProbeError + kode verdict), `cli/src/probe/adb.rs` (AdbTarget), `cli/src/probe/scenario.rs` (skenario T1 + laporan), `cli/src/probe/report.rs` (probe-report.json serializer) — pecah file sesuai tanggung jawab, jangan satu file raksasa.
- Create: `probe-profiles/devices.toml` (config example/default: entry `[devices.mumu] address = "127.0.0.1:16384"` — ini CONFIG, bukan kode).
- Create: `cli/tests/probe_scenario_test.rs` (atau unit test di modul — ikuti konvensi test repo yang ada) dengan `MockTarget`.
- Modify: `cli/src/main.rs` / `cli/Cargo.toml` (subcommand `probe` + deps `serde`/`toml` bila belum ada — cek workspace Cargo.toml dulu, pakai yang sudah ada).

## Interface

```rust
// target.rs
pub trait ProbeTarget {
    fn name(&self) -> &str;
    fn ensure_device(&mut self) -> Result<(), ProbeError>;
    fn install_apk(&mut self, apk: &Path) -> Result<String, ProbeError>; // -> package
    fn launch(&mut self, package: &str) -> Result<(), ProbeError>;
    fn tap(&mut self, x: u32, y: u32) -> Result<(), ProbeError>;
    fn key(&mut self, keycode: &str) -> Result<(), ProbeError>;
    fn screenshot(&mut self, out: &Path) -> Result<(), ProbeError>;
    fn screenshot_bytes(&mut self) -> Result<Vec<u8>, ProbeError>; // untuk diff tanpa disk (Mock + Adb: exec-out screencap -p)
    fn logcat_since(&mut self) -> Result<Vec<String>, ProbeError>;
    fn clear_logcat(&mut self) -> Result<(), ProbeError>;
    fn pid_alive(&mut self, package: &str) -> Result<bool, ProbeError>;
}
```

Kode verdict (domain RT):
- `RT-BOOT-TIMEOUT` — layar tidak render dalam 60s
- `RT-NEWGAME-STALL` — setelah D-pad+Enter, layar tidak berubah signifikan dalam 60s
- `RT-CONSOLE-ERROR` — error fatal di logcat (baris `[GameConsole]` ERROR level / `[RuntimePack]` error)
- `RT-PROCESS-CRASH` — pid game mati
- `RT-DEVICE-UNAVAILABLE` — device tidak konek
- PASS per langkah: `boot`, `new_game`, `touch_input`, `gamepad`, `stability`

## Skenario T1 (di scenario.rs, semua logika di sini — target hanya "tangan")
1. `ensure_device` → install → `clear_logcat` → launch
2. **boot**: poll tiap 2s (max 60s): `screenshot_bytes` 2x berbeda dari sebelumnya dan bukan hitam dominan (heuristik: >95% pixel identik hitam = belum render; implementasi pixel-diff sederhana pada raw bytes PNG — bandingkan ukuran + hash kasar; kalau PNG parsing terlalu berat, bandingkan ukuran byte + beberapa sampel offset, dan layar hitam dibuktikan dengan screencap kecil ~<15KB berulang). Verdict FAIL `RT-BOOT-TIMEOUT` bila timeout.
3. **title stabil**: 2 screenshot beruntun identik (interval 3s) → lanjut.
4. **new_game**: `key("KEYCODE_DPAD_DOWN")` × N (N default 0) + `key("KEYCODE_ENTER")` → poll 60s sampai screenshot berubah drastis (pixel-diff threshold longgar, mis. ukuran/perubahan hash cukup besar). FAIL `RT-NEWGAME-STALL` bila tidak berubah.
5. **touch_input**: `tap` tengah layar (butuh screen size — tambahkan `screen_size()` ke trait) → tunggu 3s → pid masih hidup.
6. **gamepad**: `key("KEYCODE_BUTTON_A")` → tunggu 3s → pid hidup.
7. **stability + error scan**: `logcat_since` → filter baris mengandung `[GameConsole]` dengan level ERROR / `[RuntimePack]` error / FATAL EXCEPTION → FAIL `RT-CONSOLE-ERROR` bila ada. `pid_alive` cek.
8. **report**: struct `ProbeReport { device, apk, package, steps: Vec<StepResult{step, status, code, detail_ms, note}>, overall }` → serialize JSON ke `dist/probe-report.json` + print ringkas ke stdout.

## CLI shape
```
runtimepack probe <APK> [--device <id|address>] [--dpad-down N] [--report <path>]
runtimepack probe --list-devices
```
- `--device` tidak diberikan → baca `probe-profiles/devices.toml`; kalau file tidak ada → scan `adb devices`; tepat 1 → pakai; 0 → error `RT-DEVICE-UNAVAILABLE` dengan pesan jelas; >1 → error minta eksplisit.
- `--list-devices` → tampilkan hasil `adb devices` + entry config.
- Config `devices.toml` format: `[devices.<id>] address = "host:port"` (toml crate bila sudah ada di workspace; kalau tidak, parser manual sederhana atau format alternatif yang sudah dipakai repo — ikuti konvensi).

## Steps (TDD)
- [ ] Step 1: Tulis `MockTarget` + test scenario dulu: PASS path lengkap (boot berubah → title stabil → new game berubah → semua langkah PASS), FAIL `RT-BOOT-TIMEOUT`, FAIL `RT-NEWGAME-STALL`, FAIL `RT-CONSOLE-ERROR`, FAIL `RT-PROCESS-CRASH`. Verifikasi `cargo test` → semua test BARU gagal (belum ada implementasi).
- [ ] Step 2: Implementasi trait + scenario + report + AdbTarget + CLI wiring + config.
- [ ] Step 3: `cargo test --workspace` hijau (semua, termasuk test lama) + `cargo build` sukses.
- [ ] Step 4: Smoke CLI tanpa device: `cargo run -q --bin runtimepack -- probe --list-devices` → tampil output walau error konek (jangan panic; pesan rapi).
- [ ] Step 5: Commit: `feat(cli): runtime probe subcommand (T1 universal) with config-driven AdbTarget`

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\native-mv-mz-save\.superpowers\sdd\2026-09-30-stability-and-diagnostics-overhaul\task-10-report.md`
isi: struktur modul, jumlah test baru + nama, hasil cargo test, smoke output list-devices, commit hash.

Reply ke saya HANYA: status, commit hash, jumlah test baru, hasil cargo test satu baris, concerns.
