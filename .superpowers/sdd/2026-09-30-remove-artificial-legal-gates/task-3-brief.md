# Task 3 Brief: Bersihkan AGENTS.md, Docs/, dan memory dari gate legalitas buatan

**Plan:** `Docs/superpowers/plans/2026-09-30-remove-artificial-legal-gates.md` (Task 3)

**Workspace:** worktree `D:\02_Development\Projects_2\RPGM-P\.worktrees\remove-legal-gates` (branch `feature/remove-legal-gates`, HEAD `5fa856e`). Windows, PowerShell 5.1.

## Context
RuntimePack = tool open-source komunitas non-komersial. Seluruh kode gate sudah dihapus (Task 1+2). Sekarang bersihkan dokumentasi & governance agar konsisten: TyranoScript = engine first-class, tanpa `TYRANO-LEGAL`, `TYR-101`, `LIC-310`, `LIC-101`, `SPIKE-TYRANO-LEGAL-001`, `PROPRIETARY-GATED`.

## BATASAN KETAT (jangan sampai merusak kontrak user)
- **AGENTS.md bagian atas** (Peran Supervisor, Workflow Wajib, Aturan Wajib §1-§6, Memory Protocol) adalah KONTRAK USER — **JANGAN DIUBAH**. Yang boleh dibersihkan HANYA bagian spec teknis lama di bawahnya (§5 item Tyrano, §7.7, §9 BLOCKED listing, §12, §19, §20) yang menyebut gate legal Tyrano.
- Hapus/ubah HANYA konten terkait gate legal artifisial. Jangan rewrite dokumen lain.

## Files (dari audit explorer — verifikasi nomor baris saat edit karena posisi bisa bergeser)
- `AGENTS.md` — §5 item 5 ("invent Tyrano redistribution permissions") dan item 12 terkait; §7.7 (seluruh pasal gate); §9 (BLOCKED listing — pertahankan status enum tapi hapus kaitan Tyrano bila ada); §12 (special case Tyrano); §19 (`LICENSE_BLOCKED` stop condition); §20 (Phase 8 "subject to legal gate").
- `Docs/LICENSE_COMPLIANCE.md` — line 3 (release blocker sentence), §5 seluruh bab TYRANO-LEGAL (81-100), line 159 (bypass item).
- `Docs/OPEN_QUESTIONS_AND_SPIKES.md` — hapus `SPIKE-TYRANO-LEGAL-001` (75-85).
- `Docs/TECHNICAL_SPEC_V1.md` — `TECH-TYRANO-004 LEGAL GATE` (394-408) → hapus atau ubah jadi catatan singkat bahwa Tyrano didukung penuh via Web runtime.
- `Docs/PRD.md` — line 33, PRD-018 (193), release gate (227) → Tyrano didukung normal.
- `Docs/ENGINE_SUPPORT_MATRIX.md` — line 13 status jadi `Supported`; 111-114 hapus catatan gate.
- `Docs/COMPATIBILITY.md` — 137, 183, 238 → hapus kaitan TYR-101 sebagai hard blocker.
- `Docs/CONFIG_AND_SCHEMAS.md` — 103 → hapus TYR-101 dari hard blockers.
- `Docs/IMPLEMENTATION_PLAN.md` — 56, 165 → hapus TYR-101 + enable packaging.
- `Docs/TEST_PLAN.md` — 154, 201, 243 → hapus asersi gate.
- `Docs/REFERENCES.md` — 111 → hapus kalimat "**This source creates TYRANO-LEGAL gate.**" (biarkan sisa deskripsi S10 netral).
- `Docs/AGENT_GUIDE.md` — 40 (Rule 10), 130, 142 → hapus.
- `Docs/README.md` — 54, 69 → hapus notasi gated.
- `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md` — 123 → hapus catatan gate.
- `Docs/RuntimePack_MASTER_SPEC_v1.md` — seluruh kemunculan (lines 157, 259, 309, 469, 503, 911-923, 1374, 1475, 2876, 2923, 2988, 3022, 3206, 3446-3456, 3511, 3631) → sinkronkan dengan perubahan di atas.
- `Docs/AGENTS-old.md` — file arsip historis; HAPUS SAJA FILE INI (arsip lama, bukan dokumen aktif — menambah kebingungan).
- `Docs/superpowers/plans/` & `Docs/superpowers/specs/` — dokumen plan/sejarah SDD lama: **BIARKAN APA ADANYA** (artefak sejarah eksekusi, bukan kontrak aktif).
- `memory/context.md` — 30, 35 → hapus entry TYR-101/LIC-xxx gate; pertahankan sisanya.

## Steps
- [ ] **Step 1: Edit semua file di atas** (baca tiap file dulu; ubah minimal, jangan rewrite).
- [ ] **Step 2: Grep verifikasi (harus tersisa 0 di file aktif):**
```powershell
Get-ChildItem -Path AGENTS.md, Docs\*.md, memory\*.md -File | Select-String -Pattern "TYRANO-LEGAL|TYR-101|LIC-310|SPIKE-TYRANO-LEGAL|PROPRIETARY-GATED"
```
Expected: 0 matches.
- [ ] **Step 3: Verifikasi tidak ada test/doc build yang rusak:**
```powershell
cargo test --workspace
```
Expected: 364 passed, 0 failed.
- [ ] **Step 4: Commit:**
```bash
git add AGENTS.md Docs/ memory/
git commit -m "docs: remove artificial legal gates and establish full first-class support for TyranoScript"
```

## Report contract
Tulis report lengkap ke:
`D:\02_Development\Projects_2\RPGM-P\.worktrees\remove-legal-gates\.superpowers\sdd\2026-09-30-remove-artificial-legal-gates\task-3-report.md`
isi: daftar file diubah, keputusan per titik ambigu, hasil grep Step 2, commit hash.

Reply ke supervisor HANYA: status, commit hash, sisa grep (harus 0), concerns.
