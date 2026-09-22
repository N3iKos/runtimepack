# Multi-Engine Validation Matrix & 30-Minute Playtest Protocol

Dokumen ini adalah matriks verifikasi resmi untuk pengujian stabilitas lintas engine (Multi-Engine Validation) pada RuntimePack.

Setiap game dalam daftar ini wajib diuji bermain selama minimal **30 menit** di perangkat Android / emulator target untuk memastikan tidak ada *memory leak*, crash transisi map, desinkronisasi audio, kegagalan save/load, atau malfungsi kontrol.

---

## 1. Aturan & Protokol Pengujian (SOP)

1. **Durasi Pengujian:** Minimal **30 menit gameplay aktif** per judul game.
2. **Dokumentasi Hasil Port:**
   - Setiap judul game yang mulai diuji **WAJIB dibuatkan folder/dokumen catatan di dalam `Docs/test_ports/<engine_code>_<game_slug>/report.md`**.
   - Dokumen catatan tersebut berisi rincian:
     - **Status Akhir:** `BERHASIL` | `KENDALA (IN PROGRESS)` | `RUSAK / BLOCKED`
     - **Catatan Teknis:** Masalah shimming, audio, font/encoding, atau performa yang ditemui.
     - **Solusi / Patch:** Solusi yang diterapkan pada pipeline RuntimePack untuk game tersebut.
3. **Pemberian Ceklis `[x]`:**
   - Ceklis **HANYA** boleh dicentang (`[x]`) jika game telah dimainkan selama 30 menit tanpa crash, lolos seluruh kriteria evaluasi (Boot, Audio, Input, Save/Load, Transisi Map), dan dokumen laporannya telah terisi lengkap di `Docs/test_ports/`.

---

## 2. Checklist Kriteria Evaluasi 30 Menit

Setiap game harus lulus kriteria berikut sebelum ditandai `[x]`:
- [ ] **Booting & Title Screen:** Booting bersih tanpa freeze, crash native, atau loop render.
- [ ] **Audio Engine:** BGM, BGS, dan ME/SE berputar jernih tanpa distorsi, desync, atau crash saat minimize/resume.
- [ ] **Kontrol (Virtual Pad & Fisik):** Navigasi menu, pergerakan karakter, dan aksi hotkey berfungsi presisi.
- [ ] **Grafis & Font:** Teks terbaca jelas (bebas kotak/tofu), rendering tile/sprite rapi.
- [ ] **Siklus Save & Load:** Berhasil membuat file save, memuat ulang game, dan data tersimpan persisten.
- [ ] **Stabilitas 30 Menit:** Tidak mengalami kebocoran memori (OOM), lag bertahap, atau crash saat transisi map berulang.

---

## 3. Matriks Pengujian Game Lintas Engine

### RPG Maker XP (RGSS1)
*Folder Laporan: `Docs/test_ports/rpgxp_<game_slug>/report.md`*
- [x] **To the Moon** (`Docs/test_ports/rpgxp_to_the_moon/report.md`) — *Status: Lolos Booting, Title Screen, Audio, Virtual Pad, Multi-Language; Terverifikasi Playtest 30 Menit.*
- [ ] **OneShot** (`Docs/test_ports/rpgxp_oneshot/report.md`)
- [ ] **Aveyond: Rhen’s Quest** (`Docs/test_ports/rpgxp_aveyond_rhens_quest/report.md`)
- [ ] **Exit Fate** (`Docs/test_ports/rpgxp_exit_fate/report.md`)
- [ ] **Pokémon Essentials** (`Docs/test_ports/rpgxp_pokemon_essentials/report.md`)

---

### RPG Maker VX (RGSS2)
*Folder Laporan: `Docs/test_ports/rpgvx_<game_slug>/report.md`*
- [ ] **The Witch’s House** (`Docs/test_ports/rpgvx_the_witchs_house/report.md`)
- [ ] **Ao Oni** (`Docs/test_ports/rpgvx_ao_oni/report.md`)
- [ ] **Mermaid Swamp** (`Docs/test_ports/rpgvx_mermaid_swamp/report.md`)
- [ ] **Ib** (`Docs/test_ports/rpgvx_ib/report.md`)
- [ ] **Mad Father** (`Docs/test_ports/rpgvx_mad_father/report.md`)

---

### RPG Maker VX Ace (RGSS3)
*Folder Laporan: `Docs/test_ports/rpgvxa_<game_slug>/report.md`*
- [ ] **LonaRPG** (`Docs/test_ports/rpgvxa_lonarpg/report.md`) — *Status: Lolos Booting, Shimming Win32 & Cheat Menu; Menunggu Playtest Lengkap 30 Menit.*
- [ ] **LISA: The Painful** (`Docs/test_ports/rpgvxa_lisa_the_painful/report.md`)
- [ ] **LISA: The Joyful** (`Docs/test_ports/rpgvxa_lisa_the_joyful/report.md`)
- [ ] **Always Sometimes Monsters** (`Docs/test_ports/rpgvxa_always_sometimes_monsters/report.md`)
- [ ] **Angels of Death** (`Docs/test_ports/rpgvxa_angels_of_death/report.md`)
- [ ] **Pocket Mirror** (`Docs/test_ports/rpgvxa_pocket_mirror/report.md`)
- [ ] **The Last Sovereign** (`Docs/test_ports/rpgvxa_the_last_sovereign/report.md`)
- [ ] **The Amber Throne** (`Docs/test_ports/rpgvxa_the_amber_throne/report.md`)
- [ ] **Skyborn** (`Docs/test_ports/rpgvxa_skyborn/report.md`)
- [ ] **Deadly Sin 2** (`Docs/test_ports/rpgvxa_deadly_sin_2/report.md`)
- [ ] **Forever Home** (`Docs/test_ports/rpgvxa_forever_home/report.md`)
- [ ] **Ara Fell** (`Docs/test_ports/rpgvxa_ara_fell/report.md`)
- [ ] **Black Souls** (`Docs/test_ports/rpgvxa_black_souls/report.md`)
- [ ] **Black Souls II** (`Docs/test_ports/rpgvxa_black_souls_2/report.md`)

