# Laporan teknis: integrasi Kirikiri 2 / Kirikiri Z ke APK Android mandiri

**Proyek:** RuntimePack — Rust CLI host, Kotlin Android shell, dan native engine runtime  
**Tanggal penelusuran:** 27 September 2026  
**Sasaran:** APK satu gim yang membuka judul gim secara langsung tanpa launcher maupun pemilih berkas.  
**Batas verifikasi:** Temuan berasal dari pemeriksaan sumber repositori dan dokumentasi resmi. Belum dilakukan clean build Android atau pengujian gim nyata dalam penelitian ini.

## 1. Keputusan arsitektur

**Kandidat basis utama: [2468785842/krkr2](https://github.com/2468785842/krkr2), mula-mula pin pada commit `dca7264572a63e753c1b07ca7053513d1751ce70`.** Fork ini mempertahankan jalur Kirikiroid2/Cocos/TVP, mencantumkan NDK r28, CMake 3.31.1, AGP 8.7.3, JDK 17, C++17, serta target ARM64. Sumber `windowEx`, `scriptsEx`, `fstat` dan sejumlah plugin lain ada. Dua gerbang sebelum distribusi: clean build semua biner dari sumber yang dapat ditelusuri dan pemeriksaan lisensi per berkas/dependensi. Fork ini pada fase terdahulu pernah mengemas `.so` hasil ekstraksi dari APK 1.3.9; hal tersebut tidak membuktikan build kode sekarang lengkap. [S1][S2]

~~~mermaid
flowchart TD
    A["Rust: audit XP3 dan manifest"] --> B["APK: aset asli + runtime ARM64"]
    B --> C["Bootstrap: siapkan filesDir/game"]
    C --> D["TVP startupFrom(entry)"]
    D --> E["Layar judul"]
    D --> F["Simpanan: filesDir/savedata"]
~~~

**Implementasi awal:** host Rust mengaudit aset tanpa mengubahnya, mengemas XP3 dan berkas lepas sebagai byte asli, lalu bootstrap menyalin berkas paket secara identik ke `filesDir/game` sebelum native TVP berjalan. Simpanan dan log berada di lokasi terpisah. Metode ini memberi seek berkas biasa kepada TVP dan FFmpeg. Optimasi tanpa salinan memerlukan seekable asset/VFS adapter; itu pekerjaan lanjutan.

## 2. Domain A — lineage, repositori, dan status build

| Repositori | Susunan dan kapasitas | Penilaian untuk APK satu gim |
|---|---|---|
| [zeas2/Kirikiroid2](https://github.com/zeas2/Kirikiroid2) | `src/core` berisi TVP, `src/plugins` plugin internal, `project/android` Activity/JNI, Cocos2d-x untuk UI. Video mengadaptasi kode Kodi. | Referensi jalur boot asli. Build Android lama memakai `gnustl_static`; kode publik kehilangan beberapa plugin yang diasosiasikan dengan APK rilis. [S3][S2] |
| [YuriSizuku/Kirikiroid2Yuri](https://github.com/YuriSizuku/Kirikiroid2Yuri) | Fork Android perantara, NDK `25.2.9519653`, script `cross_android64.sh` dan Gradle. | README sendiri mencatat bug besar build beta dari sumber; berguna sebagai perbandingan, belum bisa dianggap baseline produksi. [S4] |
| **[2468785842/krkr2](https://github.com/2468785842/krkr2)** | `cpp/core`, `cpp/plugins`, `platforms/android`. Android menghasilkan `libkrkr2.so`. | Titik awal terbaik bagi profil kompatibilitas ARM64, bersyarat clean build, pengujian gim, dan audit lisensi. [S1] |
| [krkrsdl2/krkrsdl2](https://github.com/krkrsdl2/krkrsdl2) | Port SDL2 Kirikiri 2/Z dengan proyek Android. | Pengelola menyatakan gim komersial tanpa modifikasi **tidak didukung**. Berguna bagi gim yang sumbernya bisa disesuaikan. Konfigurasi Gradle Android yang diperiksa masih lama. [S5] |
| [krkrsdl3/krkrsdl3](https://github.com/krkrsdl3/krkrsdl3) dan [krkrz/krkrz](https://github.com/krkrz/krkrz) | Port SDL3 yang berkembang dan sumber utama Kirikiri Z. | Pantau sebagai sumber perubahan engine. Kirikiri Z memisahkan plugin/submodul dan bukan paket Android Kirikiroid2 siap pakai. [S6] |

[Kirikiroid2_patch](https://github.com/zeas2/Kirikiroid2_patch) adalah koleksi patch per gim, bukan engine. Tidak ditemukan basis build yang terverifikasi setara untuk nama `weolar` atau `kirikiroid2-standalone`. Penamaan repositori komunitas tidak membuktikan kemampuan menjalankan gim. [S7]

### Manifes kompatibilitas per gim

Catat: jalur `entry`, urutan arsip dan patch, filter XP3, nama plugin **beserta metode yang digunakan**, codec video/audio, lokasi save, serta hasil pengujian runtime. Status `TOC terbaca`, `engine terkompilasi`, dan `gim mencapai judul` harus terpisah.

## 3. Domain B — direct boot tanpa file browser

### Call chain yang ditemukan pada fork terpilih

1. [`MainActivity.kt`](https://github.com/2468785842/krkr2/blob/main/platforms/android/app/java/org/github/krkr2/MainActivity.kt) mewarisi `KR2Activity`.
2. [`KR2Activity.java`](https://github.com/2468785842/krkr2/blob/main/platforms/android/app/java/org/tvp/kirikiri2/KR2Activity.java) mewarisi `Cocos2dxActivity`, membuat `Cocos2dxGLSurfaceView`, lalu meneruskan event input.
3. [`AppDelegate.cpp`](https://github.com/2468785842/krkr2/blob/main/cpp/core/environ/cocos2d/AppDelegate.cpp) membuat `TVPMainScene` dan menjadwalkan `TVPCheckStartupArg()`. Hasil `false` memunculkan `TVPMainFileSelectorForm`.
4. Pada Android, [`TVPCheckStartupArg()`](https://github.com/2468785842/krkr2/blob/main/cpp/core/environ/android/AndroidUtils.cpp) mendaftarkan event dispatcher dan pada kode semula mengembalikan `false`.
5. Pemilih biasa memanggil [`TVPMainScene::startupFrom(path)`](https://github.com/2468785842/krkr2/blob/main/cpp/core/environ/cocos2d/MainScene.cpp), lalu `doStartup()`, lalu `Application->StartApplication(path)`. Pada tahap terakhir TVP menetapkan project storage, inisialisasi TJS, memuat plugin internal dan `startup.tjs`. [S1][S3]

**Titik patch utama adalah native `TVPCheckStartupArg()` serta cabang fallback `AppDelegate`.** Nama `TVPInitEngine` tidak terbukti tersedia sebagai API JNI untuk memilih gim. Opsi port `-startup` hanya menentukan nama skrip awal; `-datapath` mengatur lokasi data tersimpan/log. Opsi desktop `-nosel` untuk tes, bukan API direct boot APK. [S8]

### Langkah perubahan

**1. Tetapkan entry saat build.** Jika ada `game/startup.tjs` lepas, entry adalah direktori `filesDir/game`. Jika skrip startup berada di XP3, pilih arsip yang benar, misalnya `filesDir/game/data.xp3`. File selector pada fork menerima direktori dengan startup script atau arsip yang valid. Gim dengan banyak XP3/patch memerlukan verifikasi urutan pemuatan.

**2. Stage aset sebelum engine activity.** Bootstrap membaca manifest aset yang dihasilkan Rust host; salin dalam buffer kecil, hitung SHA-256 dan ukuran, tulis ke nama sementara, lalu atomic rename. Jika hasil valid sudah ada, lewati penyalinan. Masuk ke engine activity sesudah semua aset siap. Gunakan `filesDir` tanpa izin penyimpanan umum; hapus permintaan `MANAGE_EXTERNAL_STORAGE` dan izin eksternal lama dari APK khusus satu gim. [S10]

**3. Ganti jalur startup Android.** Setelah kode pendaftaran event dispatcher yang sudah ada, ubah hasil fungsi dalam `AndroidUtils.cpp`:

~~~cpp
// Tambahkan include MainScene.h pada AndroidUtils.cpp.
// Setelah registrasi event dispatcher:
const std::string entry =
    GetInternalStoragePath() + "/game" + RUNTIMEPACK_ENTRY_SUFFIX;
return TVPMainScene::GetInstance()->startupFrom(entry);
~~~

Host menghasilkan `RUNTIMEPACK_ENTRY_SUFFIX = ""` bagi direktori atau `"/data.xp3"` bagi arsip. Ini menunjukkan lokasi dan logika patch, **bukan diff yang sudah dikompilasi**. Pastikan instance Activity sudah tersedia saat helper JNI `GetInternalStoragePath()` dipanggil; jika lifecycle fork berubah, kirim path lewat JNI sebelum Cocos dijalankan.

**4. Hapus fallback picker.** Ubah cabang `if (!TVPCheckStartupArg())` di `AppDelegate.cpp` agar menampilkan galat paket dan keluar dari activity, bukan memanggil `TVPMainFileSelectorForm::create()`. Jika tidak, game yang gagal boot tetap memperlihatkan browser.

**5. Pisahkan penulisan.** `TVPCheckStartupPath()` memeriksa apakah parent path dapat ditulis dan membuat `savedata` di situ. Ubah path data/simpan ke `filesDir/savedata` secara konsisten untuk entry direktori dan entry XP3. Periksa skrip yang menulis langsung di samping file gim dan file konfigurasi yang dapat menimpa `-datapath`. Audit pencarian autoload `.tpm` karena lokasi plugin diturunkan dari `TVPNativeProjectDir` dan dapat berbeda saat entry berupa arsip versus direktori.

### Opsi tanpa salinan aset

Android `AssetManager.openFd()` memberi descriptor dengan offset APK **hanya untuk aset tak terkompresi**. Bila XP3 disimpan sebagai ZIP `STORE`, implementasi bisa memasok `fd`, offset awal aset, panjang aset, serta operasi `readAt/seek` ke lapisan TVP. Berkas lepas, filter XP3, penulisan save, dan FFmpeg custom IO juga perlu jalur stream yang sesuai. Memasukkan path URI asset ke `startupFrom()` saja tidak mencukupi. [S9]

## 4. Domain C — spesifikasi XP3 untuk auditor Rust

Spesifikasi bersumber dari [pembaca XP3 Kirikiri Z](https://github.com/krkrz/krkrz/blob/master/base/XP3Archive.cpp) dan [konstanta header-nya](https://github.com/krkrz/krkrz/blob/master/base/XP3Archive.h). Semua field multi-byte adalah **little-endian**. `base` adalah posisi awal XP3; untuk arsip biasa `base = 0`. Pada XP3 yang ditempel di executable `MZ`, pembaca asli mencari signature pada batas 16 byte, lalu seluruh offset XP3 tetap relatif terhadap `base`. [S11]

### Header 11-byte

| Offset dari `base` | Ukuran | Makna |
|---|---:|---|
| `0x00` | 11 | Signature penuh: `58 50 33 0D 0A 20 0A 1A 8B 67 01` atau `b"XP3\r\n \n\x1a\x8b\x67\x01"`. Delapan byte pembuka saja tidak cukup. |
| `0x0B` | 8 | `u64 index_offset`, relatif terhadap `base`. |
| `base + index_offset` | variabel | Blok indeks pertama. |

### Blok indeks

| Posisi dalam blok | Ukuran | Makna |
|---|---:|---|
| `+0` | 1 | `flags`; `flags & 0x07`: metode `0` mentah, `1` zlib; bit `0x80`: ada indeks lanjutan. |
| `+1` bila mentah | 8 | `u64 index_size` diikuti `index_size` byte indeks. |
| `+1` bila zlib | 16 | `u64 compressed_size`, `u64 uncompressed_size` diikuti `compressed_size` byte **zlib stream**. Pembaca upstream menggunakan `uncompress()`, bukan raw deflate. |
| Tepat setelah payload blok, bila `0x80` aktif | 8 | `u64 next_index_offset` relatif terhadap `base`; ulangi hingga flag kelanjutan kosong. |

### Chunk di indeks yang telah didekompresi

Setiap chunk: `tag ASCII[4] + payload_length:u64 + payload[payload_length]`. Tag tingkat atas yang memuat file bernama `File`. Payload `File` memuat chunk bersarang `info`, `segm`, `adlr`. Tag yang tidak dikenal dapat dilewati berdasarkan panjangnya setelah diperiksa batasnya. [S11]

| Tag dan offset relatif payload | Tipe | Keterangan |
|---|---|---|
| `info + 0` | `u32` | flags; bit `1 << 31`: protected. |
| `info + 4` | `u64` | ukuran berkas asli. |
| `info + 12` | `u64` | ukuran berkas di arsip. |
| `info + 20` | `u16` | panjang nama dalam **code unit UTF-16**. |
| `info + 22` | `u16[filename_units]` | path UTF-16LE, tepat `filename_units * 2` byte. |
| `segm + 28*i + 0` | `u32` | flags segment `i`; `flags & 7`: raw `0`, zlib `1`. |
| `segm + 28*i + 4` | `u64` | offset data dari `base`. Segment boleh berbagi rentang penyimpanan. |
| `segm + 28*i + 12` | `u64` | panjang segment sebelum kompresi. |
| `segm + 28*i + 20` | `u64` | panjang segment tersimpan. Ukuran tiap record **28 byte**. |
| `adlr + 0` | `u32` | nilai Adler/hash yang diteruskan pula ke konteks extraction filter; bukan bukti isi tak terenkripsi. |

### Algoritme auditor read-only

1. `mmap` arsip secara read-only atau pakai positional reads; jangan ekstrak seluruh isinya. Hanya indeks zlib yang didekompresi ke RAM, dengan limit.
2. Cocokkan signature lengkap. Deteksi `MZ` dan pindai offset selaras 16 byte hanya bila mode arsip tertempel dinyalakan; batasi ukuran pencarian.
3. Baca pointer dan field dengan `u64::from_le_bytes`, `checked_add`, konversi `usize` yang divalidasi, serta `slice.get(range)`; data arsip tidak boleh langsung menjadi indeks memori.
4. Ikuti rantai indeks dengan `visited_offsets`, batas jumlah blok, limit ukuran terkompresi dan terdekompresi, limit jumlah entri, serta rasio dekompresi. Ambil pointer lanjutan dari **akhir payload yang disimpan**, bukan dari buffer hasil dekompresi.
5. Iterasi chunk di dalam batas masing-masing induk. Wajibkan keberadaan `info`, `segm` dan `adlr` bila entri akan diklasifikasikan sebagai XP3 standar.
6. Validasi UTF-16 dan normalisasi nama. Tandai path absolut, `..`, NUL, separator aneh, duplikat sesudah normalisasi, dan nama yang tidak dapat didekode.
7. Uji `base + data_offset + archived_size <= file_size` pada tiap segment. Bandingkan jumlah ukuran segment asli dengan `info.original_size`. Tandai metode kompresi tak dikenal dan nilai ukuran mencurigakan.
8. Keluarkan manifest nama, flag, ukuran, dan rentang byte. Pemeriksaan header PNG/Ogg/video sampel dapat dilakukan sebagai langkah terbatas yang terpisah; audit TOC **tidak** mensyaratkan membaca seluruh payload.

Pola helper Rust untuk pembacaan aman:

~~~rust
fn slice_at(data: &[u8], off: u64, len: u64) -> Option<&[u8]> {
    let start = usize::try_from(off).ok()?;
    let n = usize::try_from(len).ok()?;
    data.get(start..start.checked_add(n)?)
}

fn le_u64(data: &[u8], off: u64) -> Option<u64> {
    let bytes: [u8; 8] = slice_at(data, off, 8)?.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}
~~~

**Enkripsi khusus:** bit protected hanyalah penanda. `XP3ArchiveExtractionFilter` dapat mengubah buffer setelah pembacaan/dekompresi segment berdasarkan offset, panjang, nama, dan hash file. Ada gim yang mempertahankan TOC standar tetapi mengenkripsi isi; ada pula yang mengubah header/index. XOR adalah salah satu kemungkinan, bukan aturan. Keluaran auditor sebaiknya `standard-readable`, `protected-or-filtered-suspected`, atau `unknown-layout`, dengan kebutuhan plugin/filter yang dicatat sebagai dependensi kompatibilitas. [S11]

## 5. Domain D — shim plugin Windows di ARM64

Berkas PE Windows `.dll` atau `.tpm` tidak dapat dieksekusi di Android ARM64 dengan mengganti ekstensi. `Plugins.link("name.dll")` dialihkan ke `TVPLoadInternalPlugin()` lalu registrasi `ncbAutoRegister`. Pada port asli, jalur loader Windows dinamis dinonaktifkan. Fork modern mencatat lookup plugin yang gagal dan mengubah nama `.tpm` menjadi pendaftaran `.dll` bila sesuai. Plugin bawaan dikompilasi ke pustaka statis yang masuk ke library Android, bukan dimuat sebagai PE PC. Library utama original bernama `libgame.so`; fork terpilih menghasilkan `libkrkr2.so`.

| Kategori | Bukti sumber | Status awal |
|---|---|---|
| `csvParser`, `dirlist`, `fftgraph`, `addFont`, `getSample`, `getabout`, `saveStruct`, `varfile`, `wutcwf`, `win32dialog`, `xp3filter` | Berkas dan registrasi tersedia. | Kandidat tersedia; uji metode spesifik yang diminta gim. |
| `layerExMovie`, `perspective`/`layerExPerspective` | Kode port tersedia; nama modul dapat berbeda dari nama file C++. | Periksa alias pendaftaran dan API. |
| `scriptsEx`, `windowEx`, `fstat`, `TextRender`, `layerExDraw`, `motionplayer`, `emoteplayer` | Disertakan pada fork modern dan CMake. | Kandidat khusus fork modern; keberadaan kode bukan bukti kompatibilitas penuh. |
| `KAGParserEx` | File ada, callback registrasi yang tampak kosong. | Belum boleh dianggap implementasi fungsional. |
| `psbfile`/PSB | Proyek ada, plugin masih memiliki TODO pada sumber. | Parsial/eksperimental. |
| `wuvorbis`, `extNag`/`extNagano`, `DrawDeviceD3D`, plugin penerbit | Tidak ada pemuatan universal DLL Windows. | Tidak diketahui/tidak didukung hingga API port tersedia. |

Rujukan kode: [`cpp/plugins/CMakeLists.txt`](https://github.com/2468785842/krkr2/blob/main/cpp/plugins/CMakeLists.txt), [`PluginImpl.cpp`](https://github.com/2468785842/krkr2/blob/main/cpp/core/plugin/PluginImpl.cpp), serta berkas plugin per modul. Host harus memindai nama `.dll/.tpm` dan panggilan literal `Plugins.link(...)` lalu mencocokkan **nama dan metode** dengan capability manifest runtime. Nama yang dihitung dinamis atau skrip terenkripsi diberi status `unknown`. Kegagalan lookup yang hanya dicatat di log tidak cukup sebagai kriteria sukses build.

## 6. Domain E — grafik, video, audio, input, overlay

### Grafis dan video

Fork terpilih memakai `Cocos2dxGLSurfaceView`/EGL dengan OpenGL ES 2.0 sebagai minimum manifest, bukan renderer SDL2. Jalur movie TVP berisi demux/decode FFmpeg turunan Kodi dan presentasi frame melalui GL. Kelas `Cocos2dxVideoView` berbasis Android `MediaPlayer` memang ada di dependensi Cocos, tetapi itu tidak membuktikan movie TVP menggunakan MediaPlayer. Audio fork modern memuat renderer Oboe. [S1][S3]

| Jenis | Pemeriksaan codec yang perlu dilakukan |
|---|---|
| `.mp4` | MP4 demux, H.264 atau codec video aktual, audio, seek, sinkronisasi. |
| `.wmv` | ASF/WMV demux, WMV/VC-1 atau codec aktual dan audio pendamping. |
| `.mpg` | MPEG-PS serta codec video/audio yang disertakan dalam build FFmpeg. |
| `.ogv` | Ogg demux serta Theora/Vorbis sesuai stream aktual. |

Ekstensi tidak menjamin keberhasilan. Catat codec dari aset gim dan konfigurasi FFmpeg yang benar-benar didistribusikan. Untuk video di XP3 langsung dari APK, custom stream wajib mendukung seek dan pola baca decoder.

### Touch dan kontrol virtual

`KR2Activity` meneruskan pointer ID, `DOWN/MOVE/UP/CANCEL`, hover, tombol Back/Menu, dan roda tetikus fisik ke JNI/Cocos. Di [`TVPWindow.cpp`](https://github.com/2468785842/krkr2/blob/main/cpp/core/environ/impl/TVPWindow.cpp), satu sentuhan menjadi tombol kiri, dua sentuhan tombol kanan, tiga sentuhan tombol tengah; gerak dapat menjadi drag. Mouse-wheel diteruskan ke callback TVP yang dapat dipakai gim untuk backlog. Sapuan dua jari **tidak otomatis terjamin** menjadi mouse-wheel/backlog pada semua gim.

Overlay Android atau WebView transparan dapat ditaruh di atas GL surface. Hanya area kontrol interaktif yang boleh mengonsumsi touch; bagian lain harus melewatkan event dan pointer ID dengan koordinat yang tepat. Ketika overlay mengambil alih sentuhan, kirim `CANCEL` jika perlu. Input sintetis ke TVP diantrikan ke thread event native; uji urutan surface, fokus, IME, serta sentuhan jamak di perangkat. Kontrol native Android adalah implementasi paling kecil; WebView masuk akal untuk UI kaya setelah event routing stabil.

## 7. Domain F — lisensi dan provenance

| Komponen | Temuan primer | Implikasi untuk APK |
|---|---|---|
| Kirikiri 2 **dan KAG3** | W.Dee menerbitkan dual license: pilihan lisensi khusus Kirikiri atau GNU GPL yang disertakan. Lisensi khusus memuat ketentuan sendiri tentang redistribusi, atribusi, dan perubahan. [S12] | Jangan menyederhanakan semua kode menjadi “GPLv2 saja” ataupun “BSD”. Tentukan pilihan dan cakupan berkas yang dipakai. |
| Kirikiri Z | Root `LICENSE` dimulai dengan ketentuan bergaya BSD tiga klausul dan diikuti notice komponen lain. [S6] | Sertakan teks lisensi/notice yang cocok bagi sumber dan submodul yang benar-benar digunakan. |
| Kirikiroid2 dan fork modern | Root `LICENSE` adalah kompilasi notice. README fork modern menyebut MIT, tetapi root license dimulai dengan notice Kirikiri Z. Contoh [`RenderFormats.h`](https://github.com/2468785842/krkr2/blob/main/cpp/core/movie/ffmpeg/RenderFormats.h) dari Kodi secara eksplisit mencantumkan GPL 2 atau lebih baru. [S1][S3] | Klaim seluruh APK berlisensi MIT belum didukung. Audit lisensi per file yang dikompilasi, termasuk kode movie dan port plugin. |
| FFmpeg | Default LGPL 2.1 atau lebih baru; bagian opsional GPL membuat build FFmpeg tunduk pada GPL. [S13] | Simpan konfigurasi build, hash library, dan sumber/notice sesuai konfigurasi tersebut. |
| krkrsdl2 | Maintainer menyatakan bagian `src` MIT, komponen pihak ketiga mengikuti lisensi masing-masing. [S5] | Alternatif bila batas kompatibilitas gimnya sesuai. |

Gerbang distribusi teknis:

1. Pin commit engine, NDK, Gradle, vcpkg, submodul, dan dependensi; hasilkan SBOM serta pemetaan source-to-binary untuk setiap `.so`.
2. Hindari `.so` dari ekstraksi APK lama atau dependency yang provenance/izinnya belum jelas; buktikan clean build.
3. Periksa secara spesifik file Kodi yang terkompilasi, konfigurasi FFmpeg, plugin pihak ketiga, dan kewajiban source/notice yang berlaku.
4. Assess gabungan Kotlin shell–JNI–engine yang benar-benar ada di satu APK. Menaruh engine di `.so` terpisah **tidak otomatis meniadakan** kewajiban copyleft. Rust CLI host yang dieksekusi sebagai alat build terpisah dapat dinilai tersendiri.
5. Pastikan izin mendistribusikan konten aset/skrip masing-masing gim bagi APK yang diterbitkan.

## 8. Matriks toolchain, Android 15, dan rencana verifikasi

| Basis | NDK, build, standar C++ | Pekerjaan RuntimePack |
|---|---|---|
| Zeas | `ndk-build`, Clang, C++11, `gnustl_static`, konfigurasi Android lama. | Migrasi STL/toolchain dan dependensi. |
| Yuri | NDK `25.2.9519653`, CMake/Gradle, C++11/14. [S4] | Buktikan source build dan kompatibilitas 16 KB; perbaiki bug beta. |
| Fork 2468785842 | NDK `28.0.13004108`, CMake `3.31.1`, AGP `8.7.3`, JDK 17, C++17, `c++_shared`, ARM64/x86-64. [S1] | Rebuild dan audit semua `.so`, termasuk dependensi vcpkg/FFmpeg/Cocos. |
| krkrsdl2 | CMake C++14 dan proyek Android Gradle lama. [S5] | Modernisasi build Android; validasi gim di dalam ruang lingkup SDL2 port. |

Pada Android berhalaman memori **16 KB**, NDK r28+ secara default menghasilkan alignment ELF yang kompatibel. NDK r27 dan lebih tua membutuhkan opsi linker `-Wl,-z,max-page-size=16384` dan `-Wl,-z,common-page-size=16384` bila ingin membangun library beralignment 16 KB. AGP 8.5.1+ direkomendasikan untuk pengemasan uncompressed `.so`. Periksa **setiap** library transitif, bukan hanya engine utama. [S14]

~~~bash
# LOAD p_align masing-masing .so setidaknya 2**14.
llvm-readelf -Wl libkrkr2.so

# Alignment entri pustaka di APK.
zipalign -c -P 16 -v 4 game.apk

# Pengujian pada emulator/perangkat halaman 16 KB.
adb shell getconf PAGE_SIZE
# Keluaran yang diharapkan: 16384.
~~~

**Gerbang profil awal:** clean build ARM64 dari commit yang dipin; satu gim XP3/KAG standar mencapai layar judul tanpa picker; input kiri/kanan, save/load, serta video sampel bekerja; APK hidup pada perangkat 16 KB. Gim dengan filter arsip atau plugin tambahan masuk sebagai profil terpisah setelah API yang dibutuhkan diuji.

## Rujukan primer

- **[S1]** [2468785842/krkr2, README dan sumber](https://github.com/2468785842/krkr2); [Gradle Android](https://github.com/2468785842/krkr2/blob/main/platforms/android/app/build.gradle); [LICENSE](https://github.com/2468785842/krkr2/blob/main/LICENSE).
- **[S2]** [Catatan maintainer tentang .so 1.3.9 dan plugin yang tidak lengkap](https://github.com/2468785842/krkr2/issues/1).
- **[S3]** [zeas2/Kirikiroid2](https://github.com/zeas2/Kirikiroid2); [README](https://github.com/zeas2/Kirikiroid2/blob/master/readme.md); [LICENSE](https://github.com/zeas2/Kirikiroid2/blob/master/LICENSE).
- **[S4]** [YuriSizuku/Kirikiroid2Yuri](https://github.com/YuriSizuku/Kirikiroid2Yuri).
- **[S5]** [krkrsdl2 README dan batas kemampuan](https://github.com/krkrsdl2/krkrsdl2/blob/main/README.md); [Android project](https://github.com/krkrsdl2/krkrsdl2/tree/main/android-project).
- **[S6]** [krkrz/krkrz dan pemisahan plugin](https://github.com/krkrz/krkrz); [LICENSE](https://github.com/krkrz/krkrz/blob/master/LICENSE); [krkrsdl3](https://github.com/krkrsdl3/krkrsdl3).
- **[S7]** [zeas2/Kirikiroid2_patch](https://github.com/zeas2/Kirikiroid2_patch).
- **[S8]** [Dokumentasi command-line Kirikiri 2](https://krkrz.github.io/krkr2doc/kr2doc/contents/CommandLine.html).
- **[S9]** [Android AssetManager, syarat openFd](https://developer.android.com/reference/android/content/res/AssetManager).
- **[S10]** [Android app-specific storage](https://developer.android.com/training/data-storage/app-specific).
- **[S11]** [XP3Archive.cpp Kirikiri Z](https://github.com/krkrz/krkrz/blob/master/base/XP3Archive.cpp); [XP3Archive.h](https://github.com/krkrz/krkrz/blob/master/base/XP3Archive.h).
- **[S12]** [Lisensi Kirikiri 2/KAG3](https://krkrz.github.io/krkr2doc/); [FAQ hak cipta](https://krkrz.github.io/krkr2doc/kr2doc/contents/Copyrights.html).
- **[S13]** [Lisensi FFmpeg](https://www.ffmpeg.org/legal.html).
- **[S14]** [Android Developers: 16 KB page size](https://developer.android.com/guide/practices/page-sizes).
