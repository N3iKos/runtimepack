# Riset Teknis Visual Novel Maker (VNM) untuk Android WebView

## A. Ringkasan Temuan Utama dan Kelayakan Port

Visual Novel Maker/KOMODO Visual Novel Maker adalah engine JavaScript/HTML5 tersendiri. Dokumentasi engine menyebut satu “Basic Engine” HTML5 yang menangani grafis 2D, video, audio, dan input; scripting game dapat menggunakan JavaScript/CoffeeScript. Ini bukan RPG Maker, dan kompatibilitas RPG Maker MV/MZ tidak boleh dianggap berlaku untuk VNM.

Pada ekspor desktop lama, VNM membundel NW.js. Dokumentasi packaging lama secara eksplisit menyebut file runtime NW.js dan `package.json`, dengan `index.html` sebagai entry point game. Namun pada VNM 1.0.1111, dirilis 22 Juni 2022, desktop runtime resmi dipindahkan dari NW.js ke Electron. Changelog bahkan memperingatkan bahwa extension atau command spesifik NW.js dapat berhenti bekerja.

Dengan demikian, “VNM PC export” bukan satu ABI/runtime tunggal. Ini sangat penting bagi GOS-01. Belum ditemukan bukti publik bahwa `GOS-01` merupakan nomor versi, codename resmi, atau branch VNM. Untuk saat ini paling aman memperlakukannya sebagai identifier lokal sampai file runtime game memberikan bukti lain.

Android masih tercantum sebagai target ekspor di situs resmi VNM, tetapi dokumentasi Android yang dapat ditemukan menggunakan toolchain Cordova lama—JDK 7, Android Studio/SDK, Node.js, Cordova—dan jelas berasal dari generasi deployment lama. Ada pula laporan komunitas pada 2023 bahwa mengikuti instruksi Android tersebut masih menghasilkan kegagalan build yang tidak terselesaikan.

Untuk ekspor Web/Mobile, konfigurasi VNM sendiri membatasi storage ke “Internal Storage”, sedangkan `gs.GameStorage` mendokumentasikan bahwa backend berbeda menurut platform: di browser data masuk ke storage internal browser, sedangkan local/desktop menggunakan disk. Ini mendukung kekhawatiran terhadap shim `window.nw`: jika keberadaannya membuat runtime percaya bahwa ia sedang berada pada platform desktop, adapter dapat memindahkan bukan hanya API window tetapi juga keputusan storage ke cabang yang salah.

Rekomendasi arsitektural:

1. Jika project sumber tersedia, gunakan ekspor Web/Mobile sebagai titik awal, bukan ekspor PC.
2. Jika hanya ekspor PC yang tersedia, buat adapter khusus VNM yang melakukan fingerprint runtime dan mengimplementasikan capability yang benar-benar dibutuhkan game.
3. Jangan menjadikan “emulasi NW.js generik” sebagai sasaran utama. NW.js sendiri tidak mendukung Android, sementara VNM sudah berpindah ke Electron sejak 2022.

Keberhasilan boot, membuka menu, atau mencapai chapter tertentu hanya merupakan smoke test. Itu belum membuktikan persistensi save, voice lifecycle, recovery setelah process death, asset completeness, atau stabilitas overlay.

---

## B. Peta Arsitektur Runtime

```text
Game data / scene / script
        │
        ▼
Visual Novel Maker game framework
  GameManager
  Components / Commands
  Component_MessageBehavior
        │
        ├─────────────► gs.GameStorage
        │                 │
        │                 ├─ browser/mobile storage
        │                 └─ local filesystem pada desktop
        │
        ├─────────────► gs.AudioBuffer
        │               gs.AudioBufferReference
        │                 │
        │                 └─ Web Audio / media stack
        │
        ├─────────────► gs.Input
        │               gs.MouseInput
        │                 │
        │                 ├─ keyboard
        │                 ├─ mouse
        │                 ├─ touch
        │                 └─ gamepad
        │
        ├─────────────► gs.Graphics
        │                 │
        │                 └─ HTML canvas / WebGL
        │
        └─────────────► resource loader
                          │
                          ├─ data/
                          ├─ resources/
                          ├─ image/audio/font/video
                          └─ optional encrypted content

Platform shell:
  old desktop export  -> NW.js + Node
  newer desktop       -> Electron
  web/mobile          -> browser/mobile execution model
  old official Android packaging -> Cordova
  proposed host       -> Android WebView + VNM adapter
```

