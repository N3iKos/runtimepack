# Technical Findings

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
