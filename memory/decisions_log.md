# Decisions Log

## [2026-09-24] Centralized Panel Registry, Ghost Click Suppression, dan Matte Theme Stability
**Keputusan:**
1. Menyediakan registry modal terpusat `window.RuntimePackModalRegistry` (berbasis `Set`) dengan helper universal `setModalState(id, isOpen)` dan `registerModal(id, isOpen)` di `vpad.js`.
2. Menghubungkan seluruh siklus hidup panel overlay (`settings`, `cheat`, `diag`, `custom_control`, `inspector`) ke registry: bridge native `setOverlayModalOpen(true)` dipertahankan selama `size > 0` dan hanya dinonaktifkan (`setOverlayModalOpen(false)`) saat seluruh modal ditutup (`size === 0`).
3. Mengubah trigger `bindTap` pada `vpad.js` agar memicu aksi pada `pointerup` (setelah verifikasi pergerakan jari <= 10px) dan menekan synthetic click selama 500ms, bukan pada `pointerdown`.
4. Menerapkan interaction guard saat `#vpad-bar-collapse` ditoggle dengan menonaktifkan sementara `pointer-events: none` selama 150ms pada tombol yang baru terungkap dan menolak tap selama transisi layout.
5. Menegakkan `pointer-events: auto !important; touch-action: none !important;` serta `e.stopPropagation()` pada seluruh container dan header panel overlay (`#rp-diag-modal`, `#fnh-cheat-overlay`, `#vpad-editor-bar`, `#vpad-prop-panel`, `#vpad-key-modal`).
6. Memperbaiki aturan CSS tema matte pada `.vpad-top-btn:active` dan `.vpad-btn-pressed` agar mempertahankan warna tema aktif `#1e3a8a` / `#3b82f6` dan hanya melakukan tactile scale kompresi (`transform: scale(0.88)`), menghilangkan flicker ke abu-abu `#27272a`.
**Alasan:** Menutup satu modal (misalnya Diagnostic atau Cheat) sebelumnya langsung memicu `setOverlayModalOpen(false)` dan mematikan touch routing panel editor yang masih terbuka; mutasi DOM pada `pointerdown` collapse button memicu ghost click ke tombol Done di bawah jari; dan warna aktif tema matte tertimpa oleh rule `:active` abu-abu.
**Dampak:** Transisi antar panel mulus tanpa konflik bridge, tidak ada kebocoran klik/tap ke game di latar belakang, dan tombol aktif bebas flicker. Terverifikasi 100% pada 11 JS regression tests dan 27 integration tests Rust.

## [2026-09-24] WebActivity Modal Touch Routing & Gesture Ownership
**Keputusan:**
1. Saat `isOverlayModalOpen == true`, `shouldForwardTouchToGame(...)` langsung mengembalikan `false` dan `isTouchOnActiveControls(...)` mengembalikan `true` untuk semua koordinat layar tanpa terkecuali, sepenuhnya mem-bypass bounding box modal statis (`isTouchInsideModal`).
2. Menjaga kepemilikan gesture dari DOWN hingga UP/CANCEL via helper murni `resolveGestureCapturedByGame`: jika gesture diawali saat modal aktif, event DOWN, MOVE, dan UP/CANCEL tetap 100% berada pada `overlayWebView`.
3. Memastikan pembersihan flag penangkapan (`isTouchCapturedByGame = false`) saat modal dibuka (`setOverlayModalOpen(true)`).
4. Tidak mengubah perilaku `RgssActivity.kt` yang sudah benar menahan seluruh input di overlay saat modal aktif.
**Alasan:** Sebelumnya, sentuhan di luar bounding box modal terpusat (seperti saat editor custom control aktif layar penuh atau saat modal digeser) bocor ke `gameWebView` di bawahnya.
**Dampak:** Seluruh kontrol, panel editor, dan modal overlay di WebActivity terbebas dari kebocoran tap ke canvas game. Unit test Kotlin, JS regression tests, dan `cargo test` lulus 100%.

