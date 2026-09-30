# SDD ledger — plan: Docs/superpowers/plans/2026-09-30-remove-artificial-legal-gates.md

## Pre-flight

- Worktree: `.worktrees/remove-legal-gates`, branch `feature/remove-legal-gates`, BASE `8d0f4a6`.
- Audit explorer (ses_f0caec0c5ffe8qjZkgGUWg5nPr) memetakan seluruh gate artifisial: hanya Tyrano yang terkena; engine lain bersih.
- Spec: hasil audit + plan.

## Conflict scan (pre-flight rulings)

| Pair / Task | Produces vs Consumes | Finding | Ruling |
| :--- | :--- | :--- | :--- |
| Task 1 (hapus TYR-101 dari core) vs Task 2 (hapus injeksi di tyrano + test) | Test `tyrano_asset_audit_test.rs` meng-assert TYR-101 ADA; Task 1 menghapus konstanta → compile error bila test tidak diupdate | Task 1 & 2 harus dalam SATU urutan build hijau: Task 1 compile mungkin rusak sementara sampai Task 2 selesai → gabung Task 1+2 ke SATU dispatch executor agar workspace selalu hijau per commit | Ruling: eksekusi Task 1+2 sebagai satu unit kerja, commit terpisah tetap boleh |
| Task 3 (dokumen) vs AGENTS.md | AGENTS.md berisi kontrak supervisor user — JANGAN hapus bagian supervisor/workflow user | Executor diberi batasan eksplisit: hanya bersihkan pasal terkait legal-gate (§5 item terkait Tyrano, §7.7, §12, §19, §20); jangan sentuh kontrak supervisor/sub-agent driven di bagian atas | Binding |
| Schema BLOCKED enum | `compatibility-report.schema.json` punya enum "BLOCKED" | BLOCKED masih dipakai sebagai status kompatibilitas teknis umum (bukan gate legal) — review explorer menyarankan optional. Ruling: pertahankan enum BLOCKED di schema (bukan artificial gate, bisa dipakai untuk incompat nyata); hapus hanya penggunaannya untuk Tyrano | Binding |
| memory/ vs plan | Task 3 menyentuh memory/context.md | Hanya hapus baris gate; jangan sentuh entry lain | Binding |

## Progress