Dokumentasi “Engine Getting Started” menyebut Basic Engine menyediakan 2D graphics, video, audio dan input serta berjalan di atas teknologi HTML5. Dokumentasi API publik menunjukkan bootstrap yang mencakup loading resource sistem, setup input, audio, dan video.

Untuk rendering, sumber publik mengarah ke `gs.Graphics`, bukan PIXI. `gs.Graphics.initialize(width, height, parent, keepRatio)` menangani canvas dan aspect preservation.

Untuk input, `gs.Input` menyatukan keyboard, mouse, touch dan gamepad ke input logis. `gs.MouseInput` mendokumentasikan bahwa touch otomatis diterjemahkan menjadi mouse input: single tap menjadi left click dan double tap menjadi right click.

---

## C. Tabel Temuan

| Topik | Temuan | Versi/Konteks | Sumber | Tingkat Keyakinan |
|---|---|---|---|---|
| Engine | VNM menggunakan Basic Engine berbasis HTML5 untuk graphics/video/audio/input | Dokumentasi engine resmi | https://asset.visualnovelmaker.com/help/Engine_Getting_Started.htm | Tinggi |
| Scripting | JavaScript/CoffeeScript tersedia untuk extension/scripting | VNM umum | https://www.visualnovelmaker.com/ | Tinggi |
| Rendering | API publik adalah `gs.Graphics`; canvas/WebGL dan `keepRatio` terdokumentasi | Dokumentasi engine | https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.Graphics.html | Tinggi |
| Desktop lama | Windows package menggunakan NW.js, `package.json`, `index.html` | Dokumentasi packaging lama | https://asset.visualnovelmaker.com/help/Create_Game_Package.htm | Tinggi |
| Desktop baru | VNM 1.0.1111 beralih NW.js → Electron | 22 Jun 2022+ | https://steamcommunity.com/app/495480/announcements/ | Tinggi |
| Android | Android masih dipasarkan sebagai target | Situs resmi | https://www.visualnovelmaker.com/ | Tinggi |
| Android tooling | Panduan memakai Cordova/JDK7/Node.js | Dokumentasi deployment legacy | https://asset.visualnovelmaker.com/help/For_Android.htm | Tinggi untuk dokumen, rendah untuk relevansi modern |
| Storage | `gs.GameStorage` menyimpan save/settings; browser dan local memakai backend berbeda | API resmi | https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.GameStorage.html | Tinggi |
| Web/mobile storage | “Internal Storage” adalah opsi untuk Web & Mobile | Konfigurasi VNM | https://asset.visualnovelmaker.com/help/System.htm | Tinggi |
| Slot save | `GameManager.save(slot, …)` berbeda dari `saveGlobalData()` dan `saveSettings()` | API resmi | https://asset.visualnovelmaker.com/help/game_script_doc/classes/GameManager.html | Tinggi |
| `$PARAMS` | `PARAMS` adalah JSON base64 berisi additional info; field publik tidak dirinci | Build Configurations | https://asset.visualnovelmaker.com/help/Build_Configurations.htm | Tinggi |
| `$PARAMS.isOffline` | Tidak ditemukan spesifikasi publik yang menjadikan field ini kontrak universal | GOS-01 perlu inspeksi | — | Belum diketahui |
| Window API | NW.js menyediakan `Window.setResizable()` | NW.js | https://docs.nwjs.io/References/Window/ | Tinggi |
| Buffer | `Buffer.isBuffer()` adalah API Node | Node.js | https://nodejs.org/api/buffer.html | Tinggi |
| JSZip | output `nodebuffer` membutuhkan Node-style Buffer | JSZip | https://stuk.github.io/jszip/documentation/api_jszip/generate_async.html | Tinggi |
| Voice | `voiceDuration()` mengembalikan durasi voice dalam frames atau 0 | API resmi VNM | https://asset.visualnovelmaker.com/help/game_script_doc/classes/Component_MessageBehavior.html | Tinggi |
| Voice object | `voice` bertipe `gs.AudioBufferReference` | API resmi VNM | https://asset.visualnovelmaker.com/help/game_script_doc/classes/Component_MessageBehavior.html | Tinggi |
| Audio decode | `gs.AudioBuffer.decode()` diperlukan sebelum playback; buffer dapat di-dispose | API resmi VNM | https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.AudioBuffer.html | Tinggi |
| `voiceDuration` implementation | Body/source publik yang menunjukkan akses `.buffer` belum ditemukan | Pencarian source publik | — | Belum diketahui |
| Web Audio stop | Calling `stop()` sebelum source pernah `start()` adalah invalid state | Web Audio spec | https://www.w3.org/TR/webaudio/ | Tinggi |
| `connect()` | `AudioNode.connect()` membutuhkan destination yang valid | Web Audio/MDN | https://developer.mozilla.org/en-US/docs/Web/API/AudioNode/connect | Tinggi |
| Input | VNM menggabungkan keyboard/mouse/touch/gamepad | API resmi | https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.Input.html | Tinggi |
| Touch mapping | Touch dapat otomatis menjadi mouse events/logical clicks | API resmi | https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.MouseInput.html | Tinggi |
| Focus | `gs.Graphics` memiliki focus-lost/focus-receive callbacks | API resmi | https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.Graphics.html | Tinggi |
| Fonts | VNM menerima WOFF/FNT | Resource Standards | https://asset.visualnovelmaker.com/help/Resource_Standards.htm | Tinggi |
| OTS | Chromium memvalidasi WOFF/WOFF2 dan menolak data yang gagal parsing/validation | Chromium OTS | https://chromium.googlesource.com/external/github.com/khaledhosny/ots/+/HEAD/docs/DesignDoc.md | Tinggi |
| Encryption | VNM packaging menyediakan “Encrypt Content” | Packaging/changelog | https://asset.visualnovelmaker.com/help/Create_Game_Package.htm | Tinggi |
| Adapter VNM Android | Belum ditemukan proyek open-source yang secara eksplisit mengimplementasikan runtime compatibility VNM PC→Android WebView | Pencarian publik | — | Sedang |

