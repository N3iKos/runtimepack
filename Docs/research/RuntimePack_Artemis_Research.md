# Riset integrasi Artemis Engine ke RuntimePack

**Tanggal telaah:** 27 September 2026  
**Sasaran:** APK Android khusus satu gim; pasang → ketuk ikon → layar judul, tanpa file browser dan tanpa mengubah byte arsip gim.  
**Cakupan:** analisis format PFS di Rust, pilihan runtime Android, skrip/aset, media, kompatibilitas, dan distribusi.  
**Cara membaca:** *terverifikasi* berarti ditopang kode atau dokumentasi primer tertaut; *inferensi* berarti rancangan RuntimePack yang harus diuji; *belum terverifikasi* berarti jangan dijadikan kontrak kompatibilitas.

## 1. Ringkasan eksekutif dan putusan kelayakan

**Putusan: layak untuk subset gim, belum layak menjanjikan kompatibilitas Artemis menyeluruh.** Parser PFS dan APK yang langsung menjalankan gim dapat dibangun. Hambatan utamanya adalah cakupan instruksi skenario, Lua/native calls khusus judul, shader, video, E-mote, format save, dan hak distribusi aset. Saya merekomendasikan **art3m1s-core** sebagai kandidat runtime pertama untuk prototipe RuntimePack: implementasi ulang Rust berlisensi MPL-2.0, antarmuka C berversi, dan fungsi mount PFS, pengaturan save root, override resource, serta memuat konfigurasi dari byte. Ini *rekomendasi integrasi*, bukan klaim bahwa setiap gim Artemis sudah didukung. Lakukan satu APK uji dan jalur lengkap hingga save/load sebelum mengaktifkan dukungan secara umum. [S01][S02]

| Jalur | Boot langsung arm64 | Keterbukaan/provenans | Biaya integrasi | Putusan |
|---|---|---|---|---|
| **art3m1s-core** | Antarmuka C memungkinkan host memilih proyek tanpa picker; renderer Android GLES/ANGLE | Sumber MPL-2.0, subkomponen dan fitur opsional punya ketentuan sendiri | Sedang; Kotlin/JNI, surface, audio/video host, uji judul | **Pilihan awal** |
| **artemis-compat** | Dapat dibangun sebagai libartemis.so untuk arm64; tersedia NativeActivity/JNI dan CMake core | Sumber GPL-3.0, Lua 5.1.5 MIT; klaim implementasi mandiri dari pengembang | Sedang–tinggi; integrasi host dan kewajiban GPL | Kandidat kedua atau pembanding perilaku |
| **Artroid/Artroid+** | UI file picker bawaan; modifikasi shell memerlukan source dan hak binary | Tidak ditemukan jalur distribusi source/binary engine yang dapat dipastikan lisensinya | Tinggi/terhalang | Referensi pengujian saja |
| **Tyranor-Next dan shell port APK** | Launcher multi-engine; dapat dihilangkan jika ada runtime yang boleh didistribusikan | Wrapper/aktivitas tidak otomatis memberi izin atas binary Artemis yang dibundel | Tinggi; ABI dan legalitas harus diverifikasi | Jangan jadikan binary template sebagai basis produk |
| **Lisensi resmi dari Mikage/iMel** | Artemis resmi memang bisa diintegrasikan sebagai Android Activity | Memerlukan persetujuan tertulis atas SDK, binary, cakupan distribusi dan E-mote | Bergantung kerja sama komersial | Jalur terbaik bila memperoleh lisensi |

Sumber primer untuk masing-masing jalur: [S01][S03][S04][S05][S06][S07][S08].

**Fakta arsitektur:** Artemis Engine berasal dari Mikage, dengan versi mobile komersial yang juga dikaitkan dengan iMel; engine mendukung Windows, Android, iOS, konsol dan WebAssembly menurut situs resmi. Skripnya berbasis tag ala KAG dan memberi akses ke Lua. Dokumentasi resmi yang saya temukan **tidak menetapkan versi Lua VM resmi**, ABI resmi Android, maupun backend grafik Android secara eksplisit. Karena itu jangan menyamakan Lua 5.1 yang digunakan dua implementasi kompatibilitas dengan versi pasti binary komersial. Dokumentasi resmi Windows menyebut DirectX 9/11; port open source memakai GLES atau ANGLE/GLES; konfigurasi PC dan Android dapat memerlukan penyesuaian platform. [S07][S01][S03]

