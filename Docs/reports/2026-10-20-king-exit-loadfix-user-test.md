# Checklist uji manual — King Exit Loadfix Beta

**Status:** VALIDATED_BY_USER_DEVICE_TEST — Pengguna mengonfirmasi secara manual di perangkat/emulator bahwa game sudah normal pada kandidat V2 aligned: menu teks bahasa Indonesia, save/load normal, crash `SystemStackError` tuntas teratasi.

**V2 package:** `io.runtimepack.game.rpgmkingexit.diagnostic.loadfixv2`
**V2 APK:** `C:\Users\MhsymR\AppData\Local\Temp\opencode\kingexit_loadfix_v2_aligned_20261020_185550\kingexit_diagnostic_loadfixv2_aligned.apk`
**V2 SHA-256:** `8ad4ed186fc694ec61edbdac155cefb712c723f128d68b84139f3ff5c2156088`

### Kandidat pertama — riwayat gagal, jangan gunakan untuk validasi fix terbaru

**Target package:** `io.runtimepack.game.rpgmkingexit.diagnostic.loadfix`
**APK:** `C:\Users\MhsymR\AppData\Local\Temp\opencode\kingexit_loadfix_20261020_042500\kingexit_diagnostic_loadfix.apk`
**SHA-256:** `b48eba2292b54c45d5d5e8a00ae054a79df833f279414119b04abe2a09c38e58`

## Sebelum mulai

- Jangan uninstall APK lama atau menghapus data aplikasinya; save app-private bisa hilang saat uninstall.
- Kandidat terpasang terpisah. Save dari APK lama tidak otomatis muncul karena package ID berbeda.
- Jangan pindahkan satu-satunya file save. Jika perlu uji save lama, gunakan salinan dengan backup terverifikasi; prosedur pemindahan bukan bagian dari checklist ini.
- Simpan hasil pengujian ke slot baru, jangan menimpa save asli.
- APK beta/debug-signed, bukan rilis publik. Workaround font kandidat sama dengan fontfix sebelumnya: Source Han Sans CN menggantikan VL Gothic, outline tetap aktif.

## Uji A — sesi baru

1. Buka kandidat Loadfix Beta, pilih New Game.
2. Saat menu tersedia, periksa teks:
   - `Gunakan item`, `Barang`, `Skill`, `Perlengkapan`, `Status`.
   - `Susun Ulang`, `Pencapaian`, `Berkas`, `Pengaturan`.
   - Nama aktor, lokasi, `Lokasi`, `Uang`, `Waktu Bermain`.
3. Pastikan tidak ada key seperti `49037624:35:19` pada label.
4. Save pada slot kosong. Load slot itu, buka menu, ulangi pemeriksaan.
5. Tutup aplikasi sepenuhnya, buka lagi, load slot uji yang sama.
6. Ulangi pemeriksaan menu, dialog, pause/resume, dan respons input.

## Uji B — salinan save lama (jika tersedia)

1. Backup save lama. Jangan mengubah satu-satunya salinan.
2. Gunakan salinan yang sudah ditempatkan dengan aman ke kandidat.
3. Load dan periksa semua label seperti Uji A.
4. Jika gagal, jangan langsung save ulang/overwrite. Ambil screenshot dan catat langkah.

## Laporan singkat

```text
Package / checksum kandidat:
New Game menu:
Save ke slot baru, lalu load:
Restart aplikasi, lalu load:
Salinan save lama (diuji/tidak):
Bahasa aktor dan lokasi:
Label menu angka masih muncul (ya/tidak):
Respons dialog/menu/battle:
Screenshot + langkah jika gagal:
```

**Catatan:** Angka pada screenshot sebelumnya merupakan key terjemahan `SCRIPT_ID:LINE:COLUMN`, bukan timestamp rusak. Perbaikan mengembalikan pembacaan bahasa dan lookup terjemahan; tidak mengubah penghitung waktu atau data save lama. Host test tidak menggantikan pengujian ini.
