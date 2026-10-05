# Laporan Hasil Investigasi Teknis: Anomali Audit & Rendering Black Souls 2 (mkxp-z Android)

**Tanggal:** 25 September 2026  
**Subjek Uji:** *BLACK SOULS 2* (RPG Maker VX Ace / RGSS3 on mkxp-z Android)  
**Status:** Temuan Investigasi Sub-Agent (Analisis Akar Masalah & Rekomendasi Deteksi)

---

## 1. Ringkasan Eksekutif

Berdasarkan pengujian langsung build APK *Black Souls 2* dengan mode `--flavor beta`, ditemukan tiga anomali utama:
1. **Laporan Audit Palsu (`Valid: 996 | Missing: 0`):** Console audit menampilkan file biner internal (`rp_input_state.bin`), skrip Ruby (`runtimepack_asset_audit.rb`), dan dump skrip teks (`script_165_Main.txt`) dengan status `Decode Succeeded`.
2. **Mojibake / Huruf Kotak-Kotak (`□`):** Karakter tanda petik Jepang (`「`, `」`), titik dua fullwidth (`：`), serta huruf Hiragana/Katakana berubah menjadi kotak tahu kosong (`□`), sedangkan huruf Latin alfabet tetap terbaca normal.
3. **Layar Map Hitam Pekat (Black Screen) Tetapi Sprite & UI Normal:** Area lantai dan dinding perpustakaan Alice menjadi hitam legam, namun pilar, sprite Alice, lilin, pemain, dan jendela dialog tetap tampil utuh di posisinya.

---

## 2. Investigasi Anomali 1: Kenapa Probe Melaporkan File `.bin`, `.rb`, `.txt` sebagai "Decode Succeeded"?

### A. Akar Masalah
1. **File `asset-dependency-graph.json` Tidak Masuk ke APK:**  
   Pada `cli/src/main.rs:970-974`, file grafik dependensi aset hanya ditulis ke folder output build di PC host (`dist/black_souls/`). Di `core/src/build_plan/generator.rs:476-586`, pipeline packaging Android **tidak pernah menyalin** file `asset-dependency-graph.json` ke dalam `src/main/assets/` APK.
2. **Fallback Memindai Seluruh Storage Game (`gameDir.walkTopDown()`):**  
   Di `android/shell/app/src/main/java/io/runtimepack/host/AssetProbeRunner.kt:346-365`, probe di perangkat mencari 8 kandidat path file graph. Karena tidak ada yang ditemukan, eksekusi jatuh ke baris 367 (`// Fallback: scan gameDir recursively`).
3. **Filter Pengecualian Tidak Memeriksa Ekstensi Media:**  
   Fungsi `isExcludedPath` (`AssetProbeRunner.kt:394-402`) hanya memfilter folder `.git`, `save`, dan file `.tmp`. File biner Win32 input (`rp_input_state.bin`), skrip injeksi Ruby (`runtimepack_asset_audit.rb`), dan file dump teks (`script_165_Main.txt`) lolos sebagai kandidat aset.
4. **Bypass Decoder Bitmap Menghasilkan `DECODE_SUCCESS` Palsu:**  
   Pada `AssetProbeRunner.kt:183-209`, hanya ekstensi gambar (`isImageExtension`) yang dikirim ke `BitmapProbeDecoder`. File selain gambar masuk ke blok `else` yang hanya mengecek `fis.read(buf) > 0`:
   ```kotlin
   // For audio/font/video/data, verify that stream is readable
   val buf = ByteArray(128)
   val read = fis.read(buf)
   read > 0
   ```
   Karena file `.bin`, `.rb`, dan `.txt` berisi data (> 0 byte), nilai `decodeOk` menjadi `true`. Probe menandainya dengan `RuntimeAuditStage.DECODE_SUCCESS` dan `RuntimeAuditOutcome.PASS`, yang di UI JavaScript dipetakan menjadi teks `"Decode Succeeded"`.

### B. Bukti Source Code
* `cli/src/main.rs:970-974`: Penulisan artefak audit hanya ke direktori lokal host.
* `core/src/build_plan/generator.rs:476-586`: `src/main/assets` tidak memuat `asset-dependency-graph.json`.
* `AssetProbeRunner.kt:346-365`: Kegagalan penemuan file grafik di runtime Android.
* `AssetProbeRunner.kt:183-209`: File non-gambar dianggap lolos decode hanya dengan `read() > 0`.
* `diagnostic_suite.js:1036, 1147`: Pemetaan label `DECODE_SUCCESS` -> `"Decode Succeeded"`.

---

## 3. Investigasi Anomali 2: Kenapa Karakter CJK & Tanda Petik Menjadi Kotak-Kotak (`□`)?