## [2026-09-24] Redesign UI Virtual Pad Modern: Arsitektur 3 File, iOS Spring Physics, 3 Tema Konsisten, & Diagnostic Suite untuk Beta/Release
**Keputusan:**
1. Memecah arsitektur overlay menjadi 3 file terpisah (`index.html`, `vpad.css`, `vpad.js`), memisahkan deklarasi CSS dan struktur DOM dari file logika kontrol.
2. Menerapkan kurva animasi fisika pegas dua arah khas iOS (`cubic-bezier(0.34, 1.56, 0.64, 1)` untuk mekar/muncul dan `cubic-bezier(0.32, 0.72, 0, 1)` untuk menyusut) dengan efek origin-based dari titik pusat tombol pemicu.
3. Menstandarkan 3 tema modular yang konsisten lintas komponen (VPad, Cheat Engine, Diagnostic Suite):
   - `Minimalist Glass`: Translucent frosted glass, aksen Electric Cyan (`#38bdf8`), subtle glow.
   - `Minimalist Matte`: Solid charcoal dark (`#141416`), aksen Deep Blue (`#1e3a8a` / `#2563eb`), zero glare.
   - `Retro OSD`: Amber CRT terminal (`#ff7700`), background hitam `#0a0a0c`, tombol lingkaran, sudut panel 2px.
4. Menghadirkan Floating Joystick dengan elastisitas semi-floating (mengikuti tarikan jempol di tepi luar secara lentur) beserta opsi toggle `Semi-Floating` ⇄ `Fixed` di Control Inspector.
5. Menghadirkan animasi tactile press empuk pada D-Pad seirama tombol A, B, X, Y (kompresi ~8% dengan semburan cahaya aksen aktif).
6. Mengimplementasikan Universal Bring-To-Front: Mengklik panel mana pun (Settings Bar, Inspector, Cheat Engine, Diagnostic Suite) mengangkat panel tersebut ke urutan layer teratas, sementara tombol gameplay (A/B/X/Y, D-Pad) terkunci di layer bawah sehingga tidak pernah menonjol/menutupi modal.
7. Membuka modal `+ Add Button` / `Change Key` (`#vpad-key-modal`) langsung dengan z-index absolut tertinggi di atas seluruh panel lainnya.
8. Menjadikan Diagnostic Suite Beta (`diagnostic_suite.js`) bagian standar dari build pipeline baik flavor `--flavor beta` maupun `--flavor release`, dengan penambahan toggle `Show FPS: ON/OFF` melayang di layar secara real-time, dan penyederhanaan teks Share Bug Report bebas AI slop.
9. Menegakkan Permanent Beta Watermark di pojok kanan atas bertuliskan `Beta Test\nVeldra Community` dengan style persisten `display: block !important; opacity: 0.8 !important; pointer-events: none;` khusus pada `--flavor beta`.
**Alasan:** Layout lama monolitik di dalam `vpad.js`, animasinya kaku, tombol saling menindih z-index, dan styling tema tidak harmonis lintas komponen.
**Dampak:** UI terasa mulus setara konsol handheld modern, arsitektur modular 3 file mudah di-maintain, dan seluruh 27 tes integrasi Rust serta seluruh test suite JS bridge lulus 100%.

## [2026-09-23] Pipeline Build Flavor (--flavor beta vs --flavor release) & Permanent Beta Watermark
**Keputusan:**
1. Menambahkan CLI argument `--flavor` pada `runtimepack build` (default `"beta"`), dengan validasi strict hanya menerima `"beta"` atau `"release"` (case-insensitive) dan me-return diagnostic `BLD-101` jika invalid.
2. Mencatat build flavor di `BuildManifest` (baik pada `manifest.flavor` maupun `manifest.provenance.flavor`).
3. Mengonfigurasi layer overlay HTML/JS/CSS secara dinamis saat project emission:
   - Jika `flavor == "beta"`: menginjeksi watermark permanen `#fnh-watermark` (`Beta Test / Veldra Community`) dengan style `display: block !important; opacity: 0.8 !important; pointer-events: none;`.
   - Jika `flavor == "release"`: menonaktifkan watermark secara tuntas (`display: none !important;`) dan mengosongkan teks watermark.
4. Menegakkan kebijakan wajib di `AGENTS.md` (Section 23): Seluruh autonomous subagent wajib menjalankan build dengan `--flavor beta` secara default.
**Alasan:** Memastikan build testing/evaluasi selalu memuat identitas komunitas dan watermark visual transparan yang persisten di semua scene, sekaligus menyediakan jalur bersih untuk release resmi.
**Dampak:** CLI dan Build Planner mendukung build flavor modular, teruji penuh pada test suite integration (`test_build_flavor_beta_injects_watermark`, `test_build_flavor_release_omits_watermark`).

