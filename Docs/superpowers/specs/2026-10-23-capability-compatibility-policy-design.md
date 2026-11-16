# Capability-Based Compatibility Policy and Engine Extensibility — Design

- Date: 2026-10-23
- Status: DRAFT — menunggu review pengguna
- Baseline source: `runtime-v2` @ `442335c`, branch `fix/linux-batch-compat-20261022`
- Bukti pendukung:
  - `_reports/linux-remediation-20261022/ARCH-ENGINE-EXTENSIBILITY-20261023.md`
  - `_reports/linux-remediation-20261022/ARCH-OPSC-RISK-REVIEW-20261023.md`
  - `_reports/linux-remediation-20261022/FORESTIA-INDEPENDENT-REVIEW-20261023.md`
  - `_reports/linux-remediation-20261022/G03-RESUME-DIAGNOSTIC-20261023.md`
  - `_reports/linux-remediation-20261022/G09-G05-RESUME-CONTRACTS-20261023.md`

## 1. Ringkasan

RuntimePack saat ini mencampur tiga hal berbeda dalam satu keputusan:

1. **Fakta tentang game** — misalnya ada pemanggilan `child_process`.
2. **Kemampuan runtime** — apakah Android/runtime menyediakan penggantinya.
3. **Keputusan build** — blokir, peringatkan, atau izinkan.

Karena ketiganya bercampur, ketidakpastian analisis statis diperlakukan sama dengan bukti kerusakan nyata. Akibatnya game yang di perangkat berjalan normal tertahan, sementara sistem tidak memberi jalan untuk membedakan "belum terbukti" dari "terbukti tidak aman".

Desain ini memisahkan ketiganya secara eksplisit, menetapkan **satu titik keputusan**, dan memberi satu katup eksplisit untuk ketidakpastian tanpa mengubahnya menjadi klaim kompatibilitas.

## 2. Tujuan

- Satu keputusan kompatibilitas dihasilkan oleh satu kebijakan bersama, dipakai oleh `inspect`, `audit`, dan `build`.
- Menambah integrasi engine baru tidak memerlukan tambalan kebijakan di banyak tempat.
- Klaim kemampuan runtime hanya boleh muncul bila ada implementasi dan bukti uji.
- Ketidakpastian tetap berstatus tidak pasti; tidak boleh disamarkan menjadi "didukung" maupun "ditolak permanen".
- Gate keras yang sudah ada (integritas input, keamanan path, signing, toolchain) tidak boleh melemah.

## 3. Bukan tujuan

- Framework plugin generik atau pemuatan engine secara dinamis saat runtime.
- Menghapus scanner statis atau menggantinya dengan eksekusi game saat build.
- Menjamin game berjalan hanya karena APK berhasil dibangun.
- Pengecualian berdasarkan judul game atau hash tertentu.
- Menyatukan semua engine dalam satu refactor besar.
- Membuat matriks kemampuan untuk seluruh API Node/Python/RGSS sekaligus.

## 4. Model klasifikasi

Setiap temuan diberi satu kelas. Kelas menyatakan **status pengetahuan**, bukan tingkat bahaya.

| Kelas | Definisi | Contoh |
|---|---|---|
| `Supported` | Kebutuhan dipenuhi runtime/shim yang sudah teruji | Baca file lewat VFS runtime |
| `Advisory` | Terbukti tidak diperlukan pada target | Plugin tidak terdaftar pada manifest aktif |
| `Degraded` | Ada pengganti yang bekerja, perilaku berbeda dan terdokumentasi | Wallpaper digambar sebagai lapisan sprite, bukan wallpaper desktop |
| `Blocked` | Wajib, dan terbukti tidak tersedia | Data terenkripsi proprietary tanpa kontrak format |
| `Unknown` | Belum cukup bukti untuk menentukan kelas mana pun | Efek `eval` lintas modul belum dapat dibatasi |

Aturan penting:

- `Unknown` **tidak boleh** diperlakukan sebagai `Advisory`.
- `Degraded` hanya sah bila ada implementasi pengganti yang benar-benar ada dan diuji, bukan sekadar "tidak crash".
- `Blocked` hanya sah bila ada bukti positif ketidaktersediaan, bukan sekadar kegagalan analisis.

## 5. Dua kebijakan build

Klasifikasi menghasilkan kelas. Kebijakan build menentukan tindakan.

| Kelas | `Default` | `Experimental` |
|---|---|---|
| `Supported` | build | build |
| `Advisory` | build + laporan | build + laporan |
| `Degraded` | build + laporan | build + laporan |
| `Blocked` | **blokir** | **blokir** |
| `Unknown` | **blokir** | teruskan, berlabel `Experimental` |

### 5.1 Default

Tidak ada pelonggaran. Ini adalah Opsi C dijalankan penuh.

### 5.2 Experimental

Diaktifkan hanya lewat flag eksplisit pengguna pada CLI:

