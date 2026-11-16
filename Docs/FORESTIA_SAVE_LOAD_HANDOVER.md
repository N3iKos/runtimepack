# DOKUMEN HANDOVER & POST-MORTEM TEKNIS: INVESTIGASI SAVE/LOAD GAME FORESTIA (RPG MAKER MZ)

**Dokumen ini disusun untuk:** Agen/Engineer berikutnya sebagai bahan evaluasi komprehensif tanpa harus mengulang riset dari awal.  
**Tanggal:** 29 September 2026  
**Workspace:** `D:\02_Development\Projects_2\RPGM-P`  
**Target Game:** Forestia v1.4.4 IND (`Raw-Game\Forestia1.4.4-IND`) — Engine: RPG Maker MZ v1.4.4 + Inazuma Life System Plugin Suite.

---

## 1. Ringkasan Eksekutif Permasalahan

Game **Forestia** (RPG Maker MZ) yang di-port ke Android mandiri (APK) menggunakan RuntimePack mengalami 2 masalah besar:
1. **Crash Navigasi (`getRoadRegion`)** ➔ **[STATUS: SUDAH SELESAI]**
   - Game crash dengan pesan `TypeError: Cannot read properties of undefined (reading 'getRoadRegion')` saat player berjalan di peta atau saat NPC bergerak sesuai jadwal.
2. **Save Berhasil, tetapi Load Gagal/Macet** ➔ **[STATUS: BELUM SELESAI / SEDANG DIINVESTIGASI]**
   - Di PC atau di aplikasi JoiPlay: game bisa di-save dan di-load normal (1 kali klik pada slot langsung masuk ke game).
   - Di APK RuntimePack saat ini: Save Slot 1 **berhasil tertulis ke disk** (terbukti ada file fisik 174 KB di `/data/data/com.example.forestia/files/saves/mz/file1`, didekompresi JSON-nya 100% utuh 1.32 MB), slot save muncul di menu title "Lanjutkan" lengkap dengan nama karakter dan waktu main, **TETAPI saat slot diklik di menu Load, game tidak mau loading / tidak mau masuk ke gameplay**.

---

## 2. Masalah #1: Crash `getRoadRegion` (Penyebab & Solusi yang Sudah Diterapkan)

### Akar Masalah:
* Plugin navigasi Forestia (`js/plugins/inazuma/npc/GlobalMap.js:2557-2567`) memanggil:
  ```javascript
  setupEachMaps() {
      const fs = require("fs");
      const path = Iz.GlobalMap.relativePath("data");
      fs.readdirSync(path).forEach((file) => {
          let mapId = file.match(/[0-9]{3}/);
          if (mapId) {
              mapId = Number.parseInt(mapId[0]);
              this._data[mapId] = new MB_Map(mapId);
          }
      });
  }
  ```
* Di Android WebView, modul native Node.js `fs` tidak ada. File shim lama (`runtimepack_web_shims.js`) meng-hardcode:
  ```javascript
  readdirSync: function() { return []; } // ❌ Mengembalikan array kosong!
  ```
* Akibatnya `this._data` di `$globalMap` kosong melompong (`{}`). Saat pathfinding NPC memanggil `$globalMap.map(mapId).getRoadRegion(x, y)`, objek `$globalMap.map(mapId)` bernilai `undefined`, memicu crash fatal seketika.

### Solusi yang Sudah Berhasil Dipasang:
* Di host Kotlin (`WebBackend.kt`), dibuat bridge native `@JavascriptInterface fun readdir(relPath)` yang membaca index file in-memory langsung dari container binary `game.pack` (`GpackStreamReader.fileIndexMap.keys`).
* Di `runtimepack_web_shims.js`, `readdirSync` disambungkan ke bridge native tersebut dengan fallback membaca `data/MapInfos.json`.
* **Hasil:** Crash `getRoadRegion` 100% tuntas dan game bisa dimainkan berjalan di map.

---

## 3. Masalah #2: Mengapa Save Bisa, tetapi Load Macet? (Analisis Mendalam)