**Batas dukungan awal yang realistis:** PF8 standar, asset PNG/JPEG/OGG, skrip AST/IET/ASB yang dapat dimuat kandidat runtime, save dan tombol sentuh; beri tanda “butuh adaptasi” untuk judul yang mensyaratkan DLL Windows, instruksi ekstensi, E-mote kompleks, shader HLSL yang belum diimplementasi, atau video dengan codec yang tidak tersedia. Tidak ada angka “persen game kompatibel” yang dapat dibuktikan dari sumber.

## 2. Referensi parser arsip PFS untuk Rust

### 2.1 Skema byte dan versi

Parser GARbro adalah bukti pembacaan PF2, PF6, dan PF8; penulis packer PF8 memberi bukti independen untuk footer dan cara membangkitkan kunci. Semua bilangan multi-byte berikut **little endian**. Offset adalah offset absolut berkas kecuali dinyatakan lain. Ejaan magic adalah **ASCII huruf kecil**. Jangan salah mengartikan angka sufiks nama berkas seperti root.pfs.001 sebagai versi format internal: periksa tiga byte pertama setiap volume. [S09][S10]

| Rentang/posisi | PF6 dan PF8 | PF2 | Status |
|---|---|---|---|
| 0x00..0x02 | ASCII pf6 atau pf8 | ASCII pf2 | Tersurat dalam parser GARbro |
| 0x03..0x06 | index_size: u32, mulai dihitung dari 0x07 | Sama | Tersurat |
| 0x07..0x0A | entry_count: u32 | Field 4 byte belum teridentifikasi; jangan anggap sebagai count | Tersurat untuk lokasi; makna PF2 belum diketahui |
| 0x0B.. | Entri pertama | entry_count: u32 di 0x0B..0x0E, entri pertama 0x0F | Tersurat |
| Akhir tiap entri | name_len u32; name[name_len] byte; reserved[4]; absolute_offset u32; stored_size u32 | name_len u32; name[name_len] byte; 12 byte field tambahan; absolute_offset u32; stored_size u32 | Inferensi jarak field langsung dari parser |
| Setelah entri | PF8 yang dibuat packer memiliki footer tabel lokasi ukuran; PF6 tidak boleh diasumsikan identik | Jangan asumsikan footer PF8 | Footer terverifikasi pada packer PF8 |

Untuk PF6/PF8, jika entri mulai pada **p**, urutannya: panjang di p, nama pada p+4, reserved di p+4+L, offset di p+8+L, ukuran di p+12+L, entri berikutnya p+16+L. Untuk PF2: nama pada p+4, area tambahan 12 byte mulai p+4+L, offset pada p+16+L, ukuran pada p+20+L, berikutnya p+24+L. Parser GARbro membaca nama PF2 memakai CP932; PF6/PF8 mencoba encoding yang dipilih serta alternatif CP932/UTF-8. Pertahankan nama mentah sebelum decoding untuk audit yang tidak kehilangan informasi. Kolom ukuran adalah *stored bytes* u32; **tidak ada field ukuran hasil dekompresi atau flag kompresi per entri** dalam skema standar ini. [S09]

**Footer PF8 menurut implementasi packer yang tersedia:** setelah N entri, u32 dengan nilai N+1; N nilai u64 lokasi kolom ukuran tiap entri **relatif ke offset arsip 0x0F**; u64 sentinel 0; terakhir u32 lokasi field hitungan footer **relatif ke offset 0x07**. Dengan demikian, index_size = 4 + Σ(16 + name_len_i) + 4 + 8(N+1) + 4, dan awal payload normal = 0x07 + index_size. Validator footer dibuat *opsional/berlabel varian*, karena GARbro mengabaikan footer dan dapat membaca arsip lain yang indeksnya sah. [S10][S09]

Contoh untuk satu nama berukuran L byte pada PF8 standar: count berada pada 0x07, entri dimulai 0x0B, footer count berada di 0x0B+16+L, nilai offset kolom ukuran pada tabel pertama sama dengan (0x0B+12+L)-0x0F. Jumlah ukur arsip indeks dan posisi setiap field harus diperiksa dengan aritmetika checked.

