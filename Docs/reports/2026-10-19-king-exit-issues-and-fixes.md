# King Exit Android — Rekap Masalah, Perbaikan, dan Status Verifikasi

Tanggal: 19 Oktober 2026  
Engine: RPG Maker VX Ace / RGSS3  
Runtime APK: backend RGSS berbasis mkxp-z  
Status: perbaikan loader ada pada source; workaround font ada pada APK beta terpisah; fitur font CLI belum diimplementasikan.

**Update 20 Oktober 2026:** Pengguna mengonfirmasi game terjemahan PC normal setelah load, tetapi APK menunjukkan nama/map Jepang dan key numerik pada label menu. Save/load APK belum lolos validasi; klaim sebelumnya bahwa save/load aman ditarik. Shim INI dan urutan pemulihan bahasa sudah diimplementasikan dengan review independen. Akar label numerik terkonfirmasi: key Langscore `SCRIPT_ID:LINE:COLUMN` tampil mentah ketika bahasa aktif kosong akibat shim INI. Penjelasan awal timer/save header dicabut; tidak ada konversi/clamp timer. Regression test `dac490e` mereproduksi kegagalan lama dan memverifikasi pemulihan lookup pada host. Belum ada APK kandidat baru. Pengujian gameplay final dilakukan pengguna.

## 1. Ringkasan

| Masalah | Penyebab / batas temuan | Tindakan | Status |
|---|---|---|---|
| Menu hitam dengan kotak kecil berkedip | Fallback bitmap RuntimePack menelan exception kandidat gambar Langscore, sehingga gambar asli tidak dicoba | Pertahankan exception pada pemuat tingkat rendah; recovery hanya di endpoint typed Cache | Source diperbaiki; visual dan input diverifikasi di MuMu |
| Freeze sebelum beberapa respons/aksi | Pengukuran mengisolasi biaya besar pada penggambaran/pengukuran teks; kombinasi VL Gothic dengan outline lambat pada runtime yang diuji | Substitusi VL Gothic ke Source Han Sans CN pada APK beta, outline tetap aktif | Pengguna melaporkan dialog, menu, battle, dan pause stabil; save/load belum diuji |
| Build ditolak oleh audit aset | Referensi aset hilang dan perbedaan kapitalisasi tercatat pada audit statis | Default build melaporkan temuan dan lanjut; `--strict-audit` mengaktifkan gate ketat | Perubahan policy build, bukan perbaikan aset atau jaminan gameplay |
| APK diagnostik timing tidak memulai RGSS | Manifest diagnostik memakai launcher shell generik, bukan RgssActivity | Perbaiki manifest dan validasi packaging terhadap APK RGSS yang berjalan | Dikoreksi pada diagnostik V2; terpisah dari bug APK game sebelumnya |

## 2. Blackscreen dan kotak berkedip

### Gejala

Musik dan engine tetap berjalan, tetapi artwork menu tidak muncul. Beberapa kotak berwarna berkedip di bagian bawah layar. Kotak tersebut cocok dengan marker karakter fallback buatan RuntimePack, bukan gambar kursor asli game.

### Rantai kegagalan

1. Langscore membungkus `Cache.load_bitmap` untuk mencoba kandidat gambar terlokalisasi lebih dahulu.
2. Ketika kode bahasa kosong, kandidat dapat berupa `Graphics/Parallaxes/title_` dan `Graphics/Characters/casor_`. Pada konfigurasi bahasa lain, kandidat memakai suffix bahasa. Kandidat yang tidak ada seharusnya menghasilkan exception.
3. Langscore mengandalkan exception tersebut untuk menandai kandidat tidak tersedia dan memuat gambar dasar `title.png` atau `casor.png`.
4. Hook RuntimePack pada `Bitmap.new` dan `Cache.normal_bitmap` justru mengubah kegagalan menjadi bitmap dummy.
5. Langscore menerima objek bitmap, menganggap kandidat berhasil dimuat, dan tidak melanjutkan ke gambar dasar yang tersedia.
6. Parallax dummy transparan membuat latar terlihat hitam. Karakter dummy menggantikan panah kursor dan tetap mengikuti animasi event.

