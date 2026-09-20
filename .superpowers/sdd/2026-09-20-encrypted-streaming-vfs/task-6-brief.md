# Task 6: Full Multi-Engine Build Verification & Test Corpus Check

## Requirements
1. Execute `runtimepack build` on Lona RPG in `encrypted-streaming` mode:
   `cargo run --bin runtimepack -- build "Raw-Game\LONA-RPG-B.0.10.8.05" -o dist/lona-encrypted --package com.example.lonarpg --name "Lona RPG" --content-mode encrypted-streaming`
2. Verify:
   - Output APK `dist/lona-encrypted/LonaRPG.apk` exists.
   - `dist/lona-encrypted/game.pack` is encrypted with ChaCha20-Poly1305.
   - Calculate and record SHA-256 hash and size.
3. Install and verify on MuMu Player emulator (`127.0.0.1:16384`):
   - `adb -s 127.0.0.1:16384 install -r dist/lona-encrypted/LonaRPG.apk`
   - `adb -s 127.0.0.1:16384 shell pm clear com.example.lonarpg`
   - `adb -s 127.0.0.1:16384 shell am start -n com.example.lonarpg/io.runtimepack.host.RgssActivity`
   - Verify that loose plaintext files are NOT extracted to disk, and the startup logs indicate streaming mode.
4. Document the findings in `Docs/test_ports/rpgvxa_lonarpg/report.md`.
5. Write execution report to `.superpowers/sdd/2026-09-20-encrypted-streaming-vfs/task-6-report.md`.