### 2.2 Kunci PF8 dan enkripsi payload

Indeks/header PF8 standar **tidak terenkripsi oleh skema XOR ini**. Ambil persis index_size byte dari rentang file[0x07 .. 0x07+index_size], termasuk entry count, daftar file, dan setiap footer yang termasuk rentang tersebut. Hitung K = SHA1(rentang itu), 20 byte. Untuk suatu payload file, plaintext[i] = stored[i] XOR K[i mod 20] dengan **i mulai 0 pada awal setiap file**. XOR yang sama mengenkripsi dan mendekripsi. GARbro menghitung SHA-1 indeks dan membungkus stream file; packer PF8 mengonfirmasi batas byte dan urutan XOR. PF2/PF6 **tidak memakai XOR PF8** pada implementasi GARbro ini. SHA-1 di sini adalah konstruksi kompatibilitas format, bukan alasan menganggap arsip terlindung secara kriptografis. [S09][S10]

Pengecualian penting: unpacker PF8 lain menyediakan daftar default video .mp4/.flv yang **dibiarkan plaintext** pada beberapa judul, sedangkan GARbro melakukan XOR untuk seluruh entry PF8. Jadi auditor sebaiknya menguji magic file kecil **baik mentah maupun setelah XOR**, merekam pilihan per entry, dan tidak menebak berdasarkan ekstensi saja. Variasi kunci, magic berbeda, data terkompresi di level format dalam, dan volume terpisah perlu jalur “format tak dikenal”, bukan dekripsi dipaksakan. [S10][S09]

### 2.3 Algoritme auditor Rust tanpa mengekstrak arsip

Berikut referensi fungsi inti; ia mengembalikan slice byte asli dan hanya mengalokasikan daftar entri serta prefix yang diperiksa. Ketika input berukuran gigabita, gunakan read-only mmap atau random-access Read+Seek untuk header/index/prefix. Contoh ini menargetkan tata letak PF2/PF6/PF8 yang didokumentasikan di atas; pengecekan footer PF8 dan decoding CP932 ditaruh dalam validator terpisah agar varian tidak salah ditolak. Dependensi: crate sha1 (RustCrypto). 

~~~rust
use sha1::{Digest, Sha1};

#[derive(Debug)]
struct Entry<'a> {
    name_raw: &'a [u8],
    offset: usize,
    size: usize,
    size_field_pos: usize,
}

#[derive(Debug)]
struct Pfs<'a> {
    version: u8,
    index_end: usize,
    entries: Vec<Entry<'a>>,
    key: Option<[u8; 20]>,
}

fn take<'a>(b: &'a [u8], p: usize, n: usize) -> Result<&'a [u8], &'static str> {
    b.get(p..p.checked_add(n).ok_or("offset overflow")?)
        .ok_or("truncated archive")
}

fn le32(b: &[u8], p: usize) -> Result<usize, &'static str> {
    Ok(u32::from_le_bytes(take(b, p, 4)?.try_into().unwrap()) as usize)
}

fn parse_pfs(b: &[u8]) -> Result<Pfs<'_>, &'static str> {
    let version = match take(b, 0, 3)? {
        b"pf2" => 2, b"pf6" => 6, b"pf8" => 8,
        _ => return Err("unknown PFS magic"),
    };
    let index_size = le32(b, 3)?;
    let index_end = 7usize.checked_add(index_size).ok_or("index overflow")?;
    take(b, 7, index_size)?;
    let count_pos = if version == 2 { 0x0b } else { 0x07 };
    let count = le32(b, count_pos)?;
    if count > 1_000_000 { return Err("implausible entry count"); }
    let mut p = if version == 2 { 0x0f } else { 0x0b };
    let mut entries = Vec::new(); // allocation bound follows validated count
    for _ in 0..count {
        let len = le32(b, p)?;
        let name_pos = p.checked_add(4).ok_or("name overflow")?;
        let name = take(b, name_pos, len)?;
        let extra = if version == 2 { 12usize } else { 4usize };
        let payload_pos = name_pos
            .checked_add(len).and_then(|x| x.checked_add(extra))
            .ok_or("entry overflow")?;
        let offset = le32(b, payload_pos)?;
        let size_field_pos = payload_pos.checked_add(4).ok_or("size overflow")?;
        let size = le32(b, size_field_pos)?;
        p = size_field_pos.checked_add(4).ok_or("entry overflow")?;
        if p > index_end { return Err("entry exceeds index"); }
        if offset < index_end { return Err("payload overlaps index"); }
        take(b, offset, size)?;
        entries.push(Entry { name_raw: name, offset, size, size_field_pos });
    }
    let key = if version == 8 {
        let digest = Sha1::digest(take(b, 7, index_size)?);
        let mut out = [0u8; 20];
        out.copy_from_slice(&digest);
        Some(out)
    } else { None };
    Ok(Pfs { version, index_end, entries, key })
}