---

## D. Analisis Tiap Masalah Lokal

### 1. `$PARAMS.isOffline && window.nw` dan `fs.writeFileSync`

Yang terverifikasi adalah bahwa `gs.GameStorage` mempunyai perilaku platform-dependent: browser menggunakan internal browser storage, sementara local execution menggunakan disk.

Dokumentasi build mengonfirmasi bahwa `PARAMS` adalah JSON base64 dengan informasi tambahan, tetapi field `isOffline` tidak dijelaskan dalam dokumentasi publik yang ditemukan.

Karena itu kondisi persis:

```js
$PARAMS.isOffline && window.nw
```

harus dianggap sebagai kontrak build GOS-01 atau versi tertentu sampai source membuktikan lebih luas.

Jika kondisi itu memilih filesystem backend dan adapter membuat `fs.writeFileSync` menjadi no-op, fungsi save dapat terlihat selesai pada layer atas tanpa commit durable. Ini konsisten dengan observasi lokal, tetapi belum membuktikan bagaimana semua versi VNM memilih storage.

Data tambahan yang diperlukan:

- decoded `$PARAMS`,
- source tempat `isOffline` dibaca,
- call stack `gs.GameStorage`,
- key/path setiap write,
- cold-start read-back sesudah Android process benar-benar dibunuh.

### 2. `Buffer.isBuffer is not a function`

Node menyediakan `Buffer.isBuffer()`. JSZip juga mempunyai output Node-specific seperti `nodebuffer`.

Error:

```text
Buffer.isBuffer is not a function
```

