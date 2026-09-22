# Technical Findings

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
