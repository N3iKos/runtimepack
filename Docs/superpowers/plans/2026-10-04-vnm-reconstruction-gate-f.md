# VNM Reconstruction Gate F: Bounded Raw GPACK Streams

**Author:** Supervisor main agent.
**Goal:** Remove asset-size heap allocation from raw GPACK reads, preserving authenticated encrypted behavior and shared-runtime correctness.
**Spec:** reconstruction-roadmap Gate F; CDE reviewed against live files.

## Constraints

Only isolated runtimepack-vnm-reconstruction. No original/recovery/game writes, deletions/cleanup, Git, installs/downloads, devices/MuMu/ADB/UI or APK build. No code comments. Worker cannot write/revise plans. Retain pending RT-101 until final gate. No largeHeap, catch-OOM empty response, encryption format redesign or whole-game extraction.

## Files and Interface

Modify `android/shell/app/src/main/java/io/runtimepack/host/GpackStreamReader.kt` and its existing JVM test. Read full source/producer/crypto and all consumers before editing. Existing `openStream(path: String): InputStream?` signature unchanged. Add internal bounded slice stream relying on actual SeekableSource positional readFully interface. Promote a source test seam to internal if necessary rather than adding public factory solely for tests.

Raw compression=0/encryption=0 -> bounded stream. Encryption=1 -> existing whole-chunk authentication/decrypt then stream. Unsupported entry compression/encryption rejected before payload allocation. Validate entry offset/size/storedSize nonnegative, offset+storedSize Long overflow, raw size==storedSize. Preserve version/header checks actual documented format; no speculative unsupported flags.

## Task 1: RED regression through production API

Create virtual huge raw entry via indexed fixture/spy SeekableSource and real reader.openStream. Opening and reading a small buffer must not request full entry payload; old ByteArray path fails allocation or bounded spy expectation. Prefer safe allocation guard fixture over destabilizing test daemon. Record actual RED before patch.

Add fixture tests with exact expected bytes/SHA256:
- Irregular bulk reads across64KiB and adjacent entries; no overrun.
- read(byte[],off,len): invalid offsets/length including overflow; zero len at EOF=0; EOF=-1.
- read() unsigned byte values; no per-byte large allocations.
- available clampsInt, negative skip=0, skip pastEOF clamps.
- stream close independent of parent and other streams; reader close fails subsequent open/read safely.
- Two real concurrent source streams with irregular chunk sizes detect seek races.
- FileChannel AssetFD nonzero baseOffset preserves offsets.
- Truncated source EOFException; raw size mismatch/negative/overflow/unknown entry flags rejected.
- Encrypted valid roundtrip and tampered ciphertext authentication fail unchanged, key checked before buffered allocation.

## Task 2: Minimal bounded stream

Store cursor/remaining as Long, one-byte field reusable for read(), fill caller-provided byte buffer via source.readFully(entry.offset+cursor,b,off,min(len,remaining)). Stream construction must not allocate based on entry size. Serialized source RAF seek/read and channel positioning remain concurrency-safe. Verify underlying source baseOffset arithmetic. Synchronize individual cursor methods if shared-stream concurrent use supported; otherwise document standard InputStream thread contract and still guarantee independent streams.

close stream only marksclosed; parent close marksreaderclosed before closing source. No mark/reset support unless actual consumer requires; markSupported false/reset IOException is explicit. Any read afterclose fails consistent contract. Cipher path authenticates entire chunk before plaintext exposure; its memory limitation remains unknown device dependent, no invented50MB threshold.

## Task 3: Verify and review

Run offline Kotlin compile/JVM tests/lint and existing JS suites to detect VFS/shared bridge regressions. Run cargo check/clippy/workspace tests offline for combined reconstruction. Actual counts/raw logs recorded; historical counts not reused. Android lint failures disclosed exactly.

Report `.superpowers/sdd/2026-10-04-vnm-reconstruction/gate-f-report.md`, changed files and diff vs baseline. Code-reviewer checks source lifetime/baseOffset/concurrency/bounds/authentication and tests production code, not duplicate mock implementation. No runtime memory/playback claims. Supervisor authors final build gate only after review.