menunjukkan kontrak Node yang tidak terpenuhi, misalnya objek `Buffer` shim ada tetapi bukan Node-compatible Buffer.

Belum ada bukti publik bahwa JSZip tersebut merupakan bagian wajib dari semua VNM core runtimes.

Perlu direkam:

- versi JSZip,
- stack penuh,
- `typeof Buffer`,
- `typeof Buffer.isBuffer`,
- siapa yang membuat global `Buffer`,
- jenis output yang diminta caller.

Jangan memperbaikinya dengan menambahkan fungsi dummy sebelum tahu apakah caller membutuhkan semantic Buffer yang lebih luas.

### 3. `r.setResizable is not a function`

NW.js mendokumentasikan:

```js
Window.setResizable(resizable)
```

Error:

```text
r.setResizable is not a function
```

cocok dengan dua keadaan utama:

- `r` bukan NW Window yang diharapkan, atau
- shim `window.nw` memilih kode desktop tetapi hanya menyediakan sebagian kontrak.

Ini memperkuat prinsip bahwa capability yang belum benar-benar diimplementasikan sebaiknya tidak dipresentasikan sebagai object truthy.

Masih perlu dilihat:

- assignment `r`,
- stack pemanggilan,
- apakah asalnya `nw.Window.get()`,
- wrapper internal VNM,
- atau plugin game.

### 4. `AudioScheduledSourceNode: cannot call stop without calling start first`

Web Audio specification mendukung bahwa `stop()` pada source yang belum pernah `start()` adalah invalid operation.

Pesan lokal:

```text
AudioScheduledSourceNode: cannot call stop without calling start first
```

mempunyai dasar API yang jelas. Yang belum diketahui adalah mengapa lifecycle VNM mencapai kondisi itu.

Hipotesis yang masih terbuka:

- start dibatalkan sebelum eksekusi,
- `AudioContext` masih suspended,
- source sudah di-dispose/replaced,
- skip/dialog transition menjalankan cleanup pada node yang belum pernah dimulai.

Runtime diagnostics minimum:

```text
created
decoded
start-request
started
stop-request
ended
disposed
```

Sertakan context state dan identity/generation ID per source.

### 5. `AudioNode.connect: Overload resolution failed`

`AudioNode.connect()` memerlukan destination yang sesuai signature Web Audio.

Pesan:

```text
AudioNode.connect: Overload resolution failed
```

lebih langsung menunjuk kepada destination yang bentuk/type-nya tidak cocok, seperti `null`, `undefined`, wrapper, atau object yang bukan `AudioNode`.

Cross-`AudioContext` tetap perlu diuji, tetapi belum terbukti sebagai akar masalah.

Data yang dibutuhkan:

- constructor/type source dan destination,
- `source.context === destination.context`,
- context ID/state,
- stack tepat sebelum `connect()`.

### 6. `Component_MessageBehavior.voiceDuration`

Simbol ini memang ada dalam API resmi.

`voiceDuration()` didefinisikan sebagai durasi voice yang terkait dengan message dalam frame, atau `0` bila tidak ada voice. Properti `voice` didokumentasikan bertipe `gs.AudioBufferReference`.

Source publik untuk body `Component_MessageBehavior.voiceDuration()` belum ditemukan.

Untuk:

```text
Cannot read properties of null (reading 'buffer')
```

hipotesis yang masuk akal:

- reference/buffer belum tersedia,
- gagal load/decode,
- sudah di-dispose,
- race condition terhadap dialog transition.

Fakta bahwa Skip kadang melewati masalah hanya menunjukkan perubahan sequence/timing, bukan bukti akar masalah.

### 7. Overlay default berjalan, toggle/editor berkorelasi dengan freeze

VNM mendokumentasikan bahwa touch dapat menjadi mouse input dan engine memiliki focus lifecycle.

Dua kelas masalah yang layak diuji:

- event ganda,
- focus/state leakage.

Satu touch pada region overlay dapat:

1. menghasilkan command virtual gamepad dari host, dan
2. lolos ke WebView lalu menjadi VNM mouse click.