// Slice pendek; untuk pembacaan acak mulai pada posisi i file, gunakan
// key[(i + j) % 20], bukan key[j % 20].
fn prefix(b: &[u8], entry: &Entry<'_>, key: Option<&[u8; 20]>,
          limit: usize) -> Vec<u8> {
    b[entry.offset..entry.offset + entry.size.min(limit)]
        .iter().enumerate()
        .map(|(i, x)| *x ^ key.map_or(0, |k| k[i % 20]))
        .collect()
}
~~~

**Validasi lanjutan yang perlu ditambah di implementasi produksi:** jangan buat Vec berkapasitas count sebelum batas indeks diketahui; batasi panjang nama dan metadata total; pertahankan raw filename, decode UTF-8/CP932 dengan indikator error; normalisasi slash, larang absolute path, komponen .., drive-letter dan tabrakan setelah case folding sesuai semantik judul; cek konflik entri dan rentang data; batasi proses probing prefix; cek footer PF8 bila hadir dan tandai ketidaksesuaian; gunakan hasil signature mentah/de-XOR untuk klasifikasi. Pada platform 32-bit jangan ubah u32 offset menjadi usize tanpa memeriksa kapasitas; host RuntimePack 64-bit lebih aman. Fungsi parse di atas sengaja tidak mengeksekusi Lua dan tidak menulis aset keluar. [S09][S10]

**Status literatur:** GARbro menyediakan kode parser C# dengan lisensi MIT; tool GalgameReverse menyediakan parser dan packer PF8; art3m1s-core mencantumkan crate PF8 vendored dari pfs-rs berlisensi MIT. Periksa lisensi repo dan file spesifik sebelum mengambil kode; spesifikasi perilaku di laporan ini disusun dari dua implementasi pembaca/penulis. [S09][S10][S01]

## 3. Strategi runtime Android dan boot langsung

### 3.1 Jalur yang direkomendasikan

Alur aplikasi khusus satu judul:

1. **Rust host (build time):** identifikasi system.ini, versi PFS dari magic, daftar volume, file utama, AST/ASB/IET, font, media dan PSB. Buat manifest dengan hash setiap sumber serta diagnosis fitur. Jangan mengekstrak seluruh PFS; bungkus berkas asli di APK atau paket instalasi yang diizinkan.
2. **APK:** Kotlin LauncherActivity tunggal dengan nama/icon gim tersebut; muat hanya runtime yang dibangun dari sumber audit. Tidak ada file chooser maupun launcher multi-gim.
3. **Penyiapan pertama kali:** salin byte arsip, system.ini dan file loose yang diperlukan dari assets ke context.filesDir/game menggunakan salinan atomik dan validasi hash. Ini *penyalinan byte-identik*, bukan perubahan isi sumber. Pada proyek sangat besar, rencanakan kapasitas APK dan duplikasi ruang sementara; alternatif Android asset descriptor langsung memerlukan bridge random access yang kompatibel dengan runtime.
4. **Penyimpanan:** set root save ke context.filesDir/saves/ID_GIM; tulis konfigurasi pengguna dan log ke context.filesDir/runtime/ID_GIM. Asset game dibuat read-only pada sisi runtime dan disalin ulang hanya bila hash build berubah. Tidak perlu izin penyimpanan eksternal.
5. **Runtime:** inisialisasi GL surface, audio, input dan clock dari Activity; buat objek resource, mount basis direktori PFS/volume, set save dir, muat bytes system.ini, buat runtime dan jalankan frame loop. API art3m1s-core meliputi art3m1s_get_api_v1, resources_create/mount_directory/mount_pfs/set_save_dir/set_override, runtime_set_resources, runtime_load_project_bytes, runtime_feed_touch dan advance_present/render. **Urutan mount, penggantian indeks pada tiap pemanggilan, dan argumen platform harus diuji pada revisi yang dipin; jangan mengasumsikan mount_pfs berulang berarti penumpukan arsip.** [S02]
6. **Boot:** Activity memanggil JNI milik RuntimePack yang membungkus C ABI runtime, meneruskan konfigurasi proyek serta manifest; mulai langsung dari entry yang ditentukan system.ini / script init. Jangan ganti system.ini asli di disk jika butuh parameter Android: sediakan override in-memory atau salinan konfigurasi khusus di namespace runtime. Bila bagian ANDROID tidak ada, uji pemilihan WINDOWS dengan adapter untuk path, shader dan layanan host; nilai string platform yang diterima implementasi perlu diverifikasi per revisi. [S02][S03]