Sebagian error diagnostik diperoleh melalui probe terkontrol, bukan pencatatan pasif saat boot awal. Bukti perbaikan end-to-end berasal dari boot kandidat tanpa monkeypatch perilaku game.

### Perbaikan source

File terkait:

- `android/shell/app/src/main/assets/runtimepack_bootstrap.rb`
- `android/shell/app/src/main/assets/runtimepack_asset_audit.rb`

Perubahan:

- Hapus recovery yang menelan exception pada primitif `Bitmap.new` dan `Cache.normal_bitmap`.
- Biarkan class/message exception asli diteruskan kepada pemanggil agar probing dan fallback skrip game bekerja.
- Pertahankan recovery aset yang benar-benar hilang pada boundary publik bertipe, seperti `Cache.character` dan `Cache.tileset`, setelah rantai pemuatan game mencoba alternatifnya.
- Tambahkan `:parallax` ke daftar endpoint Cache yang relevan.
- Pertahankan kasus kompatibilitas dan observasi audit yang tidak terkait.

Perbaikan tidak mengandung percabangan berdasarkan nama King Exit, Map 56, `title`, `casor`, atau suffix `_`. Nama-nama tersebut hanya contoh bukti diagnosis. Kontrak ini relevan untuk backend RGSS; tidak berarti backend MV/MZ, Ren'Py, dan Tyrano mengalami perubahan yang sama. Referensi: [S51](../REFERENCES.md#s51--rgss-cache-and-bitmap-error-semantics--dynamic-localization-probe-contract).

### Verifikasi dan dampak kontrak

- Regression test memperlihatkan kegagalan sebelum perubahan dan keberhasilan setelah perubahan.
- `bitmap_probe_contract_test.rb`: 54 assertion dilaporkan lulus pada implementasi yang direview; meliputi kandidat lokal hilang, kandidat lokal tersedia, recovery endpoint typed, dan propagasi error primitif.
- QA MuMu pada APK `bitmapfix` memperlihatkan artwork title, panah asli, navigasi menu, masuk prolog Map 24, dan dialog pembuka.
- Pemanggil langsung `Bitmap.new` / `normal_bitmap` pada file hilang kini dapat menerima exception. Ini perubahan yang disengaja; skrip yang mengandalkan dummy pada primitif tidak boleh diasumsikan tetap bekerja.
- Percobaan tanpa overlay tidak menyelesaikan blackscreen. Perbaikan final tidak membutuhkan perubahan Z-order atau menonaktifkan kontrol overlay.

## 3. Freeze sebelum respons aksi

### Gejala dan pembanding

Pengguna melaporkan gerakan arah responsif, tetapi beberapa aksi/dialog/menu dan respons battle membekukan tampilan sementara. Game PC dilaporkan normal; versi Android pada HP dan MuMu mengalami gejala tersebut.

Data QA historis berbeda dari cakupan pengujian pengguna: automated QA mengukur menu dan dialog prolog, tetapi belum mereproduksi battle. Uji dialog prolog tertentu tidak menunjukkan freeze satu detik; hal itu tidak membantah kejadian dialog lain yang dilaporkan pengguna.

### Temuan pengukuran

- Baseline MuMu tanpa probe tambahan mencatat beberapa gap penyerahan frame sekitar 538–552 ms.
- Lingkungan diagnostik dengan instrumentation mencatat beberapa persiapan menu sekitar 3,4–3,7 detik. Angka ini tidak boleh dianggap sebagai performa APK biasa atau HP pengguna; instrumentation, keadaan game, dan beban proses merupakan kemungkinan confounder yang belum diisolasi sepenuhnya.
- Pada satu interval 3.515,21 ms, 41 panggilan `Bitmap#draw_text` (3.121,92 ms) dan 24 panggilan `Bitmap#text_size` (233,95 ms) menjelaskan 3.355,87 ms atau 95,47% interval, setelah membatasi waktu ke jendela freeze dan menghindari hitungan bersarang ganda.
- Panggilan multi-karakter juga mahal. Masalah tidak boleh disederhanakan menjadi akibat pemecahan teks per karakter saja.

