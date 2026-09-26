# Technical Findings

## [NTFS Alternate Data Stream & XML SAXParseException on Game Titles with Colons and Ampersands]
**Masalah:**
1. Gradle task `:app:processDebugMainManifest` gagal dengan: `com.android.manifmerger.ManifestMerger2$MergeFailureException: Error parsing AndroidManifest.xml: org.xml.sax.SAXParseException: The entity name must immediately follow the '&' in the entity reference`.
2. Pada Windows, kompilasi game dengan nama seperti `"Fear & Hunger 2: Termina"` menghasilkan file 0-byte bernama `Fear&Hunger2` tanpa ekstensi APK pada direktori output.
**Penyebab:**
1. String `app_name` diinjeksi mentah ke dalam template XML `AndroidManifest.xml` (`android:label="{app_name}"`). Dalam format XML, karakter `&` yang tidak diikuti entity reference langsung memicu fatal syntax error pada parser XML xerces/position parser Android.
2. Pada Windows NTFS, karakter titik dua `:` merupakan pemisah Alternate Data Stream (ADS). Menulis file ke `Fear&Hunger2:Termina.apk` tidak membuat file `.apk`, melainkan menulis payload APK ke dalam stream data alternatif bernama `:Termina.apk` di dalam metadata file `Fear&Hunger2`, sehingga file utama terlihat berukuran 0 byte dan APK tidak dapat ditemukan oleh alat standar.
**Solusi/workaround:**
1. Pasang helper `escape_xml_attribute` di `AndroidProjectGenerator` untuk mengonversi `&` menjadi `&amp;`, `<` menjadi `&lt;`, `>` menjadi `&gt;`, `"` menjadi `&quot;`, dan `'` menjadi `&apos;` sebelum dimasukkan ke dalam `AndroidManifest.xml`.
2. Pasang fungsi `derive_apk_filename` di `cli/src/main.rs`: jika nama game memiliki tanda colon `:`, dan direktori output menargetkan subtitle tersebut (seperti `dist/termina`), gunakan `Termina.apk`. Untuk kasus umum, gantikan seluruh karakter ilegal filesystem (`/`, `\`, `:`, `*`, `?`, `"`, `<`, `>`, `|`) menjadi `_` dan bersihkan spasi agar aman di lintas platform.
**Tags:** #xml #saxparseexception #manifestmerger #ntfs #alternate-data-stream #filename-sanitization #windows

## [Bitmap.new Exception Recovery in Ruby VM Swallows Cache.normal_bitmap Rescue Blocks]
**Masalah:** Saat menguji `Cache.normal_bitmap` untuk missing asset fallback, event fallback `TILESET_FALLBACK_DUMMY` atau rescue handler di `Cache.normal_bitmap` tidak terpanggil, melainkan selalu jatuh ke `PLACEHOLDER_SUBSTITUTED` generic dari `Bitmap.new`.
**Penyebab:** Pada Ruby monkey-patching, `Bitmap.new` di-override untuk me-rescue `Errno::ENOENT` dan mengembalikan dummy bitmap (32x32). Ketika `Cache.normal_bitmap` memanggil `Bitmap.new(path)` di dalam implementasinya, error `Errno::ENOENT` sudah tertangkap di level `Bitmap.new` sehingga `Cache.normal_bitmap` tidak pernah menerima exception dan tidak menjalankan blok `rescue` miliknya.
**Solusi/workaround:** Pasang penanganan spesifik path (`Graphics/Tilesets/`) pada KEDUA level: baik di `Bitmap.new` (agar instansiasi langsung maupun via Cache menghasilkan fallback 512x384 checkered bitmap dan event `TILESET_FALLBACK_DUMMY`) maupun di `Cache.normal_bitmap` (agar implementasi cache yang tidak memakai `Bitmap.new` atau me-raise secara langsung tetap tertangani).
**Tags:** #ruby #bitmap-new #cache-normal-bitmap #tileset-fallback #monkey-patching

## [DOM Mutation on PointerDown Causes Ghost Clicks & Multi-Panel Bridge Conflict]
**Masalah:** 
1. Saat tombol collapse (`─` / `≡`) pada editor bar diklik, tombol `Done` (yang baru muncul/bergeser ke posisi koordinat jari yang sama) ikut tertekan seketika sehingga editor mode langsung tertutup atau tap tembus ke elemen di belakangnya.
2. Saat membuka beberapa panel (misalnya Settings Editor, lalu membuka Diagnostic Suite), menutup Diagnostic Suite langsung memanggil `setOverlayModalOpen(false)` sehingga touch bridge native menganggap tidak ada modal aktif dan sentuhan pada Settings Bar bocor ke canvas game.
3. Tombol aktif pada tema Minimalist Matte berkedip (flicker) dari biru `#1e3a8a` menjadi abu-abu `#27272a` saat ditekan.
**Penyebab:** 
1. `bindTap` memicu aksi pada `pointerdown`. Ketika aksi memutasi DOM (menyembunyikan atau menampilkan tombol di bawah posisi fisik jari), browser melanjutkan siklus gesture fisik dan menembakkan `pointerup` dan synthetic `click` pada elemen baru yang kini berada di bawah jari (`doneBtn`).
2. Tidak ada agregator status modal terpusat; setiap modal memanggil bridge `setOverlayModalOpen` secara mandiri tanpa mengetahui apakah modal lain masih terbuka.
3. `vpad.css` tema matte memiliki selektor `body.vpad-theme-matte .vpad-top-btn:active` dengan `background: #27272a !important;` yang menimpa warna `.active`.
**Solusi/workaround:** 
1. Ubah `bindTap` agar memicu aksi pada `pointerup` (setelah verifikasi pergerakan jari dx/dy <= 10px) dan gunakan `setPointerCapture` serta penekanan synthetic click selama 500ms. Tambahkan interaction guard: saat collapse ditoggle, matikan `pointer-events: none` selama 150ms pada tombol yang baru terungkap.
2. Bangun registry modal terpusat `window.RuntimePackModalRegistry` (`Set`) di `vpad.js` dengan helper `setModalState(id, isOpen)`. Panggil `setOverlayModalOpen(true)` selama `registry.size > 0` dan hanya panggil `false` saat `registry.size === 0`.
3. Ganti aturan `:active` tema matte agar hanya melakukan scale kompresi tactile (`transform: scale(0.88) !important;`) sambil mempertahankan warna aktif `#1e3a8a` / `#3b82f6`.
**Tags:** #ghost-click #pointer-events #modal-registry #touch-bleed #vpad #matte-theme

## [Native WebActivity Touch Routing Intercepts Dynamic / Hoisted Overlay DOM Elements]
**Masalah:** Saat modal atau panel overlay (seperti Custom Control Setup `+ Add Button`, panel editor, tombol collapse/minimize `─`/`≡`) disentuh di layar, tombol `✕` (Close) dan `Cancel` sama sekali tidak merespons, dan game di latar belakang justru menerima sentuhan.
**Penyebab:** 
1. Di `WebActivity.kt`, `overlayWebView` berukuran full-screen transparan di atas `gameWebView`. Untuk membiarkan sentuhan game lewat, `dispatchTouchEvent` memeriksa posisi koordinat fisik jari terhadap rectangle statis:
   - Toolbar: `70dp × 260dp` di pojok kiri atas.
   - Modal: rectangle statis yang diasumsikan selalu berada di tengah layar (`min(92%, 520dp) × 86%`).
2. Begitu panel overlay (seperti Editor Bar, Inspector, Popovers, dan Key Modal) di-hoist langsung ke `document.body` dan dapat digeser (drag) atau berukuran full-screen, titik sentuh tombol Close `✕` dan `Cancel` jatuh di LUAR rectangle statis yang diasumsikan Kotlin. Akibatnya, `shouldForwardTouchToGame` mengembalikan `true` dan seluruh event gesture (`ACTION_DOWN`, `MOVE`, `UP`) dialihkan langsung ke `gameWebView` di bawahnya.
**Solusi/workaround:** 
Di `WebActivity.kt`, jadikan status `isOverlayModalOpen` sebagai penentu mutlak: saat mode modal/editor aktif (`isOverlayModalOpen == true`), seluruh gesture sentuh di seluruh layar dipaksa menjadi milik `overlayWebView` (`shouldForwardTouchToGame` mengembalikan `false` untuk semua koordinat) persis seperti arsitektur `RgssActivity.kt`, dan kunci `isTouchCapturedByGame = false`.
**Tags:** #touch-routing #webactivity #hit-test #modal #overlay #android

## [Listener Accumulation & Synthetic Event Duplication pada Virtual Controller DOM]
**Masalah:** Sentuhan tombol toolbar terasa ada delay atau tidak merespons (harus ditekan beberapa kali), dan semakin lama game dimainkan saat berpindah preset/mengedit tombol, interaksi overlay semakin berat dan lambat.
**Penyebab:** 
1. `renderControls()` membersihkan kontainer dengan `innerHTML = ""`, tetapi seluruh listener `window.touchmove`, `window.touchend`, `window.mousemove`, `window.mouseup` yang dipasang oleh `setupDirectManipulation`, D-Pad, dan Joystick tidak pernah di-`removeEventListener`. Setiap kali ganti preset atau re-render, puluhan listener baru menumpuk di `window` (dari 13 naik hingga 85+ listener) yang semuanya dieksekusi di setiap frame gerakan jari.
2. `bindTap` lama mengikat 4 event sekaligus (`touchstart`, `mousedown`, `pointerdown`, `click`). Di WebView modern, satu sentuhan fisik memicu rangkaian event kompatibilitas secara beruntun, sehingga callback dieksekusi berulang kali atau terblokir oleh debounce window yang kaku.
3. `cheat_menu.js` menjalankan inisialisasi ganda dari event DOM-ready dan window `load`, dan menempelkan listener tambahan pada `#vpad-btn-cheat` yang sudah dimiliki oleh `vpad.js`.
**Solusi/workaround:**
1. Sebelum `container.innerHTML = ""` dijalankan di `renderControls()`, eksekusi cleanup array untuk mencopot seluruh window listener dari kontrol lama.
2. Gunakan satu jalur otoritartif Pointer Events pada `bindTap` dengan penekanan event sintetis kompatibilitas, tanpa menggunakan debounce berbasis waktu yang membuang tap sah berikutnya.
3. Jadikan `vpad.js` sebagai pemilik tunggal aktivasi tombol Cheat di toolbar, dan pastikan inisialisasi `cheat_menu.js` sepenuhnya idempotent sebelum mengikat listener apa pun.
**Tags:** #vpad #event-leak #touch-latency #pointer-events #cheat-menu

## [Touch Routing Bounding Box Leak pada WebActivity]
**Masalah:** Saat modal atau panel Custom Control/Editor overlay dibuka pada `WebActivity`, sentuhan di luar area persegi panjang modal terpusat (misal sudut layar, panel editor yang digeser, atau tombol di luar area tengah) tembus dan memicu aksi di `gameWebView` di bawahnya.
**Penyebab:** `WebActivity.shouldForwardTouchToGame` mengecek `isTouchInsideModal(...)` yang menghitung bounding box statis 520dp terpusat. Titik di luar box tersebut dievaluasi sebagai bukan kontrol aktif (`isTouchOnActiveControls == false`) sehingga `shouldForwardTouchToGame` mengembalikan `true`.
**Solusi/workaround:** Jika `isOverlayModalOpen == true`, bypass pengecekan box statis dan langsung kembalikan `false` dari `shouldForwardTouchToGame` (dan `true` dari `isTouchOnActiveControls`). Gunakan `resolveGestureCapturedByGame` untuk mempertahankan status gesture sejak `ACTION_DOWN` hingga `ACTION_UP`/`ACTION_CANCEL`.
**Tags:** #touch-routing #modal #webactivity #android #gesture-ownership

## [Duplicate Tap, Startup Binding, dan Listener Leak pada Overlay]
**Masalah:** Satu tap fisik memicu callback toolbar empat kali, startup CheatMenu berulang menambah listener, rerender preset menaikkan listener window dari 13 menjadi 85, dan popover tertutup editor bar yang sudah dinaikkan.
**Penyebab:** `bindTap` mengikat pointer/touch/mouse/click sekaligus; `initCheatDOM` memeriksa idempotensi setelah binding dan ikut mengikat tombol milik VPad; setiap control memasang closure permanen ke window tanpa cleanup; popover memakai z-index statis.
**Solusi/workaround:** Gunakan Pointer Events sebagai jalur utama dengan compatibility-click suppression berbasis urutan event, fallback ke satu touch/click path; cek overlay sebelum binding dan biarkan VPad memiliki tombol existing; simpan cleanup listener control lalu jalankan sebelum `innerHTML` rerender; panggil elevasi panel saat membuka popover.
**Tags:** #pointer-events #tap #listener-leak #idempotency #z-index #overlay

## [Pointer Events & 1:1 Pixel Dragging pada Floating Modal Overlay]
**Masalah:** Dragging modal Diagnostic Suite dan Cheat Menu tersendat atau terputus saat digeser dengan cepat pada layar sentuh mobile, serta posisi melompat (jumping offset) saat pertama kali disentuh jika masih memiliki CSS `transform: translate(-50%, -50%)`.
**Penyebab:** 
1. Drag lama menggunakan passive touch listener dan `doc.addEventListener("touchmove")` yang bisa tertahan oleh scroll gesture browser atau dicegat `e.stopPropagation()` pada modal root.
2. Centering awal modal mengandalkan `top: 50%; left: 50%; transform: translate(-50%, -50%)`. Jika `left` diupdate tanpa menghapus translate offset, pergerakan mouse/touch menjadi offset 50% atau melompat.
**Solusi/workaround:** 
1. Gunakan modern Pointer Events (`pointerdown` dengan `header.setPointerCapture(e.pointerId)`, `pointermove`, `pointerup`, `pointercancel`) serta deklarasikan `touch-action: none` pada header.
2. Saat drag dimulai, konversi `modal.getBoundingClientRect()` ke koordinat pixel absolut `initLeft`/`initTop`, set `modal.style.transform = "none"`, sehingga delta perpindahan 1:1 langsung memodifikasi `left` dan `top` secara presisi.
3. Hapus `touchmove`/`mousemove` dari pemblokir `e.stopPropagation()` di modal root agar pointer tracking tetap diterima event loop.
**Tags:** #touch-events #pointer-events #modal-drag #vpad #diagnostic-suite #cheat-menu

## [GPU Compositor Overhead & SVG CurrentColor Contrast pada Mobile WebView]
**Masalah:** 
1. Ikon toolbar `.vpad-top-btn` SVG dengan `stroke: currentColor` tampil hitam pekat (user-agent default) sehingga tidak kontras dengan background gelap.
2. Multiple nested `backdrop-filter: blur(16px)` pada tombol-tombol di dalam modal memicu frame drops pada GPU compositor mobile.
3. Logging intensif memicu synchronous reflow berulang akibat `box.scrollTop = box.scrollHeight`.
**Solusi/workaround:** 
1. Berikan `color: #f1f5f9 !important;` eksplisit pada `.vpad-top-btn` dan `color: #f8fafc;` pada `html, body`.
2. Ringankan backdrop modal menjadi `rgba(15, 23, 42, 0.94)` dengan `backdrop-filter: blur(8px)`, serta nonaktifkan duplicate blur pada tombol di dalam modal (`backdrop-filter: none !important;`).
3. Throttle auto-scroll console via `requestAnimationFrame` saat modal terbuka.
**Tags:** #css #svg #gpu-compositor #performance #reflow
**Masalah:** Saat panel Settings atau Inspector dinaikkan z-index-nya, tombol gameplay (A, B, X, Y, D-Pad) ikut terangkat ke atas menembus dan menutupi jendela modal Cheat Engine dan Diagnostic Suite.
**Penyebab:** Seluruh elemen panel sebelumnya berada di dalam container `#vpad-root` yang juga menaungi `#vpad-controls`. Saat z-index `#vpad-root` dinaikkan agar panel settings bisa di atas modal, seluruh tombol di dalam `#vpad-controls` ikut terangkat melompati modal.
**Solusi/workaround:** Pindahkan panel mengambang (`#vpad-editor-bar`, `#vpad-prop-panel`, `#vpad-key-modal`, `#vpad-preset-popover`, `#vpad-theme-popover`) langsung menjadi anak dari `document.body`, dan kunci `#vpad-root` pada z-index dasar (`99990`) di bawah z-index panel (`100000+`). Dengan demikian, gameplay controls tidak akan pernah menonjol di atas modal, dan setiap panel dapat dinaikkan z-index-nya secara mandiri saat disentuh.
**Tags:** #overlay #z-index #stacking-context #vpad #ui

## [Touch Event Handling & Resilient DOM Ingestion pada Overlay Toolbar]
**Masalah:** Tombol toolbar overlay dinamis (`#vpad-btn-diag`) gagal merespons touch input atau hilang saat `vpad.js` me-render ulang template DOM overlay.
**Penyebab:** 
1. Di Android WebView dengan kontrol sentuh virtual, touch listener yang hanya mengikat event `touchend` dan `click` rentan tidak tertrigger jika `e.preventDefault()` dipanggil pada `touchstart` atau jika pointer bergerak lebih dari ambang batas toleransi sentuhan.
2. `vpad.js` merender `root.innerHTML` pada event `DOMContentLoaded`, yang berpotensi menimpa elemen DOM yang disisipkan lebih dulu oleh script overlay lain jika keduanya terdaftar pada event load yang sama.
**Solusi/workaround:** 
1. Masukkan `#vpad-btn-diag` langsung ke dalam template `#vpad-topbar` di `vpad.js` serta pasang polling check timer di `diagnostic_suite.js` untuk menjamin ketersediaan elemen secara persisten.
2. Di `attachTap`, tangani seluruh spektrum event (`touchstart`, `touchend`, `mousedown`, `pointerdown`, `click`) dengan debounce timestamp 250ms.
3. Sediakan broadcast receiver `io.runtimepack.EVAL` pada `RgssActivity.kt` untuk otomasi pengujian evaluasi JavaScript non-interaktif secara deterministik.
**Tags:** #vpad #diagnostic-suite #touch-event #overlay #dom-injection

## [MIME Charset Encoding Mismatch pada Binary Assets di WebResourceResponse]
**Masalah:** Gambar (`.png`, `.gif`) dan font (`.woff`, `.ttf`) gagal di-decode oleh Chromium WebView atau logcat menampilkan `Image decoding logging dropped!`, menyebabkan elemen visual canvas/DOM tetap kosong/hitam.
**Penyebab:** Konstruktor `WebResourceResponse(mime, encoding, data)` menerima argumen `encoding = "UTF-8"`. Untuk tipe data biner (gambar, audio, font), spesifikasi Android WebView mengharuskan `encoding = null`. Jika diisi `"UTF-8"`, Chromium mencoba membaca stream byte biner melalui UTF-8 character decoder sehingga byte gambar menjadi rusak.
**Solusi/workaround:** Di `VfsResourceInterceptor.kt`, buat helper `getEncoding(mimeType)` yang hanya mengembalikan `"UTF-8"` untuk resource berbasis teks (`text/*`, `application/javascript`, `application/json`), dan mengembalikan `null` untuk seluruh binary format.
**Tags:** #webview #chromium #vfs #mime #encoding

## [TyranoScript Audio Autoplay Blocking pada Tag Scenario]
**Masalah:** Eksekusi skrip scenario TyranoScript berhenti di tag audio awal (misal `[playbgm storage=title2.mp3]`) tanpa melanjutkan ke tag gambar title berikutnya (`[image storage="title03.gif"]`).
**Penyebab:** Pada browser standar, TyranoScript memeriksa `$.isNeedClickAudio()`. Jika bernilai `true` (default non-desktop), Tyrano memanggil `this.waitClick(pm)` yang menahan alur skrip KAG sampai ada gesture sentuhan fisik dari user untuk membuka izin WebAudio browser.
**Solusi/workaround:** Di `runtimepack_web_shims.js`, patch method `$.isNeedClickAudio` agar mengembalikan `false` dan setel `window.TYRANO.kag.tmp.ready_audio = true`, memanfaatkan konfigurasi Android WebView `mediaPlaybackRequiresUserGesture = false` yang mengizinkan autoplay native.
**Tags:** #tyrano #webaudio #autoplay #user-gesture

## [Undefined process.execPath Crash pada TyranoScript localFilePath]
**Masalah:** TyranoScript crash saat inisialisasi awal dengan error `Uncaught TypeError: Cannot read properties of undefined (reading 'indexOf') at tyrano/libs.js:64:30`.
**Penyebab:** Method `$.localFilePath()` di `tyrano/libs.js` mengecek `process.execPath.indexOf("var/folders") != -1`. Di lingkungan Android WebView, object `process` tidak memiliki properti `execPath`.
**Solusi/workaround:** Di `runtimepack_web_shims.js`, sediakan default `process.execPath = ""` dan `process.env.HOME = ""` pada shim global `process`.
**Tags:** #tyrano #nodejs #process #shims

## [Incompatible Encoding Regexp Match pada Zlib-Inflated Ruby Scripts]
**Masalah:** Skrip RPG Maker VX Ace melempar `Encoding::CompatibilityError: incompatible encoding regexp match (ASCII-8BIT regexp with UTF-8 string)` saat mencocokkan regex literal yang memuat karakter multibyte (seperti fullwidth colon `[\xef\xbc\x9a:]`) terhadap data game ber-encoding UTF-8 (misal `actor.note`).
**Penyebab:** Dekompresi byte skrip via `Zlib::Inflate.inflate` menghasilkan string dengan default encoding `ASCII-8BIT` (binary). Saat di-`eval`, literal regex di dalamnya mewarisi encoding `ASCII-8BIT`. Di Ruby 1.9+, pencocokan regex ASCII-8BIT terhadap string UTF-8 dilarang jika regex memuat byte di atas `0x7F`.
**Solusi/workaround:** Di `runtimepack_bootstrap.rb`, paksa string source skrip menjadi UTF-8 (`script_source.force_encoding("UTF-8")`) dan sisipkan `# encoding: utf-8\n` sebelum `eval`. Tambahkan juga monkey-patch pencegah error pada `Regexp#=~` dan `String#scan`.
**Tags:** #ruby #encoding #utf8 #ascii-8bit #zlib

## [Missing Windows RTP Assets pada Scene_Title Background Layer]
**Masalah:** Game melempar `Errno::ENOENT: No such file or directory - Graphics/Titles2/Mist` saat Title Screen RGSS3 diinisialisasi.
**Penyebab:** Game mendeklarasikan `rtp = RPGVXAce` di `Game.ini` dan memanggil layer bawaan template engine (`Mist`) yang tidak dibundel dalam folder game mandiri melainkan ada pada instalasi RTP Windows Enterbrain.
**Solusi/workaround:** Pasang hook `Bitmap.new` dan `Cache.normal_bitmap` sejak awal bootstrap yang mencegat `Errno::ENOENT` dan mengembalikan empty transparent bitmap (32x32) untuk file grafis yang hilang tanpa menghentikan eksekusi game.
**Tags:** #rgss3 #rtp #mkxp-z #bitmap-fallback #cache

## [Inisialisasi Hook Bootstrap Terhambat Evaluasi Main pada RPG Maker XP]
**Masalah:** Hook input (`patch_input_fallbacks`) dan cheat runner (`RuntimePackCheatRunner.hook_engine`) tidak pernah terpasang pada game RPG Maker XP, menyebabkan input virtual pad tidak merespons.
**Penyebab:** Pada `runtimepack_bootstrap.rb`, hook sebelumnya dipasang pada kondisi `idx == scripts.size - 1`. Pada RPG Maker XP, skrip `Main` (yang berisi loop `while $scene != nil; $scene.main; end`) berada sebelum index terakhir (misal index 122 dari 124), sehingga evaluasi skrip tidak pernah kembali sampai game ditutup. Akibatnya hook tidak sempat dieksekusi.
**Solusi/workaround:** Pasang hook secara eksplisit sebelum skrip `Main` dievaluasi: `if script_name.downcase == 'main' || idx == scripts.size - 1`.
**Tags:** #rgss1 #ruby #main-loop #bootstrap-hooks

## [Uninitialized Constant MKXP pada Port Desktop Game RPG Maker XP]
**Masalah:** Porting game RPG Maker XP desktop mkxp crash saat booting dengan `uninitialized constant StartGame::MKXP (NameError)`.
**Penyebab:** Upstream mkxp desktop menyediakan modul `MKXP` (dengan method `data_directory`, `raw_key_states`, `title`, dll.), sementara `mkxp-z` modern pada Android tidak lagi mengekspos seluruh namespace modul tersebut ke Ruby binding.
**Solusi/workaround:** Tambahkan modul stub `MKXP` di `runtimepack_bootstrap.rb` dengan method `data_directory` mengembalikan `"./."` dan `method_missing` yang mengembalikan nil tanpa exception.
**Tags:** #mkxp #ruby #mkxp-z #data-directory

## [Argument 0: Expected bool pada setter visible= RGSS3]
**Masalah:** Game crash saat inisialisasi script pencahayaan (`460_KHAS_LightLib.rb:1334`) dengan pesan error `Argument 0: Expected bool`.
**Penyebab:** Skrip game berisi typo `@sprite.visible = @sprite`. Di Windows RGSS asli, nilai truthy non-boolean diterima sebagai `true`. Namun implementasi binding C++ `mkxp-z` memeriksa tipe boolean secara ketat.
**Solusi/workaround:** Pasang monkey-patch pada kelas visual RGSS (`Sprite`, `Viewport`, `Plane`, `Window`) di `runtimepack_bootstrap.rb`:
```ruby
def visible=(val)
  self.runtimepack_orig_visible = (val ? true : false)
end
```
**Tags:** #rgss #ruby #mkxp-z #type-coercion

## [SystemStackError: stack level too deep pada JSON Parser]
**Masalah:** Game melempar `SystemStackError: stack level too deep` di `(eval):159:in next_token` saat memuat file palet/pengaturan JSON besar.
**Penyebab:** Library Ruby `101_jsonEnDecoder.rb` mengimplementasikan tokenisasi whitespace (`' '`, `"\n"`, `"\t"`) secara rekursif (`return self.next_token(debug)`). Thread bionic Android memiliki batas stack yang lebih ketat dibanding desktop (~1MB vs 8MB).
**Solusi/workaround:** Modifikasi method `next_token` di `GpackExtractor.kt` menjadi loop iteratif (`loop do ... end`) sehingga deretan spasi/baris baru tidak menambah kedalaman call stack.
**Tags:** #ruby #stack-overflow #json #bionic

## [Missing File karena Case-Sensitivity Filesystem Linux/Android]
**Masalah:** Script Ruby melempar `No such file or directory @ rb_file_s_size - Data/PaletteChanger/race/moot/eyes_moot.act`.
**Penyebab:** File di disk bernama `Data/PaletteChanger/race/Moot/eyes_moot.act` (huruf 'M' kapital), sedangkan referensi di file JSON menggunakan huruf kecil `moot`. Filesystem Windows mengabaikan perbedaan huruf besar/kecil, tetapi Android Linux peka terhadap huruf besar/kecil (*case-sensitive*).
**Solusi/workaround:** Tambahkan modul `RuntimePackCasePath` di bootstrap Ruby yang mencegat method `File.exist?`, `File.size`, `File.open`, dan menelusuri segmen path menggunakan `casecmp` untuk mencocokkan ejaan file fisik di disk.
**Tags:** #filesystem #case-sensitivity #android-linux

## [Crash dlopen Kernel32 pada Script CommandLineArgs]
**Masalah:** Dialog error `Failed loading Kernel32: dlopen failed: library "Kernel32" not found` muncul saat engine dimulai.
**Penyebab:** Skrip `0_CommandLineArgs.rb` memanggil `Win32API.new("Kernel32", ...)`. Karena Android bukan Windows, pemanggilan native dlopen gagal. Script `preloadScript` tidak efektif karena dieksekusi sebelum binding C++ mkxp-z mendefinisikan kelas `Win32API`.
**Solusi/workaround:** Pindahkan bootstrap script ke `"customScript": "runtimepack_bootstrap.rb"` di `mkxp.json` agar berjalan setelah binding native siap, lalu definisikan ulang kelas `Win32API` dengan fungsi stub safe return.
**Tags:** #win32api #mkxp-z #ruby #dlopen

## [AssetManager Buffer Limit pada File game.pack Terkompresi]
**Masalah:** Aplikasi gagal mengekstrak file `game.pack` sebesar 778 MB dari assets APK.
**Penyebab:** Gradle mengompresi file aset dengan DEFLATE secara default. Android `AssetManager.open()` memiliki batas buffer memori untuk dekompresi file berukuran besar.
**Solusi/workaround:** Konfigurasikan `androidResources.noCompress += listOf("pack", ...)` di `build.gradle.kts` agar file kontainer disimpan dalam mode `STORED` (uncompressed), sehingga dapat di-stream langsung tanpa batasan alokasi memori buffer.
**Tags:** #android #gradle #asset-manager #performance

## [SDLThread Tidak Berjalan Otomatis di Multi-Window MuMu Player]
**Masalah:** Layar tetap hitam dan log tidak menampilkan inisialisasi `SDL_main`, meskipun library native berhasil di-load.
**Penyebab:** Fork SDLActivity menonaktifkan pemanggilan `resumeNativeThread()` di `onStart()` bawaan dan menyerahkan kontrol ke subclass. Pada mode multi-window emulator, `onResume()` tidak otomatis memicu start thread bila state focus belum tercapai.
**Solusi/workaround:** Tambahkan override `onStart()` di `RgssActivity.kt` yang secara eksplisit memanggil `resumeNativeThread()`.
**Tags:** #sdl2 #android #lifecycle #multithreading

## [Corrupt JSON String pada Serialisasi Empty Hash di 101_jsonEnDecoder.rb]
**Masalah:** File `rp_cheat_state.json` terisi dengan format JSON tidak valid `{"gold":0,"variables":},"switches":}}` sehingga parser JSON di WebView overlay gagal membaca status game.
**Penyebab:** Script `101_jsonEnDecoder.rb` melakukan penggantian slice string terakhir `string[string.size - 1, 1] = "}"` pada hasil pembukaan `"{"`. Ketika hash kosong `{}` di-encode, loop `each_key` tidak berjalan sehingga `"{"` langsung diganti menjadi `"}"`. Akibatnya `JSON.encode({})` mengembalikan `"}"` alih-alih `"{}"`.
**Solusi/workaround:** Gunakan `serialize_fallback` khusus pada cheat runner bootstrap Ruby yang memetakan hash/array kosong secara eksplisit ke `"{}"` dan `"[]"`, serta pasang monkey-patch pada `JSON.encode_hash` dan `JSON.encode_array` di `patch_json_decoder`.
**Tags:** #ruby #json #serialization #edge-case

## [SyntaxError pada Script Console Lolos dari rescue => err]
**Masalah:** Jika user salah mengetik sintaks di Script Console (misal kurung tidak lengkap), muncul dialog error unhandled Ruby dan freeze game loop.
**Penyebab:** Di Ruby, `SyntaxError` diturunkan dari `ScriptError < Exception`, bukan `StandardError`. Pernyataan `rescue => err` hanya menangkap turunan `StandardError`.
**Solusi/workaround:** Tangkap error secara eksplisit menggunakan `rescue Exception => err` pada blok `eval` dan `process_cmd` agar seluruh error kompilasi dan runtime Ruby ditangkap lalu dituliskan ke file hasil `rp_cheat_eval_res.txt`.
**Tags:** #ruby #syntax-error #exception-handling #eval

## [Font setter strict bool dan glyph tombol arah pada mkxp-z Android]
**Masalah:** Popup NPC/SKIP crash dengan `Argument 0: Expected bool`, sementara tutorial menampilkan `ALT + □ □ □ □`.
**Penyebab:** Script Galv menetapkan `font.shadow = 20`, tetapi binding mkxp-z mewajibkan boolean murni. InputUtils juga memakai U+2B06/U+2B07/U+2B05/U+27A1 yang tidak dirender konsisten pada runtime Android.
**Solusi/workaround:** Bootstrap mengoersi setter boolean `Font` dan menormalisasi mapping `InputUtils.@keySYM_in_UI` menjadi label ASCII `Up`, `Down`, `Left`, `Right` setelah semua script game dimuat.
**Tags:** #rgss #mkxp-z #font #glyph #android

## [ALT dan multi-touch D-Pad tidak terkirim ke game RGSS]
**Masalah:** Kombinasi ALT + arah tidak mengubah arah kamera; sebagian tombol multi-touch melepas tombol lain lebih awal.
**Penyebab:** `vpad.js` mengirim Alt sebagai keycode JS 18, tetapi `RgssActivity.mapJsKey()` tidak memetakannya sehingga event dibuang sebelum SDL dan `rp_input_state.bin`. Handler tombol juga memanggil `btnOnUp` pada `touchend` mana pun, sehingga melepas satu jari ikut melepas tombol yang masih ditahan jari lain.
**Solusi/workaround:** Tambahkan mapping JS 18 ke `KEYCODE_ALT_LEFT` + VK `0x12` (VK_MENU) dan lengkapi F1–F12/Delete; lacak `buttonActiveTouchId` sehingga tombol hanya dilepas saat sentuhannya sendiri berakhir.
**Tags:** #rgss #mkxp-z #input #multitouch #android

## [Font bawaan game tidak ter-resolve oleh mkxp-z (kotak glyph)]
**Masalah:** Karakter fullwidth colon `：` (U+FF1A) di nama pembicara dan panah U+2B06/07/05/27A1 di tutorial tampil sebagai kotak.
**Penyebab:** mkxp-z me-resolve font via FreeType `TTF_FontFaceFamilyName` yang mengembalikan *typographic family* (name_id 16). Font `SYS-DEFAULT-NotoSansCJKtc-Black.otf` terdaftar sebagai `Noto Sans CJK TC`, sedangkan script game meminta *full name* `Noto Sans CJK TC Black` (name_id 4). Tidak cocok → mkxp-z jatuh ke font built-in Liberation Sans/Arimo yang tidak punya glyph CJK/fullwidth.
**Solusi/workaround:** Tambahkan `fontSub` di `mkxp.json` (dibuat `GpackExtractor.kt`) yang memetakan full name ke typographic family: `Noto Sans CJK TC Black>Noto Sans CJK TC`, `Noto Sans CJK TC Medium>Noto Sans CJK TC`, `Source Han Sans Bold>Source Han Sans`, `Noto Sans LRPG Black>Noto Sans LRPG`.
**Tags:** #rgss #mkxp-z #font #freetype #android

## [Normalisasi glyph gagal karena perbedaan encoding string Ruby]
**Masalah:** Normalisasi `InputUtils.@keySYM_in_UI` tidak berjalan meski sudah dipasang.
**Penyebab:** Script game di-`Zlib::Inflate` dan `eval` sebagai string ASCII-8BIT, sehingga nilai glyph adalah byte binary. `DIRECTION_LABELS` di bootstrap adalah UTF-8. `==`/`Hash#key?` lintas encoding dengan byte non-ASCII bernilai false.
**Solusi/workaround:** `value.dup.force_encoding(Encoding::UTF_8)` sebelum lookup; test memakai string binary agar tidak lolos palsu.
**Tags:** #ruby #encoding #rgss #android

## [Keyboard fisik tidak sampai ke SDL karena WebView overlay]
**Masalah:** Tombol keyboard fisik tidak terbaca game saat overlay virtual pad aktif.
**Penyebab:** `SDLActivity.dispatchKeyEvent` meneruskan tombol ke view yang fokus; WebView overlay dapat mengambil fokus sehingga tombol masuk ke WebView, bukan SDL.
**Solusi/workaround:** Override `dispatchKeyEvent` di `RgssActivity` untuk meneruskan tombol ke `SDLActivity.handleKeyEvent` selama input teks overlay tidak fokus; flag fokus di-set oleh bridge `setOverlayTextFocused` dari listener `focusin`/`focusout` overlay, dan di-reset di `onPause`/`onDestroy` serta saat window blur/hidden.
**Tags:** #sdl2 #android #keyboard #webview #input

## [MuMu Player Multi-Display & ADB Input Tap Routing]
**Masalah:** Perintah `adb shell input tap 35 235` tidak membuka menu cheat dan tampak tidak merespons layar game di MuMu Player.
**Penyebab:** (1) Toolbar button icon petir (Cheat) berada di koordinat fisik (x=40, y=170), sedangkan y=235 adalah tombol linktree. (2) MuMu Player mengelola multi-display SurfaceFlinger/WindowManager di mana display 0 adalah launcher desktop, dan aktivitas game berjalan pada display sekunder (`displayId=4`). Command `input tap` bawaan Android default ke display 0 jika opsi `-d` tidak diberikan.
**Solusi/workaround:** Kirim tap menggunakan flag display `-d 4` (atau broadcast multi-display) pada koordinat tombol yang tepat: `adb shell input -d 4 tap 35 170`. Tombol toggle header `[ Cheat Menu ]` berada di koordinat (818, 103).
**Tags:** #mumu #adb #input #multi-display #cheat-overlay