Input layar sentuh harus memetakan koordinat setelah letterboxing ke koordinat panggung; ABI art3m1s-core menyediakan runtime_feed_touch, runtime_feed_mouse_button, dan runtime_feed_key. Dokumentasi engine resmi menyebut gesture flick/multitouch untuk auto, skip, sembunyikan pesan, dan backlog yang bisa dikonfigurasi, sehingga memaksa semua gesture menjadi tap berisiko mengubah perilaku per gim. Bila menambah overlay Android, putuskan konsumsi event pada view overlay dan teruskan event yang tidak ditangani ke surface. [S02][S07]

Sketsa host API, **pseudocode** yang harus disesuaikan signature C pada revisi yang dipin:

~~~text
onCreate():
    root = filesDir/game
    save = filesDir/saves/<title-id>
    ensureByteIdenticalAssets(root, apkManifest)
    resources = api.resources_create()
    api.resources_mount_directory(resources, root)
    // Pastikan implementasi membangun rantai .pfs.000… secara otomatis;
    // jangan panggil mount_pfs berulang tanpa menguji perilaku replace/append.
    api.resources_set_save_dir(resources, save)
    runtime = api.runtime_create(screenWidth, screenHeight, GLES_BACKEND)
    api.runtime_set_resources(runtime, resources)
    api.runtime_load_project_bytes(runtime, originalSystemIniBytes, platform)
    everyFrame(delta): api.runtime_advance_present(runtime, delta)
    onTouch(event): api.runtime_feed_touch(runtime, ...)
~~~

**Library native:** kebutuhan minimal arsitektural adalah satu .so host JNI dan shared libraries yang benar-benar dihasilkan build art3m1s-core untuk arm64-v8a (renderer, Lua, codec sesuai fitur). Tidak ada daftar nama final .so yang aman untuk semua feature sets; inventaris hasil build dengan readelf dan SBOM sebelum menulis packaging rules. Pada jalur artemis-compat, README menyebut output libartemis.so, GLES2, OpenSL ES, dan FFmpeg opsional. Pada jalur official, nama/muatannya menunggu SDK/lisensi resmi. [S01][S03]

**16 KB page size:** bila menarget perangkat Android dengan halaman memori 16 KB, ikuti panduan Google: NDK r28 atau yang lebih baru, Android Gradle Plugin 8.5.1 atau yang lebih baru, serta periksa alignment seluruh .so termasuk library pihak ketiga. Ini syarat packaging yang terpisah dari dukungan game Artemis. [S11]

### 3.2 Artroid, Tyranor, dan status pembuktian

Penulis Artroid+ 3.0 menyebut penambahan E-mote, file explorer sendiri, update engine Artemis per 20 Desember 2021, dukungan Android 6–12, dan kemungkinan kebutuhan patch khusus per game; mereka menyarankan mengonversi video ke MP4/H264/AAC untuk judul tertentu. Posting ini **tidak memberikan dasar cukup untuk memastikan** apakah menjalankan interpreter Lua mandiri, wrapper engine resmi, SDL2, atau binary yang boleh dipaketkan RuntimePack. Klaim kompatibilitas versi Android adalah keadaan saat publikasi 2022, bukan jaminan Android terbaru. Jangan menyebut Artroid pasti hanya 32-bit tanpa pemeriksaan APK ABI nyata. [S05]