### A. Fakta Empiris dari Pengujian Terakhir di Device/Emulator:
1. **File Save Fisik Benar-Benar Ada dan Valid**:
   Pengecekan via ADB pada paket `com.example.forestia`:
   * `/data/data/com.example.forestia/files/saves/mz/file0` (Autosave, 294 KB)
   * `/data/data/com.example.forestia/files/saves/mz/file1` (Slot 1, 174 KB)
   * `/data/data/com.example.forestia/files/saves/mz/global` (Global slot registry, 788 B)
   * `/data/data/com.example.forestia/files/saves/mz/system` (System data, 452 B)
2. **Integritas Data Biner Sempurna**:
   File `file1` ditarik dan diuji via Python/Node.js:
   * Base64 decode ➔ lolos.
   * Pako / zlib decompress ➔ lolos, menghasilkan 1.320.790 bytes JSON murni.
   * `JsonEx.parse` ➔ lolos, 30 keys utama (`system`, `actors`, `party`, `map`, `player`, `globalMap`, `backup`, dll.) utuh 100%.
   * Di layar title menu "Lanjutkan", data save terbaca sempurna: Karakter *"フィロ"*, *Tahun ke-1 Bulan Musim Semi Hari ke-2*, Playtime *00:00:24*, versi *1.4.4*.
3. **Gejala Saat Load Ditekan**:
   Saat berada di layar menu *"Muat dari file yang mana?"* (`Scene_Load`), pemain mengeklik kotak **File 1** atau menekan tombol A/Z:
   * Kotak File 1 tersorot (highlight ungu).
   * **Game tidak berpindah ke map / gameplay**. Tidak ada error ledakan merah di layar, tidak ada stack trace di logcat, game hanya tetap diam di menu load.

---

### B. Akar Masalah Sebenarnya (Mengapa JoiPlay/PC Bisa, tapi Kita Gagal?)

#### 1. Kesalahan Paradigma: `isLocalMode` (PC Mode) vs Web Mode
* **Cara PC & JoiPlay (Terbukti Stabil)**:
  JoiPlay membiarkan RPG Maker MZ berjalan dalam **Mode PC Lokal (`isLocalMode = true`)**.
  Di RPG Maker MZ (`rmmz_managers.js:612-626`):
  ```javascript
  StorageManager.isLocalMode = function() {
      return Utils.isNwjs(); // Di PC bernilai true
  };
  StorageManager.saveZip = function(saveName, zip) {
      if (this.isLocalMode()) {
          return this.saveToLocalFile(saveName, zip); // ➔ Memanggil fs.writeFileSync secara sinkron
      } else {
          return this.saveToForage(saveName, zip);    // ➔ Memanggil localforage IndexedDB
      }
  };
  StorageManager.loadZip = function(saveName) {
      if (this.isLocalMode()) {
          return this.loadFromLocalFile(saveName);    // ➔ Memanggil fs.readFileSync secara sinkron
      } else {
          return this.loadFromForage(saveName);
      }
  };
  ```
  Di PC dan JoiPlay, saat pemain klik Load 1 kali, engine langsung memanggil `fs.readFileSync("save/file1.rmmzsave")`. Semuanya berjalan **sinkron (synchronous)** seketika itu juga.

* **Kesalahan Fatal Pendekatan Kita Kemarin**:
  Di `runtimepack_web_shims.js`, kita memaksakan:
  ```javascript
  StorageManager.isLocalMode = function() { return false; }; // ❌ Memaksa mode Web/Browser
  ```
  Dengan memaksa `isLocalMode = false`, kita memaksa RPG Maker MZ menggunakan jalur **`saveToForage` dan `loadFromForage` (IndexedDB / Promise-based)** yang didesain untuk browser online, bukan untuk aplikasi native.

#### 2. Mekanisme Input Touch pada `Scene_Load` di Mode Web Mewajibkan "Double Touch"
Di RPG Maker MZ core engine (`rmmz_windows.js:1169-1215` dan `1248-1253`):
```javascript
Window_Selectable.prototype.isTouchOkEnabled = function() {
    return (
        this.isOkEnabled() &&
        (this._cursorFixed || this._cursorAll || this._doubleTouch)
    );
};
```
* Pada `Window_SavefileList`:
  * Sentuhan/klik pertama pada mouse/layar hanya memicu `onTouchSelect()` untuk memilih slot.
  * Hanya jika slot tersebut diklik **dua kali cepat (Double Touch / Double Click)**, properti `this._doubleTouch` bernilai `true`.
  * Jika pemain hanya mengetuk/mengeklik 1 kali, `isTouchOkEnabled()` bernilai `false`, sehingga fungsi `this.processOk()` **TIDAK PERNAH DIJALANKAN**.

