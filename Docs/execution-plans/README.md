# Execution Plans — Ekspansi Engine Visual Novel

Direktori ini adalah **workspace rencana eksekusi aktif** untuk ekspansi RuntimePack ke tiga engine visual novel prioritas. Semua track bersifat *decoupled* (independen) dan dapat dieksekusi dengan urutan bebas sesuai prioritas.

Untuk arsip rencana yang sudah selesai, lihat `Docs/superpowers/plans/` (plan) dan `Docs/superpowers/specs/` (spec). Untuk laporan riset pendukung, lihat `Docs/research/`.

---

## Master Roadmap

- [`./master-roadmap.md`](./master-roadmap.md) — Roadmap induk yang membagi ekspansi ke dalam 3 Track independen (VNM, Kirikiri 2/Z, Artemis Engine).

---

## Track A — Visual Novel Maker (VNM)

HTML5/WebGL visual novel suite (Degica / Gotcha Gotcha Games).

- **Runtime:** `WebActivity` (WebGL / NW.js polyfills, bypass Apache Cordova).
- **Status:** Ready to execute.
- **Dokumen:**
  - [`./track-a-visual-novel-maker/plan.md`](./track-a-visual-novel-maker/plan.md) — Implementation plan.
  - [`./track-a-visual-novel-maker/design.md`](./track-a-visual-novel-maker/design.md) — Design specification.

---

## Track B — Kirikiri 2 / Kirikiri Z (KAG / TVP Engine)

C++/TJS2 visual novel engine dengan pipeline 4 fase sekuensial.

- **Runtime:** native C++ `libkrkr2.so` (NDK r28, 16 KB page alignment).
- **Status:** Ready to execute.
- **Dokumen:**
  - [`./track-b-kirikiri/master-roadmap.md`](./track-b-kirikiri/master-roadmap.md) — Master roadmap track B.
  - [`./track-b-kirikiri/phase-1-plan.md`](./track-b-kirikiri/phase-1-plan.md) — Phase 1 plan: Rust host tooling & in-memory XP3 parser.
  - [`./track-b-kirikiri/phase-2-plan.md`](./track-b-kirikiri/phase-2-plan.md) — Phase 2 plan: Compatibility scanner & plugin validator.
  - [`./track-b-kirikiri/phase-3-plan.md`](./track-b-kirikiri/phase-3-plan.md) — Phase 3 plan: Headless native C++ runtime & Android shell.
  - [`./track-b-kirikiri/phase-4-plan.md`](./track-b-kirikiri/phase-4-plan.md) — Phase 4 plan: Packaging pipeline, VPad overlay & end-to-end build.
  - [`./track-b-kirikiri/architecture-spec.md`](./track-b-kirikiri/architecture-spec.md) — Complete architecture specification.
  - [`./track-b-kirikiri/phase-1-design.md`](./track-b-kirikiri/phase-1-design.md) — Phase 1 design specification.

---

## Track C — Artemis Engine (PFS / Lua)

Lua-scripted visual novel engine (iMel Inc. / Mikage).

- **Runtime:** `art3m1s-core` C FFI + `ArtemisActivity.kt`.
- **Status:** Ready to execute.
- **Dokumen:**
  - [`./track-c-artemis/plan.md`](./track-c-artemis/plan.md) — Implementation plan.
  - [`./track-c-artemis/design.md`](./track-c-artemis/design.md) — Design specification.

---

## Catatan

- **Arsip rencana selesai:** `Docs/superpowers/plans/` (plan) dan `Docs/superpowers/specs/` (spec).
- **Laporan riset:** `Docs/research/`.