**Tyranor-Next bukan TyranoScript:** proyek ini launcher multi-engine yang mencakup Artemis. Keberadaan com.ies_net.artemis.ArtemisActivity dan libartemis.so pada contoh integrasi hanya menerangkan antarmuka shell. Dokumen integrasi artemis-compat secara eksplisit mencatat simbol JNI/audio Tyranor yang belum cocok dan **tidak melakukan run pada perangkat Android untuk integrasi tersebut**; klaim uji judul di README artemis-compat tidak boleh disalahartikan sebagai uji Tyranor-Next tersebut. Template lain membagikan arsip ArtemisEngineAndroid; lisensi wrapper tidak mengesahkan binary engine yang disertakan. [S04][S06][S08]

### 3.3 Kontrak uji minimal APK

Kriteria lulus per gim: boot hingga judul tanpa UI pemilih; pesan Jepang dan font; input tap/long press/backlog/back; BG, sprite, pilihan dan transisi; suara dan video intro; save → force stop → buka APK → load; satu rute cerita penuh; jika ada, E-mote di beberapa scene dan patch volume bertumpuk; validasi di arm64 Android dengan 16 KB page. Pengujian harus memakai gim dan hak yang benar-benar tersedia, tanpa mengklaim dukungan hanya karena membuka title screen.

## 4. Spesifikasi skrip dan resolusi aset

### 4.1 AST, ASB, IET

Skrip Artemis resmi berupa urutan teks dan tag ala KAG: contoh dokumentasi meliputi [lyc id=0 file=logo], [trans type=1 time=1000], [wait ...], [splay ...], [call file=... label=...], [stop]; terdapat hook Lua dan subsistem layer/teks. Ini sintaks skrip dasar, bukan bukti bahwa setiap .ast adalah dokumen tag tersebut. [S07]

Ada judul yang memakai **.ast berbentuk tabel Lua** dengan deklarasi seperti astver=2.0 dan ast={block_00000={...}}; elemen dapat memuat bg, fg, bgm, text, voice, nama penutur, label/linknext, serta teks berbahasa yang berbeda. Parser komunitas ArtemisAstParser menyediakan contoh ini. Untuk auditor, parse subset literal tabel/token yang diperlukan dan *jangan menjalankan Lua arbitrer*. Struktur persis field/ruby bergantung judul dan versi; contoh bukan universal grammar. [S12]

Implementasi kompatibilitas memisahkan interpreter .iet (teks/tag/[lua]) dari pelaksana .asb terkompilasi dan juga mengklaim dukungan .ast. **Tidak ada spesifikasi offset biner tunggal .asb atau .iet yang berhasil diverifikasi lintas-versi** pada sumber primer yang ditelaah. Catat ekstensi dan magic/prefix, simpan raw bytes, dan biarkan runtime terpilih menangani binary yang dikenalnya. Teks .iet bukan bukti semua .iet bisa diparse sebagai teks. Identifikasi entry point dari system.ini dan sistem/init.lua bila ada; jangan hardcode file judul. [S03][S01]

### 4.2 Mount volume dan patch

Umumnya ada root.pfs dan volume bernomor root.pfs.000, root.pfs.001, dst.; nama dasar bisa berbeda. README artemis-compat mendeskripsikan rantai .000/.001/... dengan volume patch belakangan mengalahkan definisi lama dan **loose files sebagai fallback saat rantai tidak menemukan path**, bukan prioritas loose global. Bukti proyek patch NUKITASHI memperlihatkan volume .080/.081 menimpa file utuh pada .069 atau base. Ini cukup untuk kebijakan awal urut angka naik, konflik terakhir menang **pada profil judul yang tervalidasi**; bukan bukti bahwa semua edisi Artemis memakai prioritas yang sama. [S03][S13]

Referensi file seperti :bg/ atau :fg/ dan perbedaan case/path harus ditafsirkan bersama konfigurasi sistem, layout folder, pencarian runtime, dan berkas yang ditimpa. Manifest audit simpan kunci resource ternormalisasi, arsip sumber, offset, hash, rank patch, dan semua kandidat konflik. Terapkan keputusan mount sesuai runtime yang benar-benar digunakan; jangan gabungkan atau tulis ulang PFS asli. Jika runtime C ABI mengganti indeks mount ketika dipanggil ulang, gunakan mount_directory atas folder lengkap bila ia auto-discovery volume, atau tambah adapter VFS terpisah yang menjaga rantai dan keputusan precedence.

