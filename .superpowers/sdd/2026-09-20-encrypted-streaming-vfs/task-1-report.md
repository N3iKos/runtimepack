# Task 1 Report: Package-Bound Key Derivation & CLI Packaging Pipeline (Rust)

## Summary
Successfully implemented package-bound key derivation using HKDF-SHA256 in `runtimepack_pack::ContentCrypto` and integrated it into the CLI `build` command for `content_mode == "encrypted-streaming"`.

## Changes Made
1. **`Cargo.toml` & `pack/Cargo.toml`**: Added `hkdf = "0.12"` to workspace dependencies and enabled it in `runtimepack-pack`.
2. **`pack/src/crypto.rs`**: Implemented `ContentCrypto::derive_package_key`:
   - `ikm`: `format!("{}::{}", package_name, app_name)`
   - `salt`: `b"runtimepack_salt_v1"`
   - `prk`: `hkdf::Hkdf::<Sha256>::new(Some(salt), ikm.as_bytes())`
   - `info`: `b"gpack-content-v1"`
   - Output: 32-byte array (`[u8; 32]`).
3. **`pack/src/lib.rs`**: Added unit test `test_derive_package_key_deterministic_and_unique` verifying determinism, uniqueness across package names and app names, and non-zero key bytes.
4. **`cli/src/main.rs`**: Updated `cmd_build` to derive the encryption key via `ContentCrypto::derive_package_key(&pkg_id, &app_name)` when `content_mode == "encrypted-streaming"`.

## Verification Results
- `cargo test -p runtimepack-pack`: 4 passed (100%), including `test_derive_package_key_deterministic_and_unique`.
- `cargo test`: 42 passed (100%) across all packages in workspace.