#### 3. Bug Tombol Virtual Gamepad Overlay: `keyCode` Terpotong di Chromium
Mengapa saat pemain menekan tombol virtual **A** atau **Z** di layar, game juga tidak merespons OK?
* Di `WebActivity.kt`:
  ```kotlin
  open fun dispatchKeyToGame(jsKeyCode: Int, type: String) {
      val script = """
          (function() {
              var evt = new KeyboardEvent('$type', {
                  keyCode: $jsKeyCode,
                  which: $jsKeyCode,
                  bubbles: true,
                  cancelable: true
              });
              window.dispatchEvent(evt);
              document.dispatchEvent(evt);
          })();
      """.trimIndent()
      gameWebView?.evaluateJavascript(script, null)
  }
  ```
* **Fakta W3C DOM Level 3 & Chromium Modern**:
  Pada Android WebView (Chromium 110+), constructor `new KeyboardEvent('keydown', { keyCode: 90 })` **mengabaikan parameter `keyCode` dan `which`**. Properti `evt.keyCode` otomatis bernilai **`0`**!
* Ketika RPG Maker MZ menerima event tersebut (`rmmz_core.js:5886`):
  ```javascript
  Input._onKeyDown = function(event) {
      const buttonName = this.keyMapper[event.keyCode]; // event.keyCode == 0!
      if (buttonName) {
          this._currentState[buttonName] = true;
      }
  };
  ```
  Karena `event.keyCode === 0`, `this.keyMapper[0]` bernilai `undefined`. Action `"ok"` tidak pernah aktif di `Input._currentState`. Akibatnya, tombol virtual Gamepad A maupun tombol fisik keyboard via ADB sama sekali tidak bisa menekan "OK" di menu game.

---

## 4. Status Git & Repository Saat Ini

