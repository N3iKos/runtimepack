# Task 1+2 Brief: Hapus gate legalitas artifisial dari Rust core, CLI, engine Tyrano, dan tests

**Plan:** `Docs/superpowers/plans/2026-09-30-remove-artificial-legal-gates.md` (Task 1 + Task 2 digabung — satu unit kerja agar workspace selalu hijau per commit)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\remove-legal-gates` (branch `feature/remove-legal-gates`, BASE `8d0f4a6`). Windows, PowerShell 5.1.

## Context
RuntimePack adalah tool open-source komunitas non-komersial. AI sebelumnya memasang gerbang legalitas artifisial (TYRANO-LEGAL, TYR-101, LIC-101, PROPRIETARY-GATED) yang memperlakukan TyranoScript sebagai engine kelas dua. Audit explorer sudah memetakan semua lokasinya. Anda menghapusnya semua dan membuat Tyrano first-class citizen.

**RULING PENTING:**
- R1: `CompatibilityStatus::Blocked` dan enum `"BLOCKED"` di `schemas/compatibility-report.schema.json` **DIPERTAHANKAN** sebagai status kompatibilitas teknis umum (bisa dipakai untuk inkompatibilitas nyata di masa depan). Yang dihapus HANYA penggunaannya untuk Tyrano.
- R2: `RemediationCategory::LegalGate` di `diagnostics/mod.rs` — hapus HANYA jika tidak dipakai di tempat lain yang bukan gate artifisial (grep dulu semua penggunaannya; kalau dipakai untuk remediasi lisensi pihak ketiga yang genuine, cukup hapus konstanta TYR/LIC-nya saja).

## Files
- Modify: `core/src/compatibility/mod.rs:38-41` — hapus cabang `else if descriptor.id == EngineId::Tyrano { CompatibilityStatus::Blocked }` (Tyrano dievaluasi normal oleh analyzer seperti engine lain).
- Modify: `core/src/diagnostics/mod.rs:31,81,96` — hapus `pub const TYR_LEGAL_GATE_BLOCKED = "TYR-101"` dan `pub const LIC_REDISTRIBUTION_BLOCKED = "LIC-101"` (ikuti R2 untuk `LegalGate` variant; hapus juga jika sudah tak ada pemakai).
- Modify: `core/src/runtime_registry/mod.rs:296,300,323` — nama jadi `"RuntimePack Tyrano Web Adapter"`, `license_id: Some("MIT")`.
- Modify: `core/src/provenance/mod.rs:110` — hapus klaim `"Redistribution restrictions apply; creates TYRANO-LEGAL gate"` (ganti dengan deskripsi netral S10 TyranoScript repo, mis. `"TyranoScript official repository (HTML/JavaScript engine)."`).
- Modify: `core/src/build_plan/audit_policy.rs:34` — hapus filter `TYR_LEGAL_GATE_BLOCKED`.
- Modify: `runtimes/manifests/tyrano-web.toml:3,27` — `name = "RuntimePack Tyrano Web Adapter"`, `license_id = "MIT"`.
- Modify: `cli/src/main.rs:771-775` — hapus blok notice TYRANO-LEGAL saat build Tyrano.
- Modify: `engines/tyrano/src/scanner.rs:145-147` — hapus kompensasi `if report.status == Blocked { report.status = Compatible; }`.
- Modify: `engines/tyrano/src/asset_audit.rs:77-79,545-546,699-706` — hapus injeksi finding TYR-101, pemetaan `make_finding` untuk kode itu, dan konstanta `TYR_LEGAL_REDISTRIBUTION_BLOCKED`.
- Modify: `tests/integration/build_test.rs:163-179` — hapus/rewrite test `test_audit_policy_tyrano_legal_gate_not_hard_blocker` (kalau test itu menguji override policy secara umum, rewrite tanpa TYR-101; kalau murni tentang TYR-101, hapus).
- Modify: `tests/integration/tyrano_asset_audit_test.rs:146-155,681-684` — hapus assertion bahwa `TYR_LEGAL_GATE_BLOCKED` muncul di findings (test lain di suite tetap utuh).

## Steps
- [ ] **Step 1: Verifikasi pemakaai `LegalGate` & TYR/LIC constants** — grep sebelum menghapus (aturan R2):
```powershell
Select-String -Path core\src\*.rs, engines\tyrano\src\*.rs, cli\src\*.rs -Pattern "LegalGate|TYR_LEGAL|LIC_REDISTRIBUTION" | Select-Object Path, LineNumber, Line
```
- [ ] **Step 2: Terapkan semua penghapusan di atas** (baca tiap file dulu; jangan hapus apapun di luar daftar).
- [ ] **Step 3: Grep verifikasi nol sisa di kode Rust:**
```powershell
Select-String -Path core\src\*.rs, engines\tyrano\src\*.rs, cli\src\*.rs, runtimes\manifests\*.toml -Pattern "TYRANO-LEGAL|TYR-101|LIC-101|PROPRIETARY-GATED" -Recurse
```
Expected: 0 matches.
- [ ] **Step 4: Jalankan seluruh workspace test:**
```powershell
cargo test --workspace
```
Expected: 100% PASS (jumlah test mungkin turun karena test gate dihapus — itu normal; tidak boleh ada FAIL).
- [ ] **Step 5: Commit (boleh 2 commit: core lalu cli/engine/tests):**
```bash
git add core/ runtimes/
git commit -m "refactor(core): remove artificial TYRANO-LEGAL status and proprietary gate from runtime registry"
git add cli/ engines/ tests/
git commit -m "fix(tyrano): remove artificial TYR-101 audit injection and CLI warning notice"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\remove-legal-gates\.superpowers\sdd\2026-09-30-remove-artificial-legal-gates\task-1-2-report.md`
isi: keputusan R2 (LegalGate dipertahankan/dihapus + alasan), hasil grep Step 3, jumlah test sebelum/sesudah, commit hash.

Reply ke supervisor HANYA: status, commit hash(es), test summary satu baris, sisa grep (harus 0), concerns.