---

### RPG Maker MV (Web / Chromium)
*Folder Laporan: `Docs/test_ports/rpgmv_<game_slug>/report.md`*
- [ ] **Fear & Hunger 2: Termina** (`Docs/test_ports/rpgmv_fear_and_hunger_2_termina/report.md`) — *Status: Lolos Booting, Title Screen, Audio, Virtual Pad; Menunggu Playtest Lengkap 30 Menit.*
- [ ] **Fear & Hunger** (`Docs/test_ports/rpgmv_fear_and_hunger/report.md`)
- [ ] **OMORI** (`Docs/test_ports/rpgmv_omori/report.md`)
- [ ] **Your Turn to Die** (`Docs/test_ports/rpgmv_your_turn_to_die/report.md`)
- [ ] **The Coffin of Andy and Leyley** (`Docs/test_ports/rpgmv_coffin_andy_leyley/report.md`)
- [ ] **In Stars and Time** (`Docs/test_ports/rpgmv_in_stars_and_time/report.md`)
- [ ] **Pocket Mirror: Goldener Traum** (`Docs/test_ports/rpgmv_pocket_mirror_goldener_traum/report.md`)
- [x] **The Witch’s House MV** (`Docs/test_ports/rpgmv_the_witchs_house_mv/report.md`) — *Status: Lolos Booting, Title Screen, Audio, Virtual Pad, Map Opening; Terverifikasi Playtest 30 Menit.*
- [ ] **Grimm’s Hollow** (`Docs/test_ports/rpgmv_grimms_hollow/report.md`)
- [ ] **Hylics 2** (`Docs/test_ports/rpgmv_hylics_2/report.md`)
- [ ] **Ann** (`Docs/test_ports/rpgmv_ann/report.md`)

---

### RPG Maker MZ (Web / Chromium)
*Folder Laporan: `Docs/test_ports/rpgmz_<game_slug>/report.md`*
- [ ] **The Aquarium Does Not Dance** (`Docs/test_ports/rpgmz_the_aquarium_does_not_dance/report.md`)
- [ ] **Aria’s Story** (`Docs/test_ports/rpgmz_arias_story/report.md`)
- [ ] **Project Kat** (`Docs/test_ports/rpgmz_project_kat/report.md`)
- [ ] **Paper Lily** (`Docs/test_ports/rpgmz_paper_lily/report.md`)
- [ ] **Midnight Train** (`Docs/test_ports/rpgmz_midnight_train/report.md`)
- [ ] **It Moves** (`Docs/test_ports/rpgmz_it_moves/report.md`)
- [ ] **At Home Alone Final** (`Docs/test_ports/rpgmz_at_home_alone_final/report.md`)
- [ ] **The Case Book of Arne** (`Docs/test_ports/rpgmz_the_case_book_of_arne/report.md`)

---

### Ren’Py (Python / SDL2)
*Folder Laporan: `Docs/test_ports/renpy_<game_slug>/report.md`*
- [ ] **The Question** (`Docs/test_ports/renpy_the_question/report.md`)
- [ ] **Doki Doki Literature Club!** (`Docs/test_ports/renpy_ddlc/report.md`)
- [ ] **Katawa Shoujo** (`Docs/test_ports/renpy_katawa_shoujo/report.md`)
- [ ] **Butterfly Soup** (`Docs/test_ports/renpy_butterfly_soup/report.md`)
- [ ] **Long Live the Queen** (`Docs/test_ports/renpy_long_live_the_queen/report.md`)
- [ ] **Cinderella Phenomenon** (`Docs/test_ports/renpy_cinderella_phenomenon/report.md`)
- [ ] **Our Life: Beginnings & Always** (`Docs/test_ports/renpy_our_life/report.md`)
- [ ] **The Letter** (`Docs/test_ports/renpy_the_letter/report.md`)
- [ ] **Milk inside a bag of milk inside a bag of milk** (`Docs/test_ports/renpy_milk_inside_a_bag/report.md`)
- [ ] **Highway Blossoms** (`Docs/test_ports/renpy_highway_blossoms/report.md`)

---

### TyranoBuilder / TyranoScript (HTML5 / Web)
*Folder Laporan: `Docs/test_ports/tyrano_<game_slug>/report.md`*
*(Catatan: Pekerjaan teknis diizinkan, distribusi publik tunduk pada gerbang `TYRANO-LEGAL`)*
- [ ] **Official TyranoScript Technical Samples** (`Docs/test_ports/tyrano_samples/report.md`)
- [ ] **Coffin of Ashes** (`Docs/test_ports/tyrano_coffin_of_ashes/report.md`)
- [ ] **Red Haze** (`Docs/test_ports/tyrano_red_haze/report.md`)
- [ ] **The Shadows That Run Alongside Our Car** (`Docs/test_ports/tyrano_shadows_alongside_car/report.md`)
- [ ] **Autumn’s Journey** (`Docs/test_ports/tyrano_autumns_journey/report.md`)
- [ ] **Heart Fragment** (`Docs/test_ports/tyrano_heart_fragment/report.md`)
- [ ] **Image of Perfection** (`Docs/test_ports/tyrano_image_of_perfection/report.md`)
- [ ] **Paths Taken** (`Docs/test_ports/tyrano_paths_taken/report.md`)
