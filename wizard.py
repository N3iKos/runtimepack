#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
RuntimePack Windows Native Interactive Build Wizard
Python implementation for robust input handling, path resolution, and CLI orchestration.
"""

import os
import sys
import json
import shutil
import subprocess
from pathlib import Path

# Ensure UTF-8 output on Windows console
if sys.platform == "win32":
    try:
        sys.stdout.reconfigure(encoding="utf-8")
        sys.stderr.reconfigure(encoding="utf-8")
    except Exception:
        pass


def clear_screen():
    os.system("cls" if os.name == "nt" else "clear")


def pause():
    input("\nTekan Enter untuk kembali ke menu utama...")


def clean_path_input(raw: str) -> str:
    if not raw:
        return ""
    cleaned = raw.strip()
    # Strip surrounding quotes from drag & drop
    if (cleaned.startswith('"') and cleaned.endswith('"')) or (cleaned.startswith("'") and cleaned.endswith("'")):
        cleaned = cleaned[1:-1].strip()
    return cleaned


def resolve_binary(bundle_dir: Path) -> Path:
    # 1. Environment variable override
    env_bin = os.environ.get("RUNTIMEPACK_EXE")
    if env_bin:
        cleaned_env = clean_path_input(env_bin)
        p = Path(cleaned_env)
        if p.is_file():
            return p.resolve()

    # 2. Bundle layouts
    binary_name = "runtimepack.exe" if sys.platform == "win32" else "runtimepack"
    candidates = [
        bundle_dir / "bin" / binary_name,
        bundle_dir / binary_name,
        bundle_dir / "target" / "release" / binary_name,
        bundle_dir / "target" / "debug" / binary_name,
    ]
    for c in candidates:
        if c.is_file():
            return c.resolve()

    # Fail closed
    print("=" * 79)
    print(f"[ERR-WIZ-001] Biner {binary_name} tidak ditemukan!")
    print("=" * 79)
    print(f"\nPastikan file '{binary_name}' berada di salah satu lokasi berikut:")
    for candidate in candidates:
        print(f"  {candidate}")
    print("\nAtau atur variabel lingkungan RUNTIMEPACK_EXE ke path biner tersebut:")
    if sys.platform == "win32":
        print("  set RUNTIMEPACK_EXE=C:\\path\\to\\runtimepack.exe\n")
    else:
        print("  export RUNTIMEPACK_EXE=/path/to/runtimepack\n")
    input("Tekan Enter untuk keluar...")
    sys.exit(1)


def run_doctor(bin_path: Path):
    clear_screen()
    print("=" * 79)
    print("                  MEMERIKSA LINGKUNGAN PENGEMBANGAN (DOCTOR)")
    print("=" * 79)
    print()
    subprocess.run([str(bin_path), "doctor"])
    pause()


def run_inspect(bin_path: Path):
    clear_screen()
    print("=" * 79)
    print("                        INSPEKSI SUMBER GAME (INSPECT)")
    print("=" * 79)
    print()
    raw = input("Drag and drop folder game ke jendela ini, lalu tekan Enter:\n> ")
    game_path = clean_path_input(raw)
    if not game_path:
        return

    p = Path(game_path)
    if not p.is_dir():
        print(f"\n[ERROR]: Folder game tidak ditemukan: '{game_path}'")
        input("Tekan Enter untuk kembali...")
        return

    clear_screen()
    print("=" * 79)
    print(f"Hasil Inspeksi Sumber Game: {game_path}")
    print("=" * 79)
    print()
    subprocess.run([str(bin_path), "inspect", str(p)])
    pause()


def run_explain(bin_path: Path):
    clear_screen()
    print("=" * 79)
    print("                    DETEKSI DAN ANALISIS ENGINE GAME (EXPLAIN)")
    print("=" * 79)
    print()
    raw = input("Drag and drop folder game ke jendela ini, lalu tekan Enter:\n> ")
    game_path = clean_path_input(raw)
    if not game_path:
        return

    p = Path(game_path)
    if not p.is_dir():
        print(f"\n[ERROR]: Folder game tidak ditemukan: '{game_path}'")
        input("Tekan Enter untuk kembali...")
        return

    clear_screen()
    print("=" * 79)
    print(f"Hasil Deteksi Engine Game: {game_path}")
    print("=" * 79)
    print()
    subprocess.run([str(bin_path), "explain", str(p)])
    pause()


def run_audit(bin_path: Path):
    clear_screen()
    print("=" * 79)
    print("                           AUDIT ASET GAME (AUDIT)")
    print("=" * 79)
    print()
    raw = input("Drag and drop folder game ke jendela ini, lalu tekan Enter:\n> ")
    game_path = clean_path_input(raw)
    if not game_path:
        return

    p = Path(game_path)
    if not p.is_dir():
        print(f"\n[ERROR]: Folder game tidak ditemukan: '{game_path}'")
        input("Tekan Enter untuk kembali...")
        return

    print("\nPilih mode audit aset:")
    print("[1] Standard (Cepat - verifikasi referensi berkas statis langsung)")
    print("[2] Deep     (Mendalam - pemindaian skrip dan database lengkap)")
    mode_sel = input("Pilihan [1/2, default: 1]: ").strip()
    audit_mode = "deep" if mode_sel == "2" else "standard"

    print("\nFolder Run-Time Package (RTP) opsional:")
    print("(Tekan Enter jika game standalone atau tidak membutuhkan RTP eksternal)")
    rtp_raw = input("> ")
    rtp_path = clean_path_input(rtp_raw)

    cmd = [str(bin_path), "audit", str(p), "--mode", audit_mode]
    if rtp_path:
        rtp_p = Path(rtp_path)
        if rtp_p.is_dir():
            cmd.extend(["--rtp-path", str(rtp_p)])
        else:
            print("[PERINGATAN]: Folder RTP tidak ditemukan, opsi dilewati.")

    clear_screen()
    print("=" * 79)
    print(f"Memulai Audit Aset ({audit_mode}): {game_path}")
    print("=" * 79)
    print()
    res = subprocess.run(cmd)
    print()
    print("-" * 79)
    print(f"[INFO] Eksekusi audit selesai dengan kode keluar: {res.returncode}")
    print("Laporan audit tersimpan di asset-audit-report.json dan asset-audit-report.html")
    print("(Catatan: Audit menemukan kelengkapan aset; hasil FAIL tidak berarti kompatibel/siap APK).")
    pause()


def resolve_output_root(bundle_dir: Path) -> Path:
    """Directory that receives per-build ``run_<n>`` folders.

    Defaults to ``<bundle>/dist``. ``RUNTIMEPACK_WIZARD_OUTPUT_ROOT`` overrides
    it, which is the supported way to build from a read-only or shared install
    location.
    """
    override = clean_path_input(os.environ.get("RUNTIMEPACK_WIZARD_OUTPUT_ROOT", ""))
    if override:
        return Path(override)
    return bundle_dir / "dist"


def prepare_run_output_dir(output_root: Path) -> Path:
    """Creates the next unused ``run_<n>`` directory or fails with ERR-WIZ-002."""
    try:
        output_root.mkdir(parents=True, exist_ok=True)
        counter = 1
        while (output_root / f"run_{counter}").exists():
            counter += 1
        run_output_dir = output_root / f"run_{counter}"
        run_output_dir.mkdir(parents=True, exist_ok=False)
        return run_output_dir
    except OSError as exc:
        print()
        print("=" * 79)
        print("[ERR-WIZ-002] Tidak dapat membuat direktori output build!")
        print("=" * 79)
        print(f"\nLokasi yang dicoba : {output_root}")
        print(f"Penyebab           : {exc}")
        print("\nKemungkinan penyebab dan solusinya:")
        print("  1. Lokasi bundle bersifat read-only (misal di Program Files atau media ROM).")
        print("     -> Set variabel lingkungan RUNTIMEPACK_WIZARD_OUTPUT_ROOT ke folder yang")
        print("        bisa ditulis, contoh:")
        print("        set RUNTIMEPACK_WIZARD_OUTPUT_ROOT=%USERPROFILE%\\RuntimePackBuilds")
        print("  2. Path menunjuk ke berkas, bukan folder.")
        print("     -> Periksa kembali nilai RUNTIMEPACK_WIZARD_OUTPUT_ROOT.")
        print("  3. Ruang disk penuh atau izin folder ditolak.")
        print("     -> Kosongkan ruang disk atau gunakan folder lain yang Anda miliki.")
        print("\nTidak ada berkas game yang diubah. Tekan Enter untuk kembali...")
        try:
            input()
        except (EOFError, KeyboardInterrupt):
            pass
        sys.exit(1)


def run_build_wizard(bin_path: Path, bundle_dir: Path):
    clear_screen()
    print("=" * 79)
    print("                       WIZARD BUILD STANDALONE APK")
    print("=" * 79)
    print()

    # Langkah 1: Folder Game Sumber
    print("Langkah 1/6: Tentukan Lokasi Folder Game Sumber")
    print("-" * 79)
    print("Drag and drop folder game ke jendela ini, lalu tekan Enter:")
    print("(Catatan: Folder sumber game 100% aman [read-only] dan tidak dimodifikasi).")
    raw_game = input("> ")
    game_path = clean_path_input(raw_game)
    if not game_path:
        return

    game_dir = Path(game_path)
    if not game_dir.is_dir():
        print(f"\n[ERROR]: Folder game tidak ditemukan: '{game_path}'")
        input("Tekan Enter untuk kembali ke menu utama...")
        return

    # Langkah 2: Nama Tampilan
    print()
    print("Langkah 2/6: Nama Tampilan Aplikasi Android")
    print("-" * 79)
    default_title = game_dir.name or "GameApp"
    if default_title == ".":
        default_title = "GameApp"
    print(f"Nama tampilan aplikasi di layar HP (default: {default_title}):")
    raw_name = input("> ").strip()
    game_name = clean_path_input(raw_name) or default_title

    # Langkah 3: Folder RTP (Opsional)
    print()
    print("Langkah 3/6: Folder RTP Eksternal (Opsional)")
    print("-" * 79)
    print("(Tekan Enter jika game tidak memerlukan folder aset RTP terpisah):")
    raw_rtp = input("> ")
    rtp_path = clean_path_input(raw_rtp)
    valid_rtp = None
    if rtp_path:
        p_rtp = Path(rtp_path)
        if p_rtp.is_dir():
            valid_rtp = str(p_rtp)
        else:
            print("[PERINGATAN]: Folder RTP tidak ditemukan, opsi dilewati.")

    # Langkah 4: Profil Kompatibilitas (Opsional)
    print()
    print("Langkah 4/6: File Profil Kompatibilitas Patch JSON (Opsional)")
    print("-" * 79)
    print("Tekan Enter untuk lewati (hanya untuk file patch .json khusus seperti di compatibility-db/).")
    print("Catatan: Cheat profile (seperti cheats.js) disuntikkan OTOMATIS jika tersedia.")
    raw_prof = input("> ")
    prof_path = clean_path_input(raw_prof)
    valid_prof = None
    if prof_path:
        p_prof = Path(prof_path)
        if not p_prof.is_file():
            print("[PERINGATAN]: File profil tidak ditemukan, opsi dilewati.")
        elif p_prof.suffix.lower() != ".json":
            print("[PERINGATAN]: Profil kompatibilitas harus berupa file patch .json!")
            print("             File 'cheats.js' adalah cheat profile yang otomatis disuntikkan oleh sistem.")
            print("             Opsi ini dilewati agar build tidak gagal.")
        else:
            valid_prof = str(p_prof)

    # Langkah 5: Flavor & Content Mode
    print()
    print("Langkah 5/6: Tahap Rilis Aplikasi (--flavor)")
    print("-" * 79)
    print("[1] Beta Test Komunitas (Default - Disarankan)")
    print("    > Memuat Beta Watermark dan panel diagnostik.")
    print("    > Konten dikemas dalam mode container aman (--content-mode packed).")
    print("[2] Rilis Final")
    print("    > Tanpa watermark diagnostik.")
    print("    > Aset game dikemas terenkripsi streaming (--content-mode encrypted-streaming).")
    flavor_sel = input("Pilih tahap rilis [1/2, default: 1]: ").strip()
    if flavor_sel == "2":
        flavor_flag = "release"
        content_flag = "encrypted-streaming"
    else:
        flavor_flag = "beta"
        content_flag = "packed"

    # Langkah 6: Toleransi Kebijakan
    print()
    print("Langkah 6/6: Kebijakan Audit Aset dan Deteksi Engine (Opsional)")
    print("-" * 79)
    print("Default audit: informatif; temuan tetap dilaporkan dan build dilanjutkan.")
    print("Catatan: Engine yang ambigu atau berkeyakinan rendah (LOW) serta dependensi")
    print("native yang tidak didukung TETAP fatal dan akan membatalkan build.\n")

    extra_flags = []
    opt_strict = input("Aktifkan audit ketat (blokir FAIL/INDETERMINATE)? [Y/N, default: N]: ").strip().lower()
    if opt_strict == "y":
        extra_flags.append("--strict-audit")

    opt_det = input("Izinkan deteksi engine berkeyakinan sedang (MEDIUM)? [Y/N, default: N]: ").strip().lower()
    if opt_det == "y":
        extra_flags.append("--accept-detection")

    # Tentukan output unik <output_root>/run_N
    output_root = resolve_output_root(bundle_dir)
    run_output_dir = prepare_run_output_dir(output_root)

    # Konfirmasi Parameter
    clear_screen()
    print("=" * 79)
    print("                       KONFIRMASI PARAMETER BUILD")
    print("=" * 79)
    print(f"\nFolder Sumber Game : \"{game_dir}\"")
    print(f"Nama Aplikasi      : \"{game_name}\"")
    print("Mode Overlay       : full")
    print(f"Flavor Rilis       : {flavor_flag}")
    print(f"Mode Konten        : {content_flag}")
    if valid_rtp:
        print(f"RTP Eksternal      : \"{valid_rtp}\"")
    if valid_prof:
        print(f"Profil Kompatibilitas : \"{valid_prof}\"")
    if extra_flags:
        print(f"Opsi Toleransi     : {' '.join(extra_flags)}")
    print(f"Direktori Output   : \"{run_output_dir}\"")
    print("\n" + "=" * 79)

    confirm = input("Mulai proses build sekarang? [Y/N, default: Y]: ").strip().lower()
    if confirm == "n":
        print("Build dibatalkan pengguna.")
        pause()
        return

    # Susun perintah eksekusi
    cmd = [
        str(bin_path),
        "build",
        str(game_dir),
        "--name",
        game_name,
        "--overlay-mode",
        "full",
        "--flavor",
        flavor_flag,
        "--content-mode",
        content_flag,
        "--output",
        str(run_output_dir),
    ]
    if valid_rtp:
        cmd.extend(["--rtp-path", valid_rtp])
    if valid_prof:
        cmd.extend(["--compat-profile", valid_prof])
    cmd.extend(extra_flags)

    clear_screen()
    print("=" * 79)
    print("                 MEMULAI PROSES BUILD STANDALONE APK...")
    print("=" * 79)
    print()

    proc = subprocess.run(cmd)

    print()
    print("=" * 79)
    print("                       VERIFIKASI HASIL BUILD")
    print("=" * 79)
    print()

    # Verifikasi ketat artefak
    manifest_file = run_output_dir / "build-manifest.json"
    apks = list(run_output_dir.glob("*.apk"))

    failed = False
    if proc.returncode != 0:
        print(f"[GAGAL]: Proses build berhenti dengan kode error: {proc.returncode}")
        failed = True
    elif not manifest_file.is_file():
        print(f"[GAGAL]: Berkas build-manifest.json tidak ditemukan di folder output: {run_output_dir}")
        failed = True
    else:
        try:
            with open(manifest_file, "r", encoding="utf-8") as f:
                manifest_data = json.load(f)
            apk_hash = manifest_data.get("provenance", {}).get("apk_hash")
            if not apk_hash:
                print("[GAGAL]: Field apk_hash di build-manifest.json kosong atau bernilai null!")
                failed = True
        except Exception as e:
            print(f"[GAGAL]: Tidak dapat memvalidasi build-manifest.json: {e}")
            failed = True

    if not failed:
        if len(apks) != 1:
            print(f"[GAGAL]: Verifikasi APK tidak valid! Ditemukan {len(apks)} file APK di folder output (wajib tepat 1).")
            failed = True

    if failed:
        print("\n" + "=" * 79)
        print("                          BUILD DINYATAKAN GAGAL")
        print("=" * 79)
        print("Silakan periksa pesan diagnostik dan log di atas untuk panduan perbaikan.")
        pause()
        return

    apk_file = apks[0]
    print("=" * 79)
    print("                          BUILD BERHASIL! (SUKSES)")
    print("=" * 79)
    print(f"\nAPK Berhasil Dibuat : {apk_file}")
    print(f"Direktori Output    : {run_output_dir}")
    print(f"Manifest Build      : {manifest_file}")
    print()

    # Buka explorer otomatis di Windows
    if sys.platform == "win32":
        try:
            os.startfile(str(run_output_dir))
        except Exception:
            subprocess.run(["explorer", str(run_output_dir)])

    pause()


def main():
    bundle_dir = Path(__file__).resolve().parent
    bin_path = resolve_binary(bundle_dir)

    while True:
        clear_screen()
        print("=" * 79)
        print("                      RUNTIMEPACK ANDROID BUILD WIZARD")
        print("        Package Desktop Games into Standalone Native Android Applications")
        print("=" * 79)
        print(f"\n[Biner Aktif]: {bin_path}\n")
        print("[1] Cek Lingkungan dan Toolchain (Doctor)")
        print("[2] Inspeksi Struktur Game (Inspect)")
        print("[3] Analisis dan Deteksi Engine Game (Explain)")
        print("[4] Audit Aset Game (Cek File Hilang / Case Discrepancy)")
        print("[5] Build APK Standalone (Wizard Interaktif)")
        print("[0] Keluar")
        print("\n" + "=" * 79)

        choice = input("Pilih nomor [0-5]: ").strip()
        if choice == "1":
            run_doctor(bin_path)
        elif choice == "2":
            run_inspect(bin_path)
        elif choice == "3":
            run_explain(bin_path)
        elif choice == "4":
            run_audit(bin_path)
        elif choice == "5":
            run_build_wizard(bin_path, bundle_dir)
        elif choice == "0":
            clear_screen()
            print("Terima kasih telah menggunakan RuntimePack!")
            sys.exit(0)
        else:
            print("\nPilihan tidak valid. Silakan coba lagi.")


if __name__ == "__main__":
    try:
        main()
    except (KeyboardInterrupt, EOFError):
        print("\nWizard ditutup.")
        sys.exit(0)