## [2026-09-23] Dukungan Porting & Shimming TyranoScript Electron & NTRdemic
**Keputusan:**
1. Mengimplementasikan parser & extractor murni Rust `AsarArchive` (`engines/tyrano/src/asar.rs`) untuk membaca header JSON Electron ASAR dan mengekstrak aset web ke build staging directory sebelum dibundel ke `game.pack`.
2. Menambahkan aturan deteksi statis paket Electron `resources/app.asar` pada `TyranoDetector` (`engines/tyrano/src/lib.rs`) dengan bobot 120 (Confidence: EXACT).
3. Mengimplementasikan `TyranoAnalyzer` (`engines/tyrano/src/scanner.rs`) untuk memfilter desktop wrapper DLL (`ffmpeg.dll`, `libEGL.dll`, `libGLESv2.dll`, `vulkan-1.dll`, `d3dcompiler_47.dll`, `vk_swiftshader.dll`) sebagai `CMP-DESKTOP-EXCLUDED` / `CMP-REDIST-EXCLUDED` dan memeriksa ada tidaknya addon biner `.node`.
4. Menyediakan polyfill `process.execPath: ""` dan `process.env.HOME: ""` di `runtimepack_web_shims.js` untuk mengatasi `TypeError: Cannot read properties of undefined (reading 'indexOf')` saat `tyrano/libs.js` mengevaluasi `$.localFilePath()`.
5. Mem-patch hook `$.isNeedClickAudio` agar mengembalikan `false` dan menyetel `ready_audio = true`, mencegah blocking eksekusi audio tag browser policy pada WebView Android dengan `mediaPlaybackRequiresUserGesture = false`.
6. Memperbaiki pemberian charset di `VfsResourceInterceptor.kt`: hanya menyuplai encoding `"UTF-8"` untuk resource teks (`text/*`, `application/javascript`, `application/json`), dan menyuplai `null` untuk seluruh binary asset (`image/*`, `audio/*`, `font/*`).
**Alasan:** Game TyranoScript desktop yang dikemas dengan Electron membungkus seluruh konten HTML5 di dalam `resources/app.asar` dan mengasumsikan keberadaan API desktop Node.js `process`.
**Dampak:** Game *[Kimochi] NTRdemic* sukses dibangun menjadi APK mandiri (`NTRdemic.apk`, 158.63 MB) dan booting mulus ke Title Screen interaktif dengan rendering grafis dan audio playback sempurna di MuMu Player.
**Keputusan:**
1. Menambahkan aturan deteksi arsip terenkripsi `.rgssad`, `.rgss2a`, `.rgss3a` pada detektor RGSS (`engines/rgss/src/lib.rs`) dengan bobot 120 (Confidence: EXACT).
2. Menambahkan filter desktop runner DLL (`steam_api`, `fluidsynth`, `utils`) pada `RgssAnalyzer` untuk proyek desktop package (ditandai adanya `mkxp.conf`/`mkxp-console.exe`), mengganti fatal `CMP-102` menjadi issue informatif `CMP-DESKTOP-EXCLUDED`.
3. Memperluas `GpackExtractor.kt` untuk membaca konfigurasi `mkxp.conf` (memetakan `execName`, `RTP` zip/folder seperti `lang.dat`, dan `fontSub`), serta membuat symlink otomatis `To the Moon.rgssad -> Game.rgssad`.
4. Menambahkan polyfill Ruby 1.8 (`NilClass#id`, `NilClass#type`, `Hash#index`, `Array#to_s`), stub modul `MKXP` (`data_directory` -> `"./."`, `raw_key_states`), dan inisialisasi `.lang` default ke bahasa Inggris di `runtimepack_bootstrap.rb`.
5. Mengubah runner skrip bootstrap agar memuat arsip skrip secara berurutan (`Scripts.rxdata`, `Scripts.rvdata`, `Scripts.rvdata2`) dan menginstal hook input/cheat sebelum evaluasi skrip `Main`.
**Alasan:** Game RPG Maker XP seperti *To the Moon* menggunakan arsip terenkripsi `.rgssad`, dialek Ruby 1.8, dan modul C++ mkxp asli yang berbeda dari RGSS3/VX Ace.
**Dampak:** Game *To the Moon* sukses dibangun menjadi APK mandiri (`ToTheMoon.apk`, 154 MB) dan booting mulus ke Title Screen interaktif di MuMu Player.