### A. Akar Masalah
1. **Game Memanggil Font Windows RTP yang Tidak Dibundel:**  
   Skrip bawaan RPG Maker VX Ace (`Window_Base` baris 87086) memanggil `Font.default_name` (`VL Gothic`), sedangkan skrip nama pembicara Black Souls 2 (`A1_System::NameWindow` baris 271413) memanggil `UmePlus Gothic`. Di PC Windows, font ini disediakan oleh installer RTP Enterbrain (`C:\Windows\Fonts`). Game standalone tidak membundel folder `Fonts/`.
2. **Substitusi Font Menargetkan Font Hantu (`fontSub`):**  
   Pada `GpackExtractor.kt:244-254`, `mkxp.json` mengonfigurasi substitusi font:
   ```kotlin
   "VL Gothic>Noto Sans CJK TC",
   "UmePlus Gothic>Noto Sans CJK TC",
   "MS Gothic>Noto Sans CJK TC"
   ```
   Font `Noto Sans CJK TC` adalah aset internal game *Lona RPG* (`SYS-DEFAULT-NotoSansCJKtc-Black.otf`). Pada game *Black Souls 2* maupun template APK Android standar, file font ini **tidak ada**.
3. **Resolusi Font FreeType di mkxp-z:**  
   `FileSystem::initFontSets` di `libmkxp-z.so` (offset `0x601f58`) hanya mencari font di folder `Fonts/` game via PhysicsFS. mkxp-z **tidak memiliki akses dan tidak membaca** font sistem Android (`/system/fonts/`).
4. **Fallback ke Built-in Font "Liberation Sans" yang Miskin Glyph CJK:**  
   Saat pencarian font gagal, `SharedFontState::openBundled` di `libmkxp-z.so` (offset `0x739435`) memuat font bawaan: **Liberation Sans**.
5. **Bedah Tabel OpenType `cmap` Liberation Sans:**  
   Pemeriksaan biner terhadap tabel `cmap` (Format 4) font internal `Liberation Sans`:

   | Karakter | Unicode Codepoint | Keterangan | Status di `Liberation Sans` | Hasil Render |
   |:---|:---|:---|:---:|:---:|
   | `A` | `U+0041` | Huruf Latin Alfabet | **PRESENT = True** | Normal |
   | `「` | `U+300C` | Tanda Petik Buka Jepang | **PRESENT = False** | `.notdef` (`□`) |
   | `」` | `U+300D` | Tanda Petik Tutup Jepang | **PRESENT = False** | `.notdef` (`□`) |
   | `あ` | `U+3042` | Huruf Hiragana A | **PRESENT = False** | `.notdef` (`□`) |
   | `ア` | `U+30A2` | Huruf Katakana A | **PRESENT = False** | `.notdef` (`□`) |
   | `：` | `U+FF1A` | Fullwidth Colon | **PRESENT = False** | `.notdef` (`□`) |

   Saat karakter CJK dipanggil, FreeType mengembalikan Glyph ID 0 (`.notdef`), yang dirender sebagai kotak kosong (`□`). Huruf alfabet Latin hasil terjemahan tetap normal karena tercakup dalam karakter Pan-European Liberation Sans.

---

## 4. Investigasi Anomali 3: Kenapa Map Hitam Pekat Tetapi Pilar, Alice, dan UI Muncul?

### A. Akar Masalah
Visual hitam pekat terjadi akibat perpaduan dua faktor: **hilangnya tileset lantai RTP** dan **layer kabut subtractive (occlusion)**.

