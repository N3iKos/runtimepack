# Configuration and Schemas

## 1. Project configuration

`runtimepack.toml`:

```toml
schema = 1

[app]
name = "Example Game"
package = "com.example.game"
version_name = "1.0.0"
version_code = 1

[engine]
mode = "auto"

[android]
abis = ["arm64-v8a"]
orientation = "auto"
format = "apk"

[content]
mode = "packed"
compression = "auto"

[compatibility]
policy = "strict"

[signing]
mode = "debug"
```

## 2. Engine override

Allowed only when user intentionally resolves detection:

```toml
[engine]
mode = "forced"
id = "rpgmaker-vxace"
```

Build report must record that detection was overridden.

## 3. Content modes

```text
plain
packed
encrypted-streaming
```

## 4. Compatibility policies

### strict

UNKNOWN or unsupported dependency fails build.

### warn

UNKNOWN may build after explicit CLI opt-in. Hard unsupported native dependency still fails unless separately overridden by a development-only flag.

## 5. Build manifest

See `schemas/build-manifest.schema.json`.

## 6. Runtime manifest

See `schemas/runtime-manifest.schema.json`.

## 7. Schema evolution

Rules:

- required semantic changes increment schema integer;
- parsers reject future unknown major schema versions;
- new optional fields may be added without breaking old readers.
