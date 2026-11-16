# Android Runtime Host

## 1. Application model

Generated apps are dedicated games, not launchers.

```text
Launcher intent
 -> BootstrapActivity
 -> RuntimeApplication
 -> RuntimeHost
 -> first frame
```

## 2. Bootstrap responsibilities

`BootstrapActivity` MUST:

- load `build-manifest.json`;
- initialize logging;
- initialize content service;
- initialize save service;
- instantiate selected backend;
- attach rendering surface;
- forward lifecycle.

It MUST NOT:

- detect game engine;
- scan source files;
- choose between multiple runtime families;
- show a game library UI.

Those decisions happened at build time.

## 3. Storage

Use app-specific internal storage for:

```text
saves/
runtime-state/
migration/
logs/   # bounded
```

Android states these locations require no storage permission and are isolated from other apps [S15].

Large immutable game content lives in APK assets or a supported asset delivery mechanism.

## 4. Permissions

Default generated manifest:

```text
no storage permission
no all-files permission
no network permission unless required
```

Possible capability-driven permissions:

```text
INTERNET
VIBRATE
POST_NOTIFICATIONS   # only if a future game capability truly needs it
```

## 5. Lifecycle

Backend must handle:

```text
onCreate
onStart
onResume
onPause
onStop
onDestroy
```

Save flush should occur on pause/stop where engine semantics permit.

## 6. Rendering abstraction

```text
RuntimeSurface
├── NativeSurface
└── WebSurface
```

RGSS uses native surface/SDL-like rendering.

MV/MZ/Tyrano use WebView/Web runtime surface.

Ren'Py uses the rendering surface required by the selected upstream Android runtime.

## 7. Web local origin

Do not load the game via broad unrestricted filesystem URLs.

Use a stable local application origin backed by a request handler/content provider. The exact implementation may use AndroidX WebKit facilities or an equivalent controlled mechanism.

Goals:

- same-origin stability;
- VFS integration;
- encrypted content streaming;
- interceptable diagnostics;
- path traversal prevention.

## 8. Web security defaults

Unless a compatibility profile requires otherwise:

- disable arbitrary file access;
- disallow mixed-content relaxation;
- expose the minimum JS bridge;
- reject navigation outside permitted origins unless intentionally opened externally;
- restrict debug WebView features in release builds.

## 9. Save service

Logical interface:

```kotlin
interface SaveService {
    fun openRead(slot: String): InputStream?
    fun openWrite(slot: String): OutputStream
    fun exists(slot: String): Boolean
    fun list(): List<String>
    fun delete(slot: String)
}
```

Web engines may internally use web storage. The adapter must ensure per-game isolation and backup/migration behavior.

## 10. Input

Common event format:

```json
{
  "type": "key_down",
  "code": "ENTER",
  "timestamp_ns": 0
}
```

Sources:

- touch virtual buttons;
- physical keyboard;
- gamepad;
- pointer/touch.

JoiPlay publicly maintains a separate virtual gamepad component for plugins [S24], supporting the design choice that input should be a reusable subsystem rather than engine code.

## 11. Health markers

Debug/test builds expose structured milestones:

```text
HOST_CREATED
CONTENT_MOUNTED
RUNTIME_CREATED
GAME_STARTED
FIRST_FRAME
SAVE_READY
```

Device CI uses these instead of screenshot guessing.