Membuka editor dapat menghilangkan focus atau memutus pointer stream sebelum release/cancel diterima engine.

Korelasi dengan toggle/editor belum membuktikan overlay sebagai penyebab freeze.

Timeline diagnosis sebaiknya menggabungkan:

- Android `MotionEvent`,
- event DOM,
- virtual key injection,
- logical input state VNM,
- focus callbacks,
- heartbeat `requestAnimationFrame`.

### 8. OTS dan `Rubik-Regular.WOFF`

VNM resmi mendukung font WOFF.

Karena Chromium OTS memvalidasi WOFF/WOFF2, log OTS lebih tepat ditafsirkan sebagai:

> byte stream yang sampai pada font sanitizer tidak diterima sebagai font valid.

Itu belum menentukan penyebab.

Kandidat:

- file corrupt/truncated,
- URL menghasilkan body yang salah,
- loader mengembalikan payload yang belum diproses dengan benar,
- bytes berbeda dari desktop.

Perbandingan yang diperlukan:

- hash/size/magic bytes,
- response path,
- MIME type,
- hasil `FontFace`,
- hasil `document.fonts`,
- line metrics setelah font aktif.

### 9. Aset diduga terenkripsi

“Encrypt Content” memang merupakan fitur packaging VNM.

Tetapi keberadaan file di directory hanya membuktikan package inventory. Bukan bukti bahwa:

- manifest dapat ditafsirkan,
- payload berhasil diproses loader,
- image dapat di-decode,
- font lolos OTS,
- audio dapat di-decode,
- resource benar-benar dapat digunakan engine.

Adapter sebaiknya mengobservasi jalur loader runtime yang sah, bukan membuat mekanisme bypass DRM.

### 10. Berhasil mencapai chapter 3

Ini merupakan smoke evidence yang berguna karena membuktikan sejumlah scene, resource, dialog dan transition sudah berjalan.

Namun belum menguji:

- durable save,
- voice edge case,
- overlay transition berulang,
- process death,
- seluruh branch,
- long-running resource lifecycle.

Karena itu chapter 3 belum cukup untuk label “stable”.

---

## E. Perbandingan Tiga Pendekatan Port

### A. Ekspor Android/Web resmi jika project sumber tersedia

Pilihan dengan surface compatibility terkecil.

Kelebihan:

- lebih sedikit shim,
- lebih selaras dengan browser/mobile branch VNM,
- risiko Node/NW API lebih rendah.

Keterbatasan:

- membutuhkan project sumber,
- custom extension dapat tetap mempunyai asumsi desktop,
- dokumentasi Android resmi bersifat legacy.

Untuk host Android modern, output Web lebih masuk akal sebagai payload WebView daripada mempertahankan Cordova/JDK7 sebagai fondasi.

### B. Adapter khusus VNM untuk game PC yang sudah diekspor

Pilihan paling realistis jika hanya ekspor PC yang tersedia.

Adapter harus dimulai dengan runtime fingerprint, bukan stub API.

Identifikasi terlebih dahulu:

- NW.js era,
- Electron era,
- browser-like,
- package yang sudah dimodifikasi.

Lalu implementasikan kontrak kecil yang benar-benar reachable:

- storage,
- window,
- selected Node primitives,
- asset resolution,
- audio lifecycle,
- input/focus,
- viewport/display.

Kelebihan:

- surface dapat dikontrol,
- mudah diinstrumentasi,
- dapat diuji sesuai perilaku game nyata.

Keterbatasan:

- native Node modules,
- native NW/Electron APIs,
- executable helper,
- packaging khusus

dapat membuat suatu game tidak feasible dalam WebView tanpa project sumber.

### C. Lapisan kompatibilitas generik NW.js/Node.js

Pendekatan paling luas dan paling berisiko.

Surface minimal dapat mencakup:

```text
require
module resolution
process
Buffer
fs
path
filesystem semantics
window APIs
lifecycle/events
native-module assumptions
```

Masalah tambahan: sejak VNM 1.0.1111 desktop runtime resmi berpindah ke Electron.