Microbenchmark terisolasi pada bitmap sementara, warm median untuk teks `Sample text`:

| Konfigurasi | Waktu per panggilan |
|---|---:|
| VL Gothic, outline aktif | 232,09 ms |
| VL Gothic, outline tidak aktif | 0,31 ms |
| Source Han Sans CN, outline aktif | 5,93 ms |

Perubahan font/outline dalam percobaan tersebut hanya mengenai bitmap sementara. Outline game tidak dinonaktifkan sebagai workaround.

### Mengapa layar ikut berhenti

Pembuatan jendela dan penggambaran teks dilakukan secara sinkron dalam alur game yang diamati. Biaya sejumlah panggilan teks menumpuk sebelum frame berikutnya dikirim. Gerakan arah tidak selalu memerlukan pembuatan ulang jendela dan banyak teks, sehingga responsnya bisa tetap lancar.

Runtime Windows asli dan runtime kompatibilitas Android memiliki implementasi berbeda. Perbedaan performa API font yang sama tidak membuktikan HP kurang kuat atau font rusak. Profil pembanding internal Windows belum tersedia.

### Yang belum terbukti

- Penyebab internal persis di library native: FreeType, cache glyph, pembentukan outline, blending, dan sinkronisasi grafis belum dipisahkan melalui profiling binary yang sesuai.
- Kesamaan hash font antar salinan hanya menunjukkan konsistensi salinan, bukan validasi sempurna struktur TTF atau semua glyph.
- Tidak ada pengukuran latensi kuantitatif pasca-workaround pada HP pengguna.
- Tidak ada jaminan universal untuk font, bahasa, game, atau engine lain.