## [2026-09-22] Integrasi Penuh RPG Maker MV Web Runtime & Fear & Hunger 2: Termina
**Keputusan:** 
1. Mengaktifkan `WebActivity.kt` di Android shell sebagai runner berbasis hardware-accelerated Chromium WebView untuk engine family `RPG Maker Web` (MV dan MZ).
2. Menormalisasi deployed NW.js packages secara otomatis: jika game memiliki `package.json` mengarah ke `www/index.html`, maka direktori `www/` dijadikan content root, dan seluruh file wrapper Windows (`Game.exe`, `nw.dll`, `node.dll`, dll.) diabaikan dari compatibility checks dan GPACK bundle.
3. Menginjeksi `runtimepack_web_shims.js` untuk menyediakan polyfill Node `path`, NW.js `Window`/`App.argv`, Greenworks Steam stub, penanganan audio extension fallback (`.ogg` vs `.m4a`), dan pengalihan storage ke IndexedDB.
4. Mendekripsi aset terenkripsi bawaan RPG Maker MV (`.rpgmvp`, `.rpgmvo`) secara on-the-fly di `VfsResourceInterceptor.kt` menggunakan encryptionKey dari `data/System.json`.
**Alasan:** RPG Maker MV/MZ adalah game berbasis WebGL/HTML5 yang membutuhkan Chromium WebView modern dan stubs desktop API agar dapat berjalan langsung di Android tanpa modifikasi manual.
**Dampak:** Game *Fear & Hunger 2: Termina* sukses di-build langsung menjadi APK standalone (`Termina.apk`, 1.52 GB) dan terverifikasi 100% playable di emulator MuMu Player.

## [2026-09-20] Dynamic Map_Hud Hooking & Sanitasi Awal Sprite Luka (Balok Hitam Pojok Kanan)
**Keputusan:** 
1. Menambahkan method hook `install_map_hud_hooks` yang dijalankan setiap frame pada `RuntimePackCheatRunner.update` sampai `Map_Hud` selesai didefinisikan oleh game script.
2. Mem-patch constructor `Map_Hud#initialize` dan method `refresh_wound_layout` agar saat instansiasi awal (ketika karakter belum memiliki luka/`total_wounds == 0`), seluruh 11 sprite luka tubuh (`@wound_layout`, `@wound_head`, `@wound_chest`, dll.) langsung disetel `opacity = 0`, `visible = false`, dan koordinat `src_rect.y` dinolkan (mencegah kalkulasi minus `26*(0-1) = -26` yang dijepit ke baris 0 hitam oleh OpenGL ES).
**Alasan:** Skrip game asli `102_01_map_hud.rb` membuat sprite luka dengan opacity 255 dan koordinat UV minus yang memicu perentangan piksel hitam di mkxp-z. Selain itu, early return `return if @total_wounds == total_wounds` di line 304 mem-bypass proses refresh pada inisialisasi awal.
**Dampak:** Balok hitam di pojok kanan atas jendela permainan saat gameplay hilang total, layar bersih 100%.

## [2026-09-20] Penonaktifan Native Scrollbar WebView Overlay
**Keputusan:** Mematikan scrollbar native pada Android WebView overlay (`wv.isVerticalScrollBarEnabled = false`, `wv.isHorizontalScrollBarEnabled = false`, `wv.overScrollMode = View.OVER_SCROLL_NEVER`) di `RgssActivity.kt` dan `WebBackend.kt`, serta menambahkan aturan CSS `scrollbar-width: none` dan `::-webkit-scrollbar { display: none !important; }` di `index.html` dan `vpad.js`.
**Alasan:** Android WebView secara bawaan merender indikator scrollbar berupa kapsul hitam/abu-abu di pojok kanan atas saat view dimuat atau disentuh, yang mengganggu estetika layar permainan.
**Dampak:** Pojok kanan atas layar bersih total tanpa artefak kapsul scrollbar hitam.

## [2026-09-20] Fix Crash Menu Items (undefined method `key_item?' for nil:NilClass) & Pemisahan SPACE
**Keputusan:**
1. Memisahkan `KeyEvent.KEYCODE_SPACE` (`0x20`) dari tombol virtual **A** di `RgssActivity.mapJsKey(90)`. Tombol A kini murni mengirimkan konfirmasi `Z` (`0x5A`) dan `ENTER` (`0x0D`).
2. Menambahkan failsafe monkey-patch pada `Menu_Items` di `runtimepack_bootstrap.rb`:
   - Mengubah handling `@phase == 4` menjadi eksklusif (`if ... elsif`), mencegah tombol USE (`:C`) dan tombol DROP (`:UI_DeleteItem`) dieksekusi bersamaan dalam satu frame.
   - Menambahkan null-safety check pada `process_item_drop` dan `item_use_confirm_scr_dorp` sehingga jika item habis atau nil, menu kembali ke `@phase = 3/2` tanpa memanggil `item.key_item?` yang memicu dialog modal freeze.