Karena itu generic NW.js layer sebaiknya hanya menjadi library pendukung adapter VNM, bukan abstraction utama proyek.

### Rekomendasi

Urutan pilihan:

1. A jika project sumber tersedia.
2. B untuk existing PC export.
3. C hanya sebagai komponen pendukung B.

---

## F. Usulan Batas Adapter VNM

### 1. Runtime Fingerprint

Sebelum memasang shim:

- identifikasi `package.json`/NW runtime,
- Electron artifacts,
- scripts VNM,
- build configuration,
- `process.versions.nw`,
- `process.versions.electron`,
- native/global APIs yang benar-benar dipanggil.

Capability yang tidak tersedia jangan diekspos sebagai truthy.

### 2. Storage Boundary

Adapter storage harus menjamin persistence, bukan hanya keberhasilan pemanggilan.

Domain harus dipisahkan:

```text
slot save
header/thumbnail
globalData
settings
```

Keberhasilan settings/global data tidak membuktikan slot save valid.

Strategi Android:

- origin WebView yang stabil,
- browser storage yang konsisten, atau
- native durable storage,
- atomic write,
- cold-start verification,
- recovery setelah interrupted write/process death.

`WebViewAssetLoader` layak digunakan untuk memberi local content origin HTTPS-style yang stabil.

Jika native persistence digunakan, `AtomicFile` dapat menjadi referensi desain transactional write.

### 3. Window Boundary

Implementasikan hanya fungsi yang benar-benar dipanggil, misalnya:

```text
nw.Window.get()
setResizable()
fullscreen
size
window events
```

Desktop-only behavior yang tidak relevan di Android dapat dimapping ke host equivalent atau semantic no-op hanya jika caller tidak membutuhkan state tertentu.

### 4. Audio Boundary

Gunakan satu lineage `AudioContext` yang jelas.

Catat:

```text
loaded
decoded
duration
context ID/state
creation
start
stop
ended
dispose
dialog ID
voice ID
```

Gunakan generation/token discipline supaya callback `onended` dari node lama tidak mengubah state object baru setelah skip/scene transition/dispose.

Uji:

- user gesture/autoplay,
- pause/resume,
- codec,
- rapid dialog transition,
- voice stop/restart.

### 5. Asset Boundary

Gunakan origin dan URL mapping deterministik.

Rekam:

- exact path,
- case,
- MIME type,
- response status,
- payload size/hash.

Bukti resource harus sampai consumer level:

- image: decode + render,
- font: accepted + applied,
- audio: decode + playback,
- data/manifest: parse,
- encrypted payload: diproses loader legitimate.

### 6. Input Boundary

Overlay tetap dipertahankan.

Untuk pointer yang mulai di control overlay, host sebaiknya memiliki stream itu sampai release/cancel.

Pada:

- `ACTION_CANCEL`,
- overlay editor open/close,
- focus loss,
- app background,
- WebView focus transition,

adapter harus menjamin semua virtual key/pointer states dilepas secara deterministik.

### 7. Display Boundary

Android host sebaiknya memiliki:

- orientation policy,
- immersive mode,
- window lifecycle.

VNM tetap mengelola:

- logical canvas resolution,
- aspect ratio,
- scaling.

Setiap resize harus diuji terhadap:

- CSS viewport,
- canvas backing size,
- device pixel ratio,
- `gs.Graphics`,
- overlay coordinate transform.

---

## G. Checklist Acceptance

### Dialog dan Voice

- dialog normal bekerja tanpa Skip,
- dialog voiced/unvoiced,
- choice,
- backlog,
- auto mode,
- rapid transition,
- tidak ada:
  - `Cannot read properties of null (reading 'buffer')`
  - `AudioScheduledSourceNode: cannot call stop without calling start first`
  - `AudioNode.connect: Overload resolution failed`

### Save/Load