## 5. Media dan karakter dinamis

| Fitur | Bukti yang ada | Strategi RuntimePack |
|---|---|---|
| Gambar | GARbro mendaftarkan PNG, JPEG, IPT di PFS; art3m1s-core menyebut tekstur E-mote dan backend GPU | Sniff magic tiap entry; WebP mungkin muncul di judul tertentu, jangan diasumsikan selalu didukung |
| Audio | GARbro mendaftarkan OGG; artemis-compat memiliki dekoder Vorbis dan OpenSL ES | Tandai Ogg Vorbis sebagai target awal; uji WAV/MP3 menurut judul/codec runtime |
| Video | Situs Artemis menyebut MP4 fullscreen tergantung perangkat dan Ogg Theora untuk layer video/alpha; artemis-compat memakai FFmpeg opsional, Artroid menyarankan transcode MP4 H264/AAC | Untuk aset video asli, pilih decoder sesuai format dan lisensi codec; verifikasi sinkron audio, alpha dan pause/resume; jangan anggap WMV/MPG/OGV semuanya otomatis bekerja di MediaCodec |
| E-mote | Engine resmi menyebut varian komersial E-mote dengan perundingan lisensi; art3m1s-core mengklaim implementasi subset PSB, artemis-compat mengakui ketidaklengkapan model/mesh/physics | Klasifikasikan PSB/MDF dan adegan yang memakainya; tuntut uji visual nyata; integrasi SDK resmi memerlukan lisensi tersendiri |
| Live2D | Teknologi terpisah dari E-mote; tidak ada bukti bahwa semua gim Artemis memerlukannya | Deteksi per judul; jangan wajibkan libLive2DCubismCore.so atau libEmote.so pada setiap APK |

Sumber: [S07][S09][S01][S03][S05]. Untuk judul seperti NUKITASHI, jangan generalisasikan kemampuan suatu build/patch dari versi Jepang ke versi lokalisasi tanpa memeriksa arsip yang akan dipaket.

## 6. Lisensi, provenance, dan risiko

1. **Artemis resmi** adalah perangkat lunak komersial/proprietary; halaman resmi menawarkan kemampuan integrasi Android sebagai Activity tetapi tidak memberikan hak mendistribusikan binary SDK secara umum. Jika ingin memakai engine resmi, minta izin tertulis Mikage/iMel yang mencakup APK per gim, ABI, distribusi binary, hak modifikasi shell, E-mote/codec dan dukungan masa depan. [S07]
2. **art3m1s-core** mendeklarasikan MPL-2.0. Kewajiban atas file yang dicakup dan dimodifikasi berbeda dari GPL; aplikasi host bisa mempunyai lisensi lain selama ketentuan MPL terpenuhi. Periksa komponen vendored seperti PF8 MIT, fitur opsional KRKRSDL3, FFmpeg, Lua/graphics dan seluruh binary final. **Memilih feature flags** penting: default repo menyertakan fitur eksperimen yang tidak diperlukan untuk Artemis. Pernyataan status license di README harus divalidasi terhadap file LICENSE, THIRD_PARTY_NOTICES, dan Cargo.lock yang dipin. [S01]
3. **artemis-compat** menyatakan GPL-3.0 untuk kodenya; Lua 5.1.5 MIT dan komponen lain memiliki lisensi masing-masing. Bila menggabungkannya dengan shell APK, rencanakan distribusi yang memenuhi kewajiban GPL untuk karya gabungan; memindahkan runtime ke .so terpisah **tidak otomatis** menghilangkan kewajiban itu. Lakukan penilaian lisensi produk sebelum merilis APK closed source. Klaim clean-room penulis adalah pernyataan proyek, bukan audit independen. [S03]
4. **Artroid, shell Tyranor, dan template binary**: source wrapper, lisensi wrapper atau nama libartemis.so tidak cukup sebagai hak memakai ulang binary engine resmi. Verifikasi rantai asal setiap .jar/.so; bila tidak jelas, keluarkan dari artifact. Meniru antarmuka fungsional dari implementasi independen yang berlisensi jelas lebih dapat diaudit daripada menyalin binary tanpa perizinan. [S05][S06][S08]
5. **Konten gim** mempunyai hak tersendiri dari engine dan parser. Compiler lokal untuk aset yang diberikan pengguna dan APK yang dibagikan publik adalah dua penggunaan berbeda; pastikan hak penggandaan/distribusi gim, patch dan soundtrack untuk model distribusi RuntimePack. Ini juga berlaku untuk gim dewasa; format/engine tidak memberikan lisensi aset.

