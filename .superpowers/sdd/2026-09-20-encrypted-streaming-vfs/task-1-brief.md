# Task 1: Package-Bound Key Derivation & CLI Packaging Pipeline (Rust)

## Requirements
Implement package-bound key derivation in `runtimepack_pack::ContentCrypto`:
1. Add function `pub fn derive_package_key(package_name: &str, app_name: &str) -> [u8; 32]` to `pack/src/crypto.rs`.
   - IKM: `format!("{}::{}", package_name, app_name)`
   - Salt: `b"runtimepack_salt_v1"`
   - HKDF: `hkdf::Hkdf::<sha2::Sha256>::new(Some(salt), ikm.as_bytes())`
   - Info: `b"gpack-content-v1"`
   - Output: 32 bytes (`[u8; 32]`).
2. Add unit test `test_derive_package_key_deterministic_and_unique` in `pack/src/lib.rs`:
   - Assert key1 == key2 for same package and name.
   - Assert key1 != key3 for different package.
   - Assert key1 != key4 for different name.
   - Assert key1 != [0u8; 32].
3. Update `cli/src/main.rs`:
   - In `content_mode == "encrypted-streaming"`, invoke `ContentCrypto::derive_package_key(&package_name, &app_name)`.
4. Run tests:
   `cargo test -p runtimepack_pack` and `cargo test`.
5. Write execution report to `.superpowers/sdd/2026-09-20-encrypted-streaming-vfs/task-1-report.md`.