- save slot 1,
- save slot 2,
- save slot 3,
- state tiap slot berbeda,
- verify header/thumbnail jika ada,
- Android force-stop,
- process death,
- relaunch dari proses baru,
- load semua slot,
- verify scene,
- verify variables,
- verify branch state,
- overwrite,
- interrupted write,
- uji `globalData` terpisah,
- uji settings terpisah.

### Overlay

Uji berulang:

- toggle overlay,
- buka/tutup editor,
- idle,
- saat dialog,
- saat choice,
- control ditahan,
- multitouch,
- pointer cancel,
- focus transition.

Acceptance:

- tidak ada duplicate action,
- tidak ada stuck key,
- focus pulih,
- tidak ada freeze.

### Background/Resume

Uji ketika:

- idle,
- animasi,
- voice,
- dialog,
- overlay interaction.

Tambahkan:

- screen lock/unlock,
- focus loss,
- resume setelah audio aktif.

### Font

- `Rubik-Regular.WOFF` tidak ditolak OTS,
- line wrap benar,
- tidak ada clipping,
- baseline sesuai,
- fallback terdeteksi bila terjadi.

### Asset

Untuk representative asset:

- image decode/render,
- audio decode/play,
- font accepted/applied,
- manifest parse,
- encrypted resource berhasil melalui loader legitimate.

### Gameplay Panjang

- lebih jauh dari chapter 3,
- branching,
- save/load berkala,
- beberapa voice-heavy scene,
- long-play/soak test,
- pantau memory/resource accumulation.

### Device Matrix

Minimal lebih dari satu:

- Android version,
- WebView/Chromium version,
- GPU profile,
- memory profile.

---

## H. Pertanyaan yang Hanya Bisa Dijawab dengan Inspeksi GOS-01 atau Pengujian Android

1. Runtime exact GOS-01 apa?
2. Apakah `GOS-01` punya makna resmi/internal?
3. Apa decoded `$PARAMS`?
4. Apa semantic aktual `isOffline`?
5. Bagaimana `gs.GameStorage` memilih backend?
6. Apa file/key untuk slot 1/2/3?
7. Bagaimana header/thumbnail/global/settings disimpan?
8. Apakah serialization melibatkan compression/encryption/custom object format?
9. Versi JSZip apa yang digunakan?
10. Siapa caller `Buffer.isBuffer`?
11. Dari mana object `r` pada `r.setResizable` berasal?
12. Apa body aktual `Component_MessageBehavior.voiceDuration`?
13. Kapan `gs.AudioBuffer` dibuat/didecode/dispose?
14. Apakah lebih dari satu `AudioContext` ada?
15. Apakah bytes font Android sama dengan desktop?
16. Apakah resource encrypted dapat diproses di pure WebView?
17. Path/case/MIME apa yang diminta runtime?
18. Siapa yang memiliki focus ketika overlay editor dibuka?
19. Saat freeze, apakah `requestAnimationFrame` masih jalan?
20. Apakah durable save bertahan setelah OS membunuh process?
21. Apakah ada custom extension/native Node module yang menjadi blocker?

---

## I. Bibliografi Beranotasi

### 1. Visual Novel Maker — Engine Getting Started

Dokumentasi resmi Basic Engine. Sumber utama untuk memahami bahwa runtime inti VNM adalah HTML5 dan mencakup graphics, video, audio serta input.

https://asset.visualnovelmaker.com/help/Engine_Getting_Started.htm

### 2. Visual Novel Maker — Create Game Package

Sumber resmi untuk anatomi export: `index.html`, `package.json`, data/resources, encryption dan NW.js pada desktop lama.

https://asset.visualnovelmaker.com/help/Create_Game_Package.htm

### 3. Visual Novel Maker — Update History / Announcements

Sumber penting untuk boundary versi VNM 1.0.1111 dan perpindahan NW.js ke Electron.

https://steamcommunity.com/app/495480/announcements/

### 4. `gs.GameStorage` API

Referensi utama persistence abstraction VNM.

https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.GameStorage.html

### 5. `GameManager` API

Menunjukkan pemisahan slot save, global data, dan settings.

https://asset.visualnovelmaker.com/help/game_script_doc/classes/GameManager.html

