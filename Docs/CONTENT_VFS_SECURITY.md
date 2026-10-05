# Content, VFS, and Asset Protection

## 1. Design goals

- one logical filesystem API across engines where practical;
- immutable base content;
- writable save layer;
- patch overlay;
- optional packing/compression/encryption;
- deterministic content hashes;
- no full plaintext extraction in protected mode.

## 2. Logical mounts

```text
game:/    immutable base game
patch:/   compatibility/user overlay
save:/    persistent writable data
cache:/   ephemeral runtime cache
```

Runtime adapters translate engine paths into these mounts.

## 3. Read resolution

```text
read logical game path
  -> patch override?
      yes -> patch content
      no  -> base content
```

A patch cannot silently delete a base file unless the overlay format explicitly declares a tombstone.

## 4. GPACK/1 format

Working name: `GPACK/1`.

### Header

```text
magic
format_version
flags
manifest_offset
manifest_length
index_offset
index_length
chunk_table_offset
chunk_count
```

### File index entry

```text
logical_path
size
content_hash
chunk_start
chunk_count
compression
encryption
```

### Chunk entry

```text
offset
stored_length
plain_length
nonce/id
authentication_tag
```

Binary layout is intentionally not finalized in this prose document. Implement it from a dedicated schema/format ADR before production.

## 5. Path rules

Canonical logical paths:

- UTF-8;
- `/` separator;
- no `..`;
- no absolute root;
- no NUL;
- preserved original spelling metadata when needed.

Lookup policy for RGSS case-insensitivity must be backend-controlled; do not globally lowercase all paths because web engines can be case-sensitive.

## 6. Content modes

### plain

Best for debugging.

### packed

Single indexed container, optional compression, no secrecy.

### encrypted-streaming

Per-chunk authenticated encryption.

Recommended primitive family:

```text
AEAD
```

Exact algorithm is chosen in a security ADR. Do not hand-roll cryptography.

## 7. Encryption key hierarchy

Static/offline mode:

```text
content key
  -> wrapped/obfuscated for application
```

This is deterrence only.

Device-bound mode, future:

```text
server/provisioned content key
  -> wrapped by Android Keystore key
```

Android Keystore supports non-exportable keys and may use secure hardware [S16].

## 8. Threat model

Protect against:

- casual APK unzip;
- trivial asset copying;
- accidental corruption;
- naive static resource browsing.

Not guaranteed against:

- rooted device;
- debugger/instrumentation;
- runtime memory extraction;
- patched native code;
- full control of process.

Documentation must call protection "hardening", not unbreakable DRM.

## 9. Integrity

All packed files/chunks must have integrity verification.

The build manifest records the final content container SHA-256.

## 10. Save content

Saves are never stored inside encrypted immutable `game.pack`.

Save encryption, if added, is a separate feature with separate migration semantics.

## 11. Web asset serving

A Web runtime request:

```text
https://game.local/img/a.png
 -> LocalOriginHandler
 -> VFS.open("img/a.png")
 -> decrypt/decompress as needed
 -> stream response
```

No full content unpack is required.

## 12. RGSS integration

Preferred end-state:

```text
RGSS runtime file abstraction
 -> RuntimePack VFS
```

If the selected upstream runtime cannot be cleanly redirected, an adaptation spike must determine the least invasive mount mechanism.

A temporary full extraction strategy is acceptable only in early prototypes and must be explicitly labeled non-production for encrypted mode.