```bash
runtimepack build <game> --allow-unknown
```

Konsekuensi yang mengikat:

- Status akhir laporan **tidak boleh** `Compatible`. Gunakan `Experimental` / `Unverified`.
- Setiap `Unknown` yang diteruskan dicatat lengkap: lokasi, alasan, dan apa yang belum dibuktikan.
- Tidak mengubah `Blocked` menjadi lolos.
- Tidak melewati kegagalan integritas input, keamanan path, signing, atau toolchain.
- Tidak menjadi pengaturan tersimpan; hanya berlaku untuk invokasi tersebut.

### 5.3 Alasan pemisahan

Tanpa katup ini, satu-satunya cara menyelesaikan game yang analisisnya mentok adalah melemahkan analisis itu sendiri — persis yang terjadi pada `442335c` dan ditolak reviewer. Dengan katup ini, analisis tetap ketat dan ketidakpastian tetap terlihat.

## 6. Titik keputusan tunggal

Saat ini ada banyak tempat yang dapat memblokir build secara independen:

- `core/src/build_plan/audit_policy.rs` (daftar hard-block berbasis kode)
- `cli/src/main.rs` (pemeriksaan status kompatibilitas)
- Empat daftar `make_finding` per engine di `asset_audit`
- `scanner.rs` dan `asset_audit.rs` yang menghitung batas wrapper dari root berbeda

Desain ini menetapkan:

```text
Temuan bertipe (finding + kelas + bukti)
        +
Profil runtime (capability + batasan)
        +
Kebijakan build (Default / Experimental)
        ↓
    Satu fungsi kebijakan di core
        ↓
Keputusan: build / blokir, beserta alasan terstruktur
```

Konsekuensi:

- `inspect`, `audit`, dan `build` memakai fungsi yang sama.
- Kode diagnostik (`CMP-*`) hanya untuk **pelaporan**, bukan daftar kebijakan tersembunyi.
- Scanner dan auditor wajib memakai **konteks input yang sama** (root konten dan batas wrapper yang sama).
- Menambah engine tidak menambah entri pada tabel pemblokir.

## 7. Batas tanggung jawab

### 7.1 Engine adapter

Melaporkan fakta dan kebutuhan:

- deteksi engine dan normalisasi struktur input;
- daftar kebutuhan (capability yang diminta);
- analisis statis yang menghasilkan temuan bertipe;
- konfigurasi/adaptasi ke runtime;
- fixture dan tes khusus engine.

Engine **tidak** menentukan kebijakan blokir.

### 7.2 Profil runtime

Menyatakan kemampuan sebenarnya:

- identitas dan versi runtime;
- capability yang tersedia beserta batasannya;
- shim yang disediakan;
- status bukti: `source-only`, `host-tested`, `device-validated`.

Capability tidak boleh diklaim tanpa implementasi dan tes. Deklarasi konfigurasi tidak menciptakan kemampuan; ia hanya mendeskripsikannya.

### 7.3 Core

Memegang:

- tipe temuan dan kelas;
- fungsi kebijakan tunggal;
- validasi manifest runtime;
- pemetaan engine → runtime tanpa cabang khusus judul game.

Core tidak memuat tabel aset atau aturan milik engine tertentu.

### 7.4 Packager

Tetap bersama untuk semua engine: project Android, container konten, signing, laporan. Backend runtime menyediakan integrasi khususnya.

## 8. Kontrak data (konseptual)

Tipe-tipe berikut menggambarkan bentuk kontrak. Nama final dan bentuk serde ditentukan pada tahap rencana implementasi.

```text
FindingClass      = Supported | Advisory | Degraded | Blocked | Unknown
BuildPolicy       = Default | Experimental
CapabilityId      = namespace.nama          (mis. web.node.path, python.native_ext)
ProofStatus       = SourceOnly | HostTested | DeviceValidated
CapabilityClaim   = { id, tersedia, batasan, shim, proof_status }
RuntimeProfile    = { identitas, versi, capabilities[] }
Requirement       = { capability_id, wajib, situs_bukti }
Finding           = { kelas, capability_id?, bukti, alasan }
Decision          = { izin_build, status_laporan, alasan[], unknown_diteruskan[] }
```

Aturan validasi:

- `CapabilityClaim` tanpa implementasi → tidak boleh `tersedia = true`.
- `Requirement` wajib yang tidak ada padanannya → `Blocked` atau `Unknown`, tidak pernah `Supported`.
- `RuntimeProfile` dengan field tidak dikenal → error, bukan diabaikan diam-diam.
- Identitas runtime ikut tercatat pada laporan agar hasil dapat direproduksi.

## 9. Provenance

Setiap laporan build wajib memuat:

- versi ruleset kebijakan;
- identitas sumber game (hash konten);
- identitas runtime (nama + versi);
- hash shim/overlay yang dipakai;
- status validasi runtime (`NOT_RUN` bila belum diuji perangkat);
- kebijakan build yang dipakai (`Default` / `Experimental`).