### 6. `Component_MessageBehavior` API

Mendokumentasikan `voiceDuration()` dan properti voice terkait.

https://asset.visualnovelmaker.com/help/game_script_doc/classes/Component_MessageBehavior.html

### 7. `gs.AudioBuffer` API

Referensi load/decode/duration/dispose audio.

https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.AudioBuffer.html

### 8. Visual Novel Maker — For Android

Dokumentasi deployment Android legacy berbasis Cordova/Node/JDK.

https://asset.visualnovelmaker.com/help/For_Android.htm

### 9. Visual Novel Maker — System

Referensi konfigurasi storage dan target Web/Mobile.

https://asset.visualnovelmaker.com/help/System.htm

### 10. `gs.Input`

Referensi unified keyboard/mouse/touch/gamepad input.

https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.Input.html

### 11. `gs.MouseInput`

Referensi touch-to-mouse behavior.

https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.MouseInput.html

### 12. `gs.Graphics`

Referensi canvas, focus, aspect ratio, dan display behavior.

https://asset.visualnovelmaker.com/help/engine_doc/classes/gs.Graphics.html

### 13. Android `WebViewAssetLoader`

Referensi Android modern untuk local content dengan origin HTTPS-style.

https://developer.android.com/develop/ui/views/layout/webapps/load-local-content

### 14. AndroidX `AtomicFile`

Referensi desain atomic write untuk native persistence bridge.

https://developer.android.com/reference/androidx/core/util/AtomicFile.html

### 15. NW.js Window API

Referensi kontrak `setResizable()` dan API window lain.

https://docs.nwjs.io/References/Window/

### 16. Web Audio API Specification

Referensi normatif lifecycle `AudioScheduledSourceNode`.

https://www.w3.org/TR/webaudio/

### 17. MDN `AudioNode.connect()`

Referensi signature dan error umum `connect()`.

https://developer.mozilla.org/en-US/docs/Web/API/AudioNode/connect

### 18. Chromium Autoplay Policy

Referensi suspended `AudioContext` dan user gesture.

https://developer.chrome.com/blog/autoplay/

### 19. Visual Novel Maker Resource Standards

Referensi format resource termasuk font WOFF.

https://asset.visualnovelmaker.com/help/Resource_Standards.htm

### 20. Chromium OpenType Sanitizer

Referensi validasi WOFF/WOFF2 oleh Chromium.

https://chromium.googlesource.com/external/github.com/khaledhosny/ots/+/HEAD/docs/DesignDoc.md

### 21. JSZip `generateAsync()`

Referensi output `nodebuffer`.

https://stuk.github.io/jszip/documentation/api_jszip/generate_async.html

### 22. Node.js Buffer API

Referensi `Buffer.isBuffer()`.

https://nodejs.org/api/buffer.html

---

## Kesimpulan Desain

Batas paling penting untuk eksperimen berikutnya bukan “berapa banyak NW.js yang bisa di-stub”, tetapi **runtime detection yang benar dan branch platform yang konsisten**.

Untuk GOS-01, tiga hal sebaiknya dibuktikan sebelum ada patch baru:

1. fingerprint exact runtime/export generation,
2. complete trace keputusan storage dari `$PARAMS`/`window.nw` sampai durable write,
3. trace lifecycle voice dari `Component_MessageBehavior` sampai native `AudioScheduledSourceNode`.

Setelah itu baru layak menentukan apakah GOS-01 dapat menggunakan semantics browser/mobile sepenuhnya atau membutuhkan profile NW-era khusus.

Overlay tidak perlu dihilangkan. Desain yang lebih tepat adalah menjadikannya input owner yang terisolasi dan memasukkan focus/cancel/release sebagai bagian dari kontrak adapter.

Status saat ini:

**Feasibility plausible, tetapi stabilitas belum terbukti.**

Bukti mencapai chapter 3 berguna sebagai smoke evidence, tetapi save setelah process death, dialog normal/voice, overlay transition, font, lifecycle, dan long-play masih merupakan blocker validasi utama.