**Alasan:** Tester mengalami freeze saat makan karena tombol virtual A memancarkan Z dan SPACE sekaligus, yang mengeksekusi konsumsi makanan dan drop item dalam 1 frame, memicu exception `key_item? for nil:NilClass`.
**Dampak:** Karakter dapat mengonsumsi makanan/item dengan lancar tanpa freeze atau dialog error.

## [2026-09-20] Perbaikan Popup Error Save/Load Lona RPG (Scene Transition & Screenshot Missing)
**Keputusan:**
1. Mengubah mekanisme scene transition cheat Save/Load dari `SceneManager.call` menjadi `SceneManager.goto(Scene_Save)` / `SceneManager.goto(Scene_Load_OnGameMenu)`, serta mem-monkey patch `SceneManager.call`/`return` agar membungkam `msgbox` debug bawaan developer game (`"fucking return #<Scene_Map:...>"`).
2. Mematikan `"pathCache": false` di `mkxp.json` agar mkxp-z dapat mendeteksi file screenshot baru yang dibuat saat game berjalan (`UserData/Save01.png`), serta membungkus `Cache.savefile_bitmap` dan `Bitmap.new` dengan fallback bersih ke `SaveNotFound.png` jika file PNG save tidak ditemukan tanpa memicu exception dialog.
**Alasan:** Lona RPG menaruh `msgbox` kasar di dalam `SceneManager.call`/`return` dan crash saat file screenshot save belum terindeks atau tidak ditemukan oleh mkxp-z.
**Dampak:** Menu Save Anywhere dan Load Anywhere terbuka mulus, bersih, dan 100% stabil.

## [2026-09-20] Resolusi Force Close RGSS & Pembatasan Zero-Disk Streaming ke Engine Web
**Keputusan:** 
1. Mengembalikan mode build default RGSS (VX Ace) ke kontainer `packed` standar dengan ekstraksi ke internal storage privat (`files/game/`).
2. Mode streaming zero-disk in-memory VFS (`VfsResourceInterceptor`) difokuskan untuk engine Web (MV, MZ, Tyrano) karena browser WebView mendukung `shouldInterceptRequest`. Engine native C++ mkxp-z tetap membutuhkan file tree fisik di storage privat sampai native driver PhysFS C++ di-recompile.
**Alasan:** Uji coba mode `encrypted-streaming` pada mkxp-z menyebabkan engine C++ tidak dapat menemukan `Game.ini` di disk dan langsung memicu shutdown.
**Dampak:** Game Lona RPG kembali booting 100% stabil ke Title Screen tanpa crash.

## [2026-09-20] Watermark Permanen Early Access Beta Test (Veldra Works)
**Keputusan:** Memasang watermark layar permanen di pojok kanan atas bertuliskan `Beta Test Veldra Works` (menggunakan layer DOM CSS `#fnh-watermark` dengan `display: block !important`, `opacity: 0.8`, `pointer-events: none`, text-shadow kontras) yang aktif di seluruh scene (Title, gameplay, dan saat virtual gamepad di-hide).
**Alasan:** User meminta build khusus rilis Early Access / Beta Test yang memuat identitas komunitas Veldra Works secara permanen dan profesional.
**Dampak:** Tanda beta test tampil elegan di pojok kanan atas layar tanpa mengganggu input touch/mouse maupun visual game.

## [2026-09-20] Navigasi Kategori SVG, Toggle Switch Persistent, dan Full Native Keyboard Sync
**Keputusan:** 
1. Merombak Cheat Menu menjadi berbasis Category Tabs horizontal dengan icon SVG monokrom (Vitals, Medical, Combat, Economy) tanpa teks deskripsi panjang agar tampilan bersih dan profesional (non-AI slop).
2. Menghadirkan kontrol toggle switch (`[ ON ]` / `[ OFF ]`) untuk God Mode, 1-Hit Kill, dan Speed Hack.
3. Mengimplementasikan persistent engine hooks di `runtimepack_bootstrap.rb`:
   - `$runtimepack_god_mode`: mengunci Health/STA pemain ke 200/100, memblokir `take_skill_effect`, dan mengabaikan `determine_death` di setiap frame lintas perpindahan map.
   - `$runtimepack_one_hit_kill`: saat pemain menyerang musuh di `BattleSystem#take_damage`, HP/STA musuh langsung di-set ke 0 / -100 sehingga mengeksekusi runtutan `process_death`, loot item, dan kill count resmi.