#### 1. Lantai dan Tembok Berasal dari Tileset RTP yang Hilang
Pada `Data/Map101.rvdata2` (Perpustakaan Alice, Tileset #3 `内装`):
* **Layer 0 (Lantai):** Memakai autotile `Inside_A2` (429 tile) -> **MISSING** (Aset RTP Windows).
* **Layer 0 (Dinding):** Memakai autotile `Inside_A4` (317 tile) -> **MISSING** (Aset RTP Windows).
* **Layer 0 (Props Dasar):** Memakai `Inside_A5` (290 tile) -> **MISSING** (Aset RTP Windows).
* **Layer 1 (Karpet/Dekorasi):** Memakai `Inside_A2` (257 tile) -> **MISSING** (Aset RTP Windows).
* **Layer 2 (Perabot):** Memakai `Inside_B` (147 tile) -> **MISSING** (Aset RTP Windows).

Saat engine meminta tileset tersebut, auto-recovery RuntimePack (`runtimepack_bootstrap.rb:703-726`) mencegat error missing file dan menyuplai `Bitmap.new(32, 32)` transparan agar game tidak force close. Akibatnya, **seluruh lantai dan tembok digambar 100% transparan**, memperlihatkan warna dasar layar kosong (**hitam pekat**).

#### 2. Kenapa Pilar, Alice, Lilin, dan UI Tetap Muncul?
* **Pilar (Layer 2):** Menggunakan tile ID 256..511 yang berasal dari `Inside_C.png`. File `Graphics/Tilesets/Inside_C.png` adalah aset kustom yang **ada secara fisik di folder game**. Tekstur ini valid, sehingga pilar dirender normal.
* **Alice, Pemain, & Lilin:** Merupakan objek `Sprite_Character` event (`$c_54b`, `$game_player`, `!Flame.png`) dari folder `Graphics/Characters/` yang filenya lengkap. Di RGSS, karakter memiliki koordinat `z = y * 32 + 32` (`z >= 32` s/d `z = 500+`), berada jauh di atas layer map (`z = 0`).
* **Jendela UI:** Berada pada `@viewport3` (`z = 100`) menggunakan `Graphics/System/Window.png` yang valid.

#### 3. Lapisan Kabut Kegelapan BMSP Map Fog (`kurayami01.png`)
Skrip 151 (`hulogu` / BMSP Map Fog) pada Map 101 memiliki tag note:
`==マップフォグkurayami01[0,0,10,170,2]==`
* Skrip ini membuat `Plane.new` satu layar penuh dengan `z = 10`, `opacity = 170`, dan `blend_type = 2` (Subtractive/Pengurangan).
* File `Graphics/Parallaxes/kurayami01.png` memiliki ukuran 640x480 RGBA dengan alpha penuh (255): bagian tengah berwarna hitam `(0, 0, 0)`, sedangkan sudut berwarna putih `(254, 254, 254)`.
* Di DirectX 9 (PC), subtractive blending mengurangi warna terang untuk menghasilkan efek vignette gelap. Namun di OpenGL ES (Android mkxp-z), operasi pengurangan pada layer lantai yang sudah transparan/hitam (`z = 0`) menjepit pixel ke `(0, 0, 0)`.
* Karena posisinya di `z = 10`, Plane kegelapan ini berada **di atas lantai (`z = 0`)**, tetapi **di bawah karakter, pilar, dan UI (`z >= 32`)**.

---

## 5. Rencana Fitur Deteksi Baru (Kebutuhan Perbaikan Tool)

Agar stress test dan audit diagnostik dapat secara mandiri mengungkap bug ini sebelum game dimainkan, berikut fitur deteksi yang dirancang:

### 1. Perbaikan Packaging & Target Probe (`AssetProbeRunner`)
* **Wajib Bundel File Grafik:** Modifikasi `generator.rs` agar otomatis menyertakan `asset-dependency-graph.json` ke dalam assets APK Android.
* **Filter Ekstensi Probe Fallback:** Jika fallback folder terpaksa aktif, batasi pemindaian hanya pada ekstensi media yang valid (`.png`, `.jpg`, `.ogg`, `.wav`, `.mp3`, `.ttf`, `.otf`). Abaikan file `.bin`, `.rb`, `.txt`, dan `.json`.
* **Koreksi Status Decode:** File teks/biner tidak boleh diberi status `DECODE_SUCCESS`.

### 2. Fitur Audit Font & Glyph Coverage
* **Statik (Build Time):**
  * Ekstraksi nama font default dan font kustom dari `Scripts.rvdata2`.
  * Verifikasi apakah target `fontSub` dan font fisik ada di `Fonts/`.
  * Parsing tabel `cmap` font aktif menggunakan `ttf-parser`. Bandingkan dengan teks dialog dan simbol khusus (`「`, `」`, CJK, fullwidth). Jika Glyph ID = 0, keluarkan error:
    `CMP_AUD_FONT_GLYPH_MISSING` (menyebutkan karakter dan Unicode hex).
* **Runtime (Dalam Game):**
  * Panggil `Font.exist?(font_name)` saat probe berjalan. Jika font gagal di-resolve oleh mkxp-z, catat finding kegagalan fallback Liberation Sans.

### 3. Fitur Deteksi Tileset Hilang & Layar Hitam (Black-Screen Occlusion)
* **Audit Statik Tileset Map:**
  * Periksa tabel tile pada `Map*.rvdata2`. Jika layer lantai (Layer 0) mengandalkan slot tileset (A1–A5) yang filenya tidak ada di disk (RTP missing), beri peringatan:
    `CMP_AUD_MAP_TILES_MISSING_TRANSPARENT_FLOOR` (Layer 0 akan transparan/hitam).
* **Deteksi Bidang Oklusi Subtractive:**
  * Pindai note tag map untuk Plane/Sprite kabut dengan `blend_type == 2` di antara layer map dan karakter (`1 <= z < 32`).
* **Analisis Framebuffer Runtime:**
  * Ambil sampel pixel framebuffer area gameplay saat player berada di map. Jika luminansi area non-karakter bernilai `< 0.02` (hitam pekat) padahal map memiliki tile, beri peringatan:
    `FAIL: RUNTIME_BLACK_MAP_LAYER_DETECTED`.