**Risiko teknis terbesar:** kompatibilitas ASB/IET dan script Lua khusus, aturan mount antargenerasi, E-mote lanjutan, shader/video, ABI/perangkat Android terbaru, serta ukuran APK/ruang saat first run. **Risiko hukum terbesar:** binary Artemis/Artroid tidak jelas izin redistribusinya dan aset gim tidak otomatis dapat dibagikan.

### Keputusan implementasi yang dapat langsung diambil

- Buat modul Rust pfs_audit untuk PF2/PF6/PF8 read-only, metadata mentah dan probe 32–256 byte; tandai confidence de-XOR dan setiap anomaly.
- Pilih revisi art3m1s-core yang dipin, audit manifest dependency/features, bangun arm64-v8a dan API C minimal; buat Activity khusus satu gim dengan save root internal dan tanpa chooser.
- Buat fixture arsip kecil berizin untuk memeriksa formula SHA-1/footer, dan satu gim uji nyata berizin untuk memeriksa script → title → save/load → rute lengkap.
- Jika judul gagal, simpan laporan resource/scene/instruksi yang gagal lalu bandingkan artemis-compat; jangan otomatis memakai binary resmi yang ada di APK/template lain.
- Untuk rilis publik, lakukan tinjauan lisensi tertulis atas runtime, seluruh library transitif dan konten gim per judul.

## Sumber primer

- [S01] [art3m1s-core: README, fitur, lisensi dan batas](https://github.com/Alphaly2K/art3m1s-core).
- [S02] [art3m1s-core: FFI_REFERENCE.md, resource dan runtime ABI](https://github.com/Alphaly2K/art3m1s-core/blob/master/doc/FFI_REFERENCE.md).
- [S03] [artemis-compat: README, kemampuan, Lua, backend, lisensi dan batas](https://github.com/Weiss-UltimateSavior/artemis-compat).
- [S04] [artemis-compat: embedding guide, kendala Tyranor JNI dan status uji](https://github.com/Weiss-UltimateSavior/artemis-compat/blob/main/docs/embedding.md).
- [S05] [Catatan rilis penulis Artroid+ 3.0](https://myskrpatch.tistory.com/m/82).
- [S06] [Tyranor-Next: repositori multi-engine](https://github.com/Weiss-UltimateSavior/Tyranor-Next); [template Artemis Android oleh pihak ketiga](https://github.com/ATSPwang618/artemis-engine-port-tools).
- [S07] [Artemis Engine: deskripsi dan contoh script situs resmi Mikage](https://www.ies-net.com/?page_id=24).
- [S08] [RinneMobile: contoh shell dan deteksi Artemis](https://github.com/Weiss-UltimateSavior/RinneMobile).
- [S09] [GARbro: ArcPFS.cs, pembaca PF2/PF6/PF8 dan lisensi file](https://github.com/morkt/GARbro/blob/master/ArcFormats/Artemis/ArcPFS.cs).
- [S10] [GalgameReverse: artemis_pf8.py, header/footer dan XOR](https://github.com/YuriSizuku/GalgameReverse/blob/master/project/artemis/src/artemis_pf8.py).
- [S11] [Android Developers: dukungan ukuran halaman 16 KB](https://developer.android.com/guide/practices/page-sizes).
- [S12] [ArtemisAstParser: contoh berkas AST dan struktur block](https://github.com/slqy123/ArtemisAstParser/blob/main/README.md).
- [S13] [NUKITASHI JP restoration patch: contoh urutan patch .069/.080/.081](https://github.com/That-Ruben/nukitashi_series_JP_restoration_patch).

**Catatan metodologi:** spesifikasi byte diperiksa terhadap kode pembaca GARbro dan kode pembuat/pembaca PF8; klaim kemampuan runtime didasarkan pada dokumentasi pengembang, belum benchmark perangkat RuntimePack. Poin yang dinyatakan belum terverifikasi sengaja tidak diperlakukan sebagai kontrak format atau janji kompatibilitas.