4. Menghubungkan seluruh penekanan keyboard fisik (A–Z, 0–9, F1–F12, Space, Enter, Esc, Shift, Ctrl, Alt) langsung ke buffer Win32 `rp_input_state.bin` di `RgssActivity.dispatchKeyEvent`, sehingga skrip Hime AllKey di Lona RPG dapat membaca seluruh tombol keyboard fisik secara native.
**Alasan:** User membutuhkan navigasi yang ringkas per kategori, toggle status yang jelas, God Mode yang persisten, fitur 1-Hit Kill, dan kemampuan keyboard fisik penuh tanpa batas.
**Dampak:** Menu cheat tampil bersih dan responsif, fitur cheat combat bekerja akurat, dan kontrol keyboard fisik berfungsi 100% seperti di PC.

## [2026-09-20] Otomasi Ekstraksi Icon Game.exe & Integrasi Android Launcher Mipmap
**Keputusan:** Mengekstrak resource icon PE bawaan dari `Game.exe` (.rsrc RT_GROUP_ICON / RT_ICON) dan me-rescale menjadi resource launcher mipmap Android (`ic_launcher.png` dan `ic_launcher_round.png` untuk mdpi, hdpi, xhdpi, xxhdpi, xxxhdpi), serta mendaftarkan `android:icon="@mipmap/ic_launcher"` dan `android:roundIcon="@mipmap/ic_launcher_round"` pada generator manifest Android.
**Alasan:** Icon APK sebelumnya kosong/default Android polos, sehingga tampilan saat terpasang di perangkat/emulator tampak tidak profesional.
**Dampak:** Aplikasi hasil build memiliki icon asli game yang tajam dan rapi di semua launcher Android.

## [2026-09-20] Cheat Engine ⇄ Cheat Menu Toggle & Modular Template (cheat_profile.js)
**Keputusan:** Menyediakan tombol toggle di header modal cheat untuk berpindah antara "Cheat Engine" (browser dinamis variabel/switch/spawner/console) dan "Cheat Menu" (cheat siap pakai dari `cheat_profile.js` dengan format template JavaScript deklaratif untuk AI Agent / Programmer).
**Alasan:** User membutuhkan kemudahan instan untuk cheat umum tanpa harus mencari manual nomor ID switch/variabel, sementara fleksibilitas AI Agent dan developer tetap terjaga melalui konfigurasi file template JS.
**Dampak:** Memudahkan penambahan cheat spesifik game secara instan, modular, dan terstruktur.

## [2026-09-20] Mode Hide Gamepad & Hardware Keyboard/Mouse Full Pass-Through
**Keputusan:** (1) Memperluas pemetaan tombol virtual ke seluruh tombol keyboard PC (A-Z, 0-9, Numpad, F1-F12, Simbol, Navigasi). (2) Meneruskan hardware keyboard langsung ke SDL (`onNativeKeyDown`/`Up`) dan Win32 `rp_input_state.bin` (`virtualKeyBuffer`) di `RgssActivity.dispatchKeyEvent`. (3) Saat virtual controls di-hide, layar touch/mouse diteruskan langsung ke `mSurface` (`dispatchTouchEvent`, `dispatchGenericMotionEvent`), serta tombol mouse kiri/kanan dipetakan ke `VK_LBUTTON`/`VK_RBUTTON`.
**Alasan:** Emulsi SDL menolak key event dengan source 0/unknown dari emulator, dan Win32API game membaca tombol dari buffer file `rp_input_state.bin`. Selain itu WebView menelan seluruh MotionEvent touch/mouse.
**Dampak:** Keyboard dan mouse fisik PC/emulator langsung berfungsi responsif untuk mengontrol game, dan penyembunyian gamepad langsung membuka mode keyboard/mouse penuh.