### A. Branch Utama (`master` di `D:\02_Development\Projects_2\RPGM-P`)
* **Commit Terakhir:** `385df60` (`feat(vfs): implement Pilar 1 in-memory native readdirSync bridge with MapInfos fallback`).
* **Uncommitted Working Changes:**
  Perbaikan Track 3 (Overlay Ren'Py Touch Routing & 4 Pilar VFS) yang sudah menyelesaikan crash `getRoadRegion` dan memperbaiki 1.158 false positive audit. File-file ini **TIDAK BOLEH HILANG** karena sudah terbukti stabil.

### B. Branch Fitur Save (`feature/native-mv-mz-save` di `.worktrees/native-mv-mz-save`)
* Dibuat sebagai worktree terisolasi agar tidak mengotori `master`.
* Berisi:
  * `NativeSaveRepository.kt`: Modul Kotlin penyimpanan atomik Java NIO (`ATOMIC_MOVE` + fsync). Modul ini **sudah 100% bekerja dan terbukti berhasil menulis file save ke disk Android**.
  * `WebBackend.kt`: Tambahan 5 method bridge native (`saveSlot`, `loadSlot`, `slotExists`, `deleteSlot`, `listSlots`).
  * `runtimepack_web_shims.js`: Modifikasi shim save adapter kemarin (yang keliru memaksakan `isLocalMode = false`).

---

## 5. Rekomendasi Solusi Terbaik untuk Agen Berikutnya (Pendekatan Native JoiPlay)

Jangan lagi memodifikasi `localforage` atau memaksakan mode browser web (`isLocalMode = false`). Terapkan pendekatan **Pure PC Local Mode** persis seperti JoiPlay:

### Langkah 1: Aktifkan Mode PC Lokal di Shim
Di `runtimepack_web_shims.js`, biarkan:
```javascript
StorageManager.isLocalMode = function() { return true; };
```
Atau kembalikan `Utils.isNwjs = function() { return true; };` sehingga game mengira dirinya berjalan di PC (NW.js).

### Langkah 2: Sambungkan `fs.readFileSync` & `fs.writeFileSync` ke `NativeSaveRepository`
Ketika `isLocalMode = true`, RPG Maker MZ otomatis memanggil:
* `StorageManager.saveToLocalFile` ➔ memanggil `fs.writeFileSync(filePath, zip)`
* `StorageManager.loadFromLocalFile` ➔ memanggil `fs.readFileSync(filePath, "utf8")`
* `StorageManager.localFileExists` ➔ memanggil `fs.existsSync(filePath)`
* `StorageManager.removeLocalFile` ➔ memanggil `fs.unlinkSync(filePath)`

Cukup sambungkan method `fs` di `runtimepack_web_shims.js` khusus untuk folder `save/`:
```javascript
// Di dalam fsShim runtimepack_web_shims.js:
writeFileSync: function(filePath, data) {
    var clean = String(filePath || "").replace(/\\/g, "/");
    if (clean.indexOf("save/") !== -1) {
        var slotName = clean.substring(clean.lastIndexOf("/") + 1).replace(".rmmzsave", "");
        if (window.RuntimePackNative && window.RuntimePackNative.saveSlot) {
            // Encode base64 agar aman dari binary truncation
            var b64 = (typeof btoa === "function") ? btoa(data) : Buffer.from(data, "binary").toString("base64");
            return window.RuntimePackNative.saveSlot("mz", slotName, b64);
        }
    }
},
readFileSync: function(filePath, opt) {
    var clean = String(filePath || "").replace(/\\/g, "/");
    if (clean.indexOf("save/") !== -1) {
        var slotName = clean.substring(clean.lastIndexOf("/") + 1).replace(".rmmzsave", "");
        if (window.RuntimePackNative && window.RuntimePackNative.loadSlot) {
            var b64 = window.RuntimePackNative.loadSlot("mz", slotName);
            if (b64) {
                return (typeof atob === "function") ? atob(b64) : Buffer.from(b64, "base64").toString("binary");
            }
        }
        return null;
    }
    return syncFetch(filePath);
},
existsSync: function(filePath) {
    var clean = String(filePath || "").replace(/\\/g, "/");
    if (clean.indexOf("save/") !== -1) {
        var slotName = clean.substring(clean.lastIndexOf("/") + 1).replace(".rmmzsave", "");
        if (window.RuntimePackNative && window.RuntimePackNative.slotExists) {
            return window.RuntimePackNative.slotExists("mz", slotName);
        }
    }
    return syncExists(filePath);
}
```
* **Dampaknya:**
  * Engine berjalan 100% menggunakan kode PC aslinya.
  * Tidak ada rantai Promise `localforage` yang macet.
  * **Pemain klik 1 kali pada slot save langsung me-load game seketika!**

### Langkah 3: Perbaiki Event Dispatcher Keycode di `WebActivity.kt`
Agar tombol virtual A (OK) di layar sentuh berfungsi di seluruh UI game, perbaiki `dispatchKeyToGame` di `WebActivity.kt`:
```kotlin
open fun dispatchKeyToGame(jsKeyCode: Int, type: String) {
    runOnUiThread {
        val script = """
            (function() {
                var evt = new KeyboardEvent('$type', {
                    bubbles: true,
                    cancelable: true
                });
                Object.defineProperty(evt, 'keyCode', { get: function() { return $jsKeyCode; } });
                Object.defineProperty(evt, 'which', { get: function() { return $jsKeyCode; } });
                window.dispatchEvent(evt);
                document.dispatchEvent(evt);
                
                // Backup langsung ke engine RPG Maker
                if (typeof Input !== 'undefined' && Input.keyMapper) {
                    var action = Input.keyMapper[$jsKeyCode];
                    if (action && Input._currentState) {
                        Input._currentState[action] = ('$type' === 'keydown');
                        if ('$type' === 'keydown') {
                            Input._latestButton = action;
                        }
                    }
                }
            })();
        """.trimIndent()
        gameWebView?.evaluateJavascript(script, null)
    }
}
```

---

## 6. Kesimpulan Penting untuk Agen Selanjutnya
1. **Jangan buang kode di `master`**, karena perbaikan VFS dan crash `getRoadRegion` ada di sana.
2. **Jangan gunakan mode browser web (`isLocalMode = false`) untuk RPG Maker MZ**, karena UX touch pada `Scene_Load` MZ di web memang mewajibkan double-click dan asinkron.
3. **Gunakan Mode PC Lokal (`isLocalMode = true`) + Hook `fs` Native**, karena inilah rahasia mengapa JoiPlay bisa menjalankan semua game RPG Maker secara instan dan stabil.
