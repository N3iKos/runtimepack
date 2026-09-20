# SDD ledger — plan: Docs/superpowers/plans/2026-09-20-encrypted-streaming-vfs.md

## Pre-flight Scan
| Task A | Task B | Shared Interface / File | Finding | Ruling |
|---|---|---|---|---|
| Task 1 | Task 2 | `ContentCrypto::derive_package_key` (Rust) vs `GpackCrypto.derivePackageKey` (Kotlin) | Must use identical salt `runtimepack_salt_v1`, info `gpack-content-v1`, and IKM `$pkg::$name` | Spec is binding: HKDF-SHA256 test will assert matching vectors |
| Task 2 | Task 3 | `GpackCrypto.decryptChunk` -> `GpackStreamReader` | Nonce formulation and 16-byte Poly1305 tag must align | Exact nonce formulation specified in Task 2 and consumed in Task 3 |
| Task 3 | Task 4 | `GpackStreamReader` -> `VfsResourceInterceptor` | `openStream` must return plaintext `InputStream` | Aligned on Kotlin `InputStream` |
| Task 3 | Task 5 | `GpackStreamReader` -> `GpackExtractor` | Encrypted mode must bypass file extraction | Handled in Task 5 via `isEncryptedStreamingMode` |

Task 1: complete (commits 4c9c790..d6f3558, review clean)
Task 2: complete (commits d6f3558..33bccd6, review clean)
Task 3: complete (commits 33bccd6..8954feb, review clean)
Task 4: complete (commits 8954feb..51db6ac, review clean)
Task 5: complete (commits 51db6ac..f197dac, review clean)
Task 6: complete (ready for review)
