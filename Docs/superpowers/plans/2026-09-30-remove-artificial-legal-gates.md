# Plan: Remove Artificial Legal Gates & Unblock All Engines

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Menghapus seluruh batasan legalitas artifisial, kode blocking fiktif (`TYRANO-LEGAL`, `TYR-101`, `LIC-101`), dan peringatan lisensi paranoid yang ditambahkan oleh AI sebelumnya, sehingga TyranoScript dan semua engine yang didukung RuntimePack menjadi 100% first-class citizen tanpa hambatan birokrasi buatan di tool open-source komunitas.

**Architecture:** Tiga fase pembersihan berurutan: (1) Core Runtime & Compatibility resolution di Rust, (2) Engine Tyrano, CLI output & unit test suite, (3) Governance, master specification, dan dokumentasi arsitektur.

**Tech Stack:** Rust (cargo workspace: `core`, `cli`, `engines/tyrano`, `runtimes`, `tests`), Markdown documentation.

**Spec:** Hasil audit sub-agent explorer 2026-09-30 (laporan audit repositori `task-audit-legal-gates`).

---

## Global Constraints

- **Input game tetap 100% immutable** — tidak ada aset game di `Raw-Game/` yang disentuh.
- **Zero test regressions** — seluruh test suite Rust (`cargo test --workspace`) dan unit test Android/JS harus lulus 100% setelah penghapusan gate.
- **Engine equality** — seluruh engine (RPG Maker XP/VX/VX Ace/MV/MZ, Ren'Py, TyranoBuilder/TyranoScript) diperlakukan setara sebagai runtime resmi RuntimePack tanpa status `BLOCKED`, `PROPRIETARY-GATED`, atau peringatan distributif buatan.
- **Open-source community focus** — dokumentasi dan pedoman sub-agent diarahkan untuk utilitas komunitas dan pelestarian game non-komersial.

---

## File Structure

| File | Aksi | Tanggung Jawab |
| :--- | :--- | :--- |
| `core/src/compatibility/mod.rs` | Modify | Hapus hardcoded `CompatibilityStatus::Blocked` pada Tyrano |
| `core/src/diagnostics/mod.rs` | Modify | Hapus `LegalGate` enum variant, `TYR-101`, dan `LIC-101` |
| `core/src/runtime_registry/mod.rs` | Modify | Hapus string `(Subject to TYRANO-LEGAL gate)` dan ganti `PROPRIETARY-GATED` dengan `MIT` |
| `core/src/provenance/mod.rs` | Modify | Hapus klaim pembatasan legalitas S10 |
| `core/src/build_plan/audit_policy.rs` | Modify | Hapus filter sisa `TYR_LEGAL_GATE_BLOCKED` |
| `runtimes/manifests/tyrano-web.toml` | Modify | Normalisasi nama dan license_id menjadi standar open source |
| `cli/src/main.rs` | Modify | Hapus notice warning pembatasan distribusi Tyrano di stdout |
| `engines/tyrano/src/scanner.rs` | Modify | Hapus logika unblock kompensasi yang sebelumnya dipicu oleh core |
| `engines/tyrano/src/asset_audit.rs` | Modify | Hapus injeksi otomatis finding `TYR-101` dari audit aset |
| `tests/integration/build_test.rs` | Modify | Hapus/update assertion test yang sebelumnya menguji keberadaan `TYR-101` |
| `tests/integration/tyrano_asset_audit_test.rs` | Modify | Hapus assertion test bahwa `TYR-101` harus muncul di audit Tyrano |
| `AGENTS.md` | Modify | Bersihkan aturan paranoia (§5, §7.7, §12, §19, §20) agar sub-agent memperlakukan Tyrano secara normal |
| `Docs/` & `memory/` | Modify | Bersihkan seluruh dokumen referensi gate legalitas buatan |

---

### Task 1: Clean Core Runtime, Diagnostics, & Compatibility in Rust

**Files:**
- Modify: `core/src/compatibility/mod.rs:38-41`
- Modify: `core/src/diagnostics/mod.rs:31,81,96`
- Modify: `core/src/runtime_registry/mod.rs:296,300,323`
- Modify: `core/src/provenance/mod.rs:110`
- Modify: `core/src/build_plan/audit_policy.rs:34`
- Modify: `runtimes/manifests/tyrano-web.toml:3,27`

- [ ] **Step 1: Edit `core/src/compatibility/mod.rs`**
Hapus cabang:
```rust
} else if descriptor.id == EngineId::Tyrano {
    // Tyrano is gated by TYRANO-LEGAL
    CompatibilityStatus::Blocked
}
```
Sehingga Tyrano dievaluasi secara dinamis berdasarkan analyzer, sama seperti engine lainnya.

- [ ] **Step 2: Edit `core/src/diagnostics/mod.rs`**
Hapus `LegalGate` dari `RemediationCategory` (atau biarkan jika dipakai di tempat lain yang bukan artificial gate). Hapus konstanta `TYR_LEGAL_GATE_BLOCKED` dan `LIC_REDISTRIBUTION_BLOCKED`.

- [ ] **Step 3: Edit `core/src/runtime_registry/mod.rs` & `runtimes/manifests/tyrano-web.toml`**
Ganti:
`name: "RuntimePack Tyrano Web Adapter (Subject to TYRANO-LEGAL gate)"` menjadi `"RuntimePack Tyrano Web Adapter"`.
Ganti `license_id: Some("PROPRIETARY-GATED")` menjadi `Some("MIT")`.

- [ ] **Step 4: Edit `core/src/provenance/mod.rs` & `core/src/build_plan/audit_policy.rs`**
Hapus string klaim gate S10 di `provenance/mod.rs` dan filter `TYR_LEGAL_GATE_BLOCKED` di `audit_policy.rs`.

- [ ] **Step 5: Verifikasi Kompilasi & Unit Test Core**
Run: `cargo test -p runtimepack-core`
Expected: PASS.

- [ ] **Step 6: Commit**
```bash
git add core/ runtimes/
git commit -m "refactor(core): remove artificial TYRANO-LEGAL status and proprietary gate from runtime registry"
```

---

### Task 2: Clean CLI, Engine Adapter, and Integration Tests

**Files:**
- Modify: `cli/src/main.rs:771-775`
- Modify: `engines/tyrano/src/scanner.rs:145-147`
- Modify: `engines/tyrano/src/asset_audit.rs:77-79,545-546,699-706`
- Modify: `tests/integration/build_test.rs:163-179`
- Modify: `tests/integration/tyrano_asset_audit_test.rs:146-155,681-684`

- [ ] **Step 1: Edit `cli/src/main.rs`**
Hapus blok peringatan CLI:
```rust
if descriptor.id == EngineId::Tyrano {
    println!(
        "Notice:         Tyrano distribution is subject to TYRANO-LEGAL gate review (informational)"
    );
}
```

- [ ] **Step 2: Edit `engines/tyrano/src/scanner.rs`**
Hapus kompensasi manual `if report.status == CompatibilityStatus::Blocked { report.status = CompatibilityStatus::Compatible; }` karena core sudah tidak lagi memaksakan Blocked.

- [ ] **Step 3: Edit `engines/tyrano/src/asset_audit.rs`**
Hapus injeksi otomatis finding `TYR-101` pada audit aset (lines 699-706) beserta pemetaan `TYR_LEGAL_GATE_BLOCKED` di `make_finding`.

- [ ] **Step 4: Update Unit & Integration Tests**
- Di `tests/integration/build_test.rs`: hapus atau sesuaikan `test_audit_policy_tyrano_legal_gate_not_hard_blocker`.
- Di `tests/integration/tyrano_asset_audit_test.rs`: hapus assertion yang mengharapkan `TYR_LEGAL_GATE_BLOCKED` muncul di audit.
Run: `cargo test --workspace`
Expected: 100% tests PASS (355+ tests).

- [ ] **Step 5: Commit**
```bash
git add cli/ engines/ tests/
git commit -m "fix(tyrano): remove artificial TYR-101 audit injection and CLI warning notice"
```

---

### Task 3: Clean AGENTS.md, Governance, and Project Documentation

**Files:**
- Modify: `AGENTS.md`
- Modify: `Docs/LICENSE_COMPLIANCE.md`
- Modify: `Docs/OPEN_QUESTIONS_AND_SPIKES.md`
- Modify: `Docs/TECHNICAL_SPEC_V1.md`
- Modify: `Docs/PRD.md`
- Modify: `Docs/ENGINE_SUPPORT_MATRIX.md`
- Modify: `Docs/COMPATIBILITY.md`
- Modify: `Docs/CONFIG_AND_SCHEMAS.md`
- Modify: `Docs/IMPLEMENTATION_PLAN.md`
- Modify: `Docs/TEST_PLAN.md`
- Modify: `Docs/REFERENCES.md`
- Modify: `Docs/AGENT_GUIDE.md`
- Modify: `Docs/README.md`
- Modify: `Docs/MULTI_ENGINE_VALIDATION_MATRIX.md`
- Modify: `Docs/RuntimePack_MASTER_SPEC_v1.md`
- Modify: `memory/context.md`

- [ ] **Step 1: Update `AGENTS.md`**
- §5 (Anti-hallucination): Hapus larangan "invent Tyrano redistribution permissions".
- §7.7 (Tyrano): Ubah instruksi agar TyranoScript diperlakukan setara dengan engine web lainnya (HTML5/KAG over Web runtime) tanpa gate pemblokiran rilis.
- §12, §19, §20: Hapus referensi `LICENSE_BLOCKED` dan `TYRANO-LEGAL` gate.

- [ ] **Step 2: Update `Docs/` Canonical Specifications**
- Hapus Bab 5 `TYRANO-LEGAL` di `LICENSE_COMPLIANCE.md`.
- Hapus `SPIKE-TYRANO-LEGAL-001` di `OPEN_QUESTIONS_AND_SPIKES.md`.
- Perbarui `ENGINE_SUPPORT_MATRIX.md` status Tyrano dari "Technical target, legal gate" menjadi "Supported".
- Sinkronkan `PRD.md`, `COMPATIBILITY.md`, `IMPLEMENTATION_PLAN.md`, `TEST_PLAN.md`, dan `RuntimePack_MASTER_SPEC_v1.md`.
- Hapus catatan gate di `memory/context.md`.

- [ ] **Step 3: Verifikasi Konsistensi Repositori**
Jalankan pencarian grep untuk memastikan tidak ada lagi referensi gate yang tertinggal:
```powershell
Select-String -Path "Docs\*.md", "AGENTS.md" -Pattern "TYRANO-LEGAL|TYR-101|LIC-310"
```
Expected: 0 matches (atau hanya arsip handover historis jika relevan).

- [ ] **Step 4: Commit**
```bash
git add AGENTS.md Docs/ memory/
git commit -m "docs: remove artificial legal gates and establish full first-class support for TyranoScript"
```

---

## Execution Handoff

Plan lengkap telah disimpan ke `Docs/superpowers/plans/2026-09-30-remove-artificial-legal-gates.md`.

Pelaksanaan dilakukan menggunakan pendekatan **Subagent-Driven Development (SDD)**:
- Task 1 & 2 didelegasikan ke sub-agent `executor` (Rust code & integration tests).
- Task 3 didelegasikan ke sub-agent `executor` (Documentation & governance cleanup).
- Seluruh diff direview oleh `code-reviewer` sebelum merge final.