## [2026-09-19] Resolusi font lewat `fontSub`, bukan penambalan glyph per karakter
**Keputusan:** Memperbaiki kotak glyph CJK/fullwidth dengan menambahkan `fontSub` di `mkxp.json` (full name → typographic family FreeType), bukan mengganti karakter di teks game.
**Alasan:** Akar masalahnya adalah kegagalan resolusi font di mkxp-z (`Noto Sans CJK TC Black` vs `Noto Sans CJK TC`), sehingga satu perbaikan menyelesaikan seluruh glyph CJK sekaligus; menambal per karakter bersifat whack-a-mole.
**Alternatif yang dipertimbangkan:** Normalisasi teks game (U+FF1A→`:`), atau mengganti `Font.default_name`; keduanya tidak menyelesaikan akar masalah dan lebih invasif.
**Dampak:** Glyph CJK/fullwidth dirender oleh font bawaan game yang benar.

## [2026-09-19] Routing keyboard fisik via `dispatchKeyEvent` + bridge fokus, WebView tetap focusable
**Keputusan:** Meneruskan tombol fisik ke SDL lewat override `dispatchKeyEvent` selama input teks overlay tidak fokus; flag fokus di-set bridge `setOverlayTextFocused` dan di-reset di `onPause`/`onDestroy`/blur/hidden.
**Alasan:** Membuat WebView non-fokus memang mengembalikan keyboard ke game, tetapi merusak pengetikan soft-keyboard di cheat console.
**Alternatif yang dipertimbangkan:** WebView non-focusable + `FOCUS_BLOCK_DESCENDANTS` (ditolak karena mematahkan input teks overlay).
**Dampak:** Keyboard fisik terbaca game; soft-keyboard overlay tetap berfungsi.

## [2026-09-19] Strategi pengujian stabilitas: fokus RGSS3, target awal 20 game
**Keputusan:** Untuk sementara fokus optimisasi RPG Maker VX Ace/RGSS3 dengan target 20 game uji (3 proyek minimal, 4 ringan, 5 banyak script, 3 Win32API/input kustom, 3 berat, 2 terjemahan/mod).
**Alasan:** Keragaman script dan failure mode lebih penting daripada jumlah; LonaRPG saja tidak mewakili ekosistem VX Ace.
**Alternatif yang dipertimbangkan:** Klaim kompatibilitas universal lebih awal (ditolak; harus engine/profile/capability-specific).
**Dampak:** Definisi stabilitas bertahap: 10=alpha, 20=beta, 30=kandidat stabil, 50+basis luas; tiap game wajib lolos boot→map→battle→save/load→lifecycle.

