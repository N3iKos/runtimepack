# Progress

## Status saat ini (update setiap sesi)
- **Sedang kerjain apa:** Menyelesaikan integrasi Milestone 4 RGSS native slice, shimming kompatibilitas Lona RPG, dan setup file memori/dokumentasi proyek.
- **Next step:** Pengujian gameplay in-game Lona RPG (New Game, map transition, combat, save/load) dan optimasi runtime web backend untuk engine MV/MZ.
- **Blocker:** Tidak ada blocker aktif.

## Selesai
- [2026-09-18] Inisialisasi git repository dan konfigurasi `.gitignore`
- [2026-09-18] Setup toolchain Rust `stable-x86_64-pc-windows-gnullvm` dengan LLVM-MinGW UCRT
- [2026-09-18] Implementasi Milestone 0 (arsitektur workspace Cargo, domain types, error taxonomy, schemas, toolchain lock)
- [2026-09-18] Implementasi Milestone 1 (detektor statis XP, VX, VX Ace, MV, MZ, Ren'Py, Tyrano, serta perintah `inspect` dan `explain`)
- [2026-09-18] Implementasi Milestone 2 (kerangka Android host shell, `BootstrapActivity`, `RuntimeApplication`, `BuildManifestLoader`)
- [2026-09-18] Implementasi Milestone 3 (GPACK/1 container packer, reader, path traversal validation, dan build pipeline)
- [2026-09-18] Implementasi compatibility scanner RGSS dan RPG Maker Web serta database deklaratif `compatibility-db/`
- [2026-09-18] Implementasi Milestone 5 & 6 (Web runtime adapter, JS bridge, dan Ren'Py versioned scanner)
- [2026-09-18] Implementasi Milestone 7 (kontainer terenkripsi streaming AEAD ChaCha20-Poly1305)
- [2026-09-18] Implementasi Milestone 8 & 9 (Signing manager, third-party notices, release hardening, toolchain auto-driver Gradle)
- [2026-09-18] Setup toolchain Android lokal (Microsoft OpenJDK 17, Android SDK platform 34, build-tools 34.0.0, Gradle 8.7)
- [2026-09-18] Milestone 4 RGSS Native Player: integrasi binary `libmkxp-z.so`, `libruby.so`, SDL2, dan `RgssActivity`
- [2026-09-18] Integrasi unified virtual controller overlay `vpad.js` (5 preset, drag-and-drop editor, 4 ikon toolbar) dan `cheat_menu.js`
- [2026-09-18] Perbaikan kompresi `game.pack` (`noCompress`) dan relokasi index table ke posisi depan (linear streaming)
- [2026-09-18] Perbaikan startup SDLThread dari `onStart()` pada multi-window emulator
- [2026-09-18] Shimming Win32API (`GetCommandLine`, `GetKeyboardState`, `GetCursorPos`, `GetSystemMetrics`, `SystemParametersInfo` non-zero workarea)
- [2026-09-18] Shim modul `DL` (`DL::CPtr`, `DL.malloc`) untuk kompatibilitas script Hime AllKey di Ruby modern
- [2026-09-18] Nonaktifkan crash optimasi BitmapChanger DLL dengan fallback pure Ruby tanpa popup error
- [2026-09-18] Patching parser `101_jsonEnDecoder.rb` menjadi non-rekursif untuk mencegah `SystemStackError: stack level too deep`
- [2026-09-18] Penanganan case-insensitive filesystem resolver pada skrip Ruby (`File.size`, `File.open`, `File.exist?`)
- [2026-09-18] Penanganan error encoding ASCII-8BIT ke UTF-8 pada logging error TKG (`open(filename, "wb")`)
- [2026-09-18] Penanganan coercion boolean pada setter `Sprite#visible=` untuk mengatasi typo game `@sprite.visible = @sprite`
- [2026-09-18] Lona RPG terbukti sukses booting dan merender Title Screen interaktif penuh pada MuMu Player