Perubahan semantik kebijakan menaikkan versi ruleset. Cache dengan versi lama tidak boleh dianggap sah.

## 10. Penanganan error

- Manifest runtime tidak valid → gagal dengan pesan jelas, bukan penelanan error.
- Engine ID tidak dikenali → error eksplisit, bukan fallback diam-diam.
- Capability diminta tetapi tidak dideklarasikan runtime → `Unknown`, bukan dianggap tersedia.
- Temuan tanpa bukti situs → `Unknown`, bukan `Advisory`.
- Kegagalan I/O saat membaca input game → gagal, tidak dianggap kosong.

## 11. Strategi pengujian

Berlapis, dan setiap lapisan punya klaim berbeda:

| Lapisan | Membuktikan |
|---|---|
| Kontrak capability | Deklarasi runtime cocok dengan implementasi shim |
| Fixture engine | Analisis menghasilkan kelas yang benar untuk input terkontrol |
| Regresi game nyata | Perubahan kebijakan tidak mengubah hasil game yang sudah terverifikasi |
| Pengujian perangkat | Game benar-benar berjalan (di luar cakupan build) |

Aturan:

- Setiap capability baru menambah satu tes kontrak.
- Setiap bug game nyata menambah satu fixture regresi.
- Tes negatif wajib ada: input yang harusnya `Blocked` tidak boleh lolos menjadi `Advisory`.
- Klaim `Degraded` wajib punya tes yang menunjukkan pengganti benar-benar berjalan.

## 12. Tahapan migrasi

Urutan ini mengikuti temuan arsitektur. Setiap tahap berdiri sendiri dan tidak melemahkan gate yang ada.

### Tahap 1 — Fondasi

- Selaraskan penulisan engine ID (satu dialek).
- Registry runtime dibaca dari data; manifest rusak menghasilkan error.
- Hapus duplikasi daftar detektor.

### Tahap 2 — Satu keputusan

- Satukan pandangan input scanner dan auditor.
- Satukan fungsi keputusan.
- Ganti daftar pemblokir berbasis kode dengan kelas temuan.

### Tahap 3 — Opsi C pada RPG Maker Web

- Terapkan model kelas dan dua kebijakan build pada satu runtime.
- Uji dengan fixture dan game nyata (Forestia, B-Prison).
- Tidak ada pengecualian berdasarkan judul atau hash.

### Tahap 4 — Perluas

- Migrasikan runtime lain satu per satu dengan regresi.
- Tambah capability sesuai kebutuhan nyata, bukan spekulasi.

## 13. Risiko dan mitigasi

| Risiko | Mitigasi |
|---|---|
| `Degraded` menjadi tempat sampah `Unknown` | `Degraded` wajib punya implementasi pengganti dan tes; `Unknown` punya status sendiri |
| `Experimental` dipakai sebagai jalan pintas permanen | Flag tidak tersimpan; laporan selalu berlabel belum terverifikasi |
| Matriks capability menjadi dokumentasi yang melenceng | Setiap entri butuh tes kontrak; status bukti eksplisit |
| Refactor besar merusak engine yang sudah jalan | Migrasi bertahap, regresi per engine, tidak ada perubahan perilaku diam-diam |
| Klaim berlebihan pada pengguna | Pemisahan tegas: APK terbentuk ≠ game berjalan |

## 14. Keputusan yang sudah disetujui

- Model klasifikasi lima kelas (Opsi C).
- Dua kebijakan build: `Default` dan `Experimental` dengan flag eksplisit.
- `Experimental` tidak pernah menghasilkan status `Compatible` dan tidak melewati `Blocked`.
- Migrasi bertahap, dimulai dari fondasi, bukan dari matriks capability.
- Tidak ada pengecualian berdasarkan judul game atau hash.

## 15. Keputusan atas pertanyaan terbuka (disetujui 2026-10-23)

1. **Nama flag:** `--allow-unknown`. Dipilih karena menyatakan secara tepat apa yang dilonggarkan (hanya kelas `Unknown`) dan tidak menyiratkan bahwa `Blocked` dapat dilewati.
2. **`Degraded` dipilih otomatis, tetapi wajib tercatat.** Build lanjut; laporan memuat daftar degradasi beserta perbedaannya. Dasar: pengganti sudah ada dan diuji, sehingga tidak perlu friksi tambahan.
3. **Cakupan capability tahap awal: subset minimum.** Hanya capability yang benar-benar muncul pada Forestia dan B-Prison. Mekanisme dibuktikan lebih dulu; perluasan menyusul berdasarkan kebutuhan nyata.
4. **Berkas peringatan di dalam APK: ya, satu berkas teks.** Nama: `EXPERIMENTAL-NOTICE.txt` di root APK, memuat daftar `Unknown` yang diteruskan dan pernyataan bahwa build belum teruji perangkat. Hanya dibuat pada build `Experimental`.