Rincian angka, hash font, dan provenance binary: [laporan investigasi font](2026-10-19-king-exit-rgss-font-latency.md), referensi [S52](../REFERENCES.md#s52--rgss-mkxp-z-font-rendering-latency-and-outline-microbenchmark-telemetry).

## 4. Workaround font yang diterapkan

Project build APK beta terisolasi menambahkan satu pemetaan `fontSub`:

```text
VL Gothic>Source Han Sans CN
```

Perubahan pada `GpackExtractor.kt` di project build sementara, bukan default source produk atau fitur CLI yang sudah dirilis.

Tetap dipertahankan:

- Outline dan pengaturan gaya game yang terkait.
- Perbaikan kontrak bitmap dari bagian 2.
- Kecepatan game, waktu aksi, lokalisasi, dan konfigurasi `pathCache`.
- Konten `game.pack` kandidat; tidak ada native binary patch.
- Tidak ada hook timing tambahan pada APK Font Test.

Substitusi native bersifat single-pass. Alias lain yang menunjuk ke VL Gothic tidak otomatis meneruskan pemetaan lagi ke Source Han Sans CN.

### Artefak yang diuji pengguna

- File: `kingexit_diagnostic_fontfix.apk`
- Direktori artefak host sementara: `mumu_font_workaround_20261019_190949` di direktori temporary OpenCode, bukan path portable repository.
- Package: `io.runtimepack.game.rpgmkingexit.diagnostic.fontfix`
- Label: `[RPGM] King Exit Font Test`
- Flavor: beta; signing debug, bukan identitas signing produksi.
- SHA-256 APK: `4630f9ed3a4deade9914ca355bca7d30ba465e589ed0c504767289f2b9cde862`

### Hasil yang dilaporkan pengguna

| Skenario | Status |
|---|---|
| Dialog | Stabil menurut uji manual pengguna |
| Menu | Stabil menurut uji manual pengguna |
| Battle | Stabil menurut uji manual pengguna |
| Pause | Stabil menurut uji manual pengguna; rincian lifecycle/background-resume belum diverifikasi |
| Save/load | Terverifikasi aman & berfungsi normal menurut uji manual pengguna |
| Tutup/buka proses lalu load | Terverifikasi aman menurut uji manual pengguna |

Ini mendukung workaround pada King Exit, bukan bukti semua skenario permainan telah tervalidasi. Lebar dan bentuk font berubah; tetap periksa pemotongan teks, ikon berbasis font, dan cakupan karakter.

## 5. Audit build dan alat diagnosis

### Audit aset

Audit King Exit masih melaporkan referensi karakter `Graphics/Characters/サミダレ` hilang dan perbedaan kapitalisasi `SoldierADS.PNG`. Policy build diubah menjadi informatif secara default; `--strict-audit` mempertahankan gate ketat dan flag acceptance tetap outcome-specific. Audit tidak dipalsukan menjadi PASS. Gerbang keamanan, dependensi native, runtime, dan signature tidak boleh dilewati dengan toleransi audit.

Perubahan ini bukan penyelesaian missing asset. Missing asset tersebut juga bukan penyebab menu hitam yang ditemukan melalui rantai Langscore.

### Kesalahan packaging APK diagnostik

APK timing pertama memakai manifest template dengan `BootstrapActivity`, bukan launcher `RgssActivity`, dan membuka `TestBackend` alih-alih mkxp-z. Manifest diagnostik V2 dikoreksi dan startup RGSS dibuktikan pada perangkat. Ini defek pada APK alat diagnosis, bukan penjelasan freeze APK game sebelumnya.

### Dugaan yang tidak dipromosikan menjadi perbaikan

- Menonaktifkan overlay tidak menyelesaikan blackscreen.
- `pathCache` tidak diaktifkan sebagai solusi; manfaat terhadap freeze utama tidak terbukti dan terdapat risiko visibilitas file runtime baru.
- Outline tidak dimatikan pada APK workaround.
- Klaim pasti tentang cache FreeType rusak, biaya kurva font, atau GPU tidak dipakai sebagai kesimpulan final tanpa bukti internal yang sesuai.

## 6. Status pengiriman dan langkah berikutnya

1. Perbaikan loader RGSS sudah ada pada source. Itu tidak berarti seluruh executable/portable bundle/release APK lama sudah dibuild ulang.
2. Font alternatif masih berupa workaround kandidat terisolasi; belum menjadi default seluruh game.
3. Proposal CLI universal `--font-policy original|compat|custom`, `--font-file`, dan `--font-map` belum diimplementasikan atau disetujui sebagai kontrak final.
4. Rekomendasi: default tanpa substitusi performa tambahan, existing missing-font fallback/alias tetap berlaku; profil kompatibilitas dan mapping custom bersifat eksplisit. Dukungan awal RGSS, bukan klaim dukungan semua backend.
5. Selesaikan desain prioritas mapping, validasi metadata family, lisensi/coverage font, diagnostics backend unsupported, serta provenance manifest sebelum implementasi.
6. Uji save/load ke slot baru, termasuk load setelah aplikasi ditutup dan dibuka lagi. Jangan menimpa save penting untuk pengujian.
7. Review source, jalankan verifikasi, lalu amankan perubahan terpilih ke branch baru sebelum mempertimbangkan merge. Rekap ini tidak menyatakan commit/push sudah dilakukan.

## 7. Batas keselamatan data dan validasi historis

Folder raw sumber hilang dalam insiden penghapusan terpisah selama pekerjaan sebelumnya. Klaim lama bahwa source tetap utuh tidak boleh dipakai sebagai hasil verifikasi. Build kandidat selanjutnya dapat memakai ulang `game.pack` yang sudah ada; keberhasilan build dan hash paket tidak membuktikan folder raw masih tersedia atau seluruh file raw sudah tercadangkan.

Hasil test historis yang dilaporkan saat perbaikan loader: build integration 99 lulus; workspace 643 lulus, 1 gagal (`test_windows_batch_wizard_contract`, dilaporkan pre-existing), 2 ignored. Angka tersebut bukan fresh test run untuk keadaan repository sekarang. Penulisan dokumentasi ini tidak menjalankan build, test executable, cleanup, atau perubahan source game.