## [2026-09-19] Posisi open source: open-core
**Keputusan (arah, belum final):** Membuka core (CLI, detector, analyzer, schemas, VFS, Android shell, test framework) dengan lisensi permisif, dan memisahkan runtime/aset berlisensi terbatas, signing, serta profil game privat.
**Alasan:** Mendapat kontribusi shim/fixture/pengujian perangkat dan audit keamanan tanpa melanggar kewajiban lisensi runtime (mkxp-z GPL, RTP EULA, Ren'Py, Tyrano).
**Alternatif yang dipertimbangkan:** Tertutup penuh, atau membuka semuanya tanpa audit lisensi (ditolak).
**Dampak:** Perlu audit lisensi per dependency sebelum rilis; klaim dukungan harus spesifik.

## [2026-09-19] Tunda "Save Anywhere" default
**Keputusan:** Menunda fitur cheat default "Save Anywhere" atas permintaan user.
**Alasan:** Tidak dapat dijamin universal (bergantung pada `$game_system.save_disabled`/menu standar; game dengan menu/save kustom bisa mengabaikannya).
**Alternatif yang dipertimbangkan:** Implementasi sekarang dengan penandaan profile-dependent (ditunda, bukan ditolak).
**Dampak:** Fokus tetap pada stabilitas input dan font RGSS3.

## [2026-09-18] JoiPlay / M-Tool Style Dynamic Cheat UI di cheat_menu.js
**Keputusan:** Memperbarui `cheat_menu.js` menjadi dynamic engine-agnostic cheat UI dengan 4 tab inti (`#fnh-tab-variables`, `#fnh-tab-switches`, `#fnh-tab-spawner`, `#fnh-tab-console`), live search/filtering, inline variable/switch mutator, dan bookmark snippet script console berbasis `localStorage` (`vpad_user_cheat_snippets`).
**Alasan:** Pengguna membutuhkan kontrol in-game yang fleksibel untuk memodifikasi state game RPG Maker/RGSS dan Web tanpa hardcoding ID item/variabel spesifik ke game tertentu.
**Alternatif yang dipertimbangkan:** Menjaga cheat menu statis per-game (sulit di-maintain dan tidak scalable lintas ribuan game RPG Maker).
**Dampak:** Menu cheat berfungsi dinamis pada seluruh game RPG Maker (XP/VX/Ace/MV/MZ) dengan antarmuka bergaya JoiPlay & M-Tool, mendukung Clean Slate & Retro OSD theme, dan 100% lulus uji otomatis.

## [2026-09-18] Line-Based JSON Decoder Patching di GpackExtractor
**Keputusan:** Mengganti regex replace dengan pencarian batas method eksplisit (`next_token` hingga `parse`) saat mem-patch `101_jsonEnDecoder.rb` di storage internal Android.
**Alasan:** File asli game mengandung blok kode yang di-comment out dengan komentar `# def self.next_token`, sehingga pencarian berbasis regex regex multiline ceroboh memakan baris komentar dan menghasilkan `SyntaxError: unexpected end`.
**Alternatif yang dipertimbangkan:** Menulis ulang file utuh dari asset patch terpisah (kurang fleksibel bila versi game berbeda).
**Dampak:** Ekstraksi game Lona RPG berjalan bersih tanpa error sintaksis Ruby pada modul JSON.

## [2026-09-18] Penggunaan customScript Bootstrap untuk Mengatasi Override Native Win32API
**Keputusan:** Menggunakan `"customScript": "runtimepack_bootstrap.rb"` pada `mkxp.json` alih-alih `"preloadScript"`.
**Alasan:** Pada arsitektur `mkxp-z`, file-file di `preloadScript` dieksekusi SEBELUM `mriBindingInit()`. Akibatnya, kelas C++ `Win32API` bawaan mkxp-z didefinisikan belakangan dan menimpa shim Ruby yang sudah dibuat di preload.
**Alternatif yang dipertimbangkan:** Memodifikasi dan mengompilasi ulang source C++ mkxp-z (memerlukan setup NDK manual yang berat).
**Dampak:** `runtimepack_bootstrap.rb` dieksekusi setelah seluruh binding native siap, mengizinkan monkey-patching kelas `Win32API`, penanganan case-sensitivity `File.size`, dan `visible=` coercion secara mulus.

## [2026-09-18] Uncompressed (STORED) Asset Packaging untuk game.pack
**Keputusan:** Menambahkan `noCompress += listOf("pack", ...)` pada konfigurasi `build.gradle.kts` Android.
**Alasan:** Android `AssetManager.open()` memiliki batas buffer dekompresi in-memory untuk file terkompresi DEFLATE besar (>100MB), menyebabkan `IOException` saat membaca `game.pack` (778 MB).
**Alternatif yang dipertimbangkan:** Memecah `game.pack` menjadi ribuan file asset kecil (membuat build lambat dan mengurangi proteksi kontainer).
**Dampak:** File `game.pack` disimpan tanpa kompresi ZIP di dalam APK dan bisa di-stream langsung secara instan tanpa limit memori.

## [2026-09-18] Integrasi Native mkxp-z ARM64 dan SDLActivity Bridge
**Keputusan:** Mengadopsi library native C++ `mkxp-z` (`arm64-v8a`), Ruby VM, SDL2, OpenAL Soft, dan `SDLActivity` Java sebagai fondasi runtime RGSS.
**Alasan:** Terbukti stabil, mendukung performa OpenGL ES native tanpa emulasi CPU Windows, dan kompatibel dengan RPG Maker XP/VX/VX Ace.
**Alternatif yang dipertimbangkan:** Porting Wine/Box64 (overhead performa tinggi, butuh resource besar, dan bukan native game feel).
**Dampak:** APK berperilaku seperti game native Android mandiri dengan frame rate 60 FPS.

## [2026-09-18] Unified Web Overlay untuk Kontrol Virtual dan Cheat
**Keputusan:** Menggunakan overlay WebView transparan di atas SurfaceView native untuk menjalankan `vpad.js` dan `cheat_menu.js`.
**Alasan:** Menghadirkan in-game layout editor interaktif bergaya Winlator/JoiPlay, 5 preset bawaan, dual theme (Clean Slate & Retro OSD), serta integrasi cheat tanpa perlu menulis ulang UI sentuh di C++ untuk setiap engine yang berbeda.
**Alternatif yang dipertimbangkan:** Render kontrol sentuh manual lewat skrip Ruby RGSS atau rendering C++ ImGui.
**Dampak:** UI kontrol sentuh seragam dan identik di seluruh engine keluarga RuntimePack (RGSS, Web, Ren'Py, Tyrano).
