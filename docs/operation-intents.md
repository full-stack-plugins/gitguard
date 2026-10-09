# Durable operation intents

Implemented for independent task 4.4 review. This is an opt-in Linux local storage library, not a Git writer, grant issuer or platform receipt service. Existing execution requests still fail closed. The other privileged-execution tasks and production trust-provider selection remain separate.

`IntentStore::create/open` accepts an existing owner-private directory on the same filesystem profile as durable evidence history (Linux ext-family, XFS, Btrfs or overlayfs). Only this environment's overlayfs process crashes/restarts have been exercised; a filesystem type number does not prove hardware power-loss guarantees. Network/pseudo/unknown filesystems reject. The controller must protect the directory and its ancestors against deletion, rollback, copying and same-UID edits. All participating executors must use one store namespace. Copies, hostile filesystem replacement, malicious privileged actors and network-wide uniqueness are outside this local profile.

`prepare(&GrantExpectation)` uses the privately frozen expectation's operation ID and full digest. Its identity includes exact actor/action/repository/candidate/full candidate binding/target/expected target/controller policy. Same ID and digest returns existing historical state; a different digest rejects. Preparing desired work is not authorization. The schema filename `operation-receipt.schema.json` follows the roadmap, but its versioned payload is an **intent observation**, never a claim that a platform operation succeeded.

States and generations are closed:

| State | Generation | Meaning |
|---|---:|---|
| prepared | 0 | Durable desired request; no attempt reserved |
| attempt_recorded | 1 | One attempt reservation durably recorded; observers must treat its external outcome as unknown |
| recovery_required | 2 | The reservation holder explicitly reports uncertainty; no replay permitted |

`claim(&prepared_receipt)` is a state CAS. Only the first caller can receive the private, non-Clone/non-Deserialize `AttemptReservation`, and only after file and directory sync. A replay returns an error, never another reservation. The reservation is storage ownership, **not authority to mutate Git**; a future protected writer must revalidate current grant and evidence and execute an atomic platform precondition. `mark_uncertain` consumes a reservation and records the next state. A crash may leave `attempt_recorded` without that marker: this is already non-retryable, not a lease that expires. No API resumes a reservation, resets a state, purges uniqueness history, records a fabricated platform success, or performs compensation. Later task 4.5 must authenticate platform receipt/history before any recovery decision. No currently exposed state permits blind retry.

A stable nonblocking `flock` serializes read/modify/commit across participating processes. Every operation reopens the lock without symlink traversal and checks identity and inode against the opened store. The directory descriptor anchors all children. Files must be owner-private regular files with one link. Missing, malformed, unsupported or checksum-inconsistent snapshots never initialize implicitly. Event replay also validates every transition, preventing a rehashed invalid state machine. SHA256 detects corruption, not malicious controller authentication or rollback.

The append-only logical journal is stored by atomically replacing a complete strict snapshot. Temporary file contents are synced before rename and directory sync follows rename. A failure whose commit may have occurred returns `OutcomeUncertain`, no reservation; reopen and observe. The pre-rename process-crash test can leave an uncommitted staging file; it neither resets nor consumes an operation key. A failure after durable commit but before delivery sacrifices availability rather than issue another attempt. No exact-once external mutation claim is made.

Limits: 16 MiB snapshot, 4096 operation IDs, 12288 events, 128-byte IDs and fixed 71-byte digests. A typed event therefore remains below 4 KiB. Borrowed serialization is counted before snapshot/checksum allocation. Input file metadata is checked before bounded read and strict decoding. Capacity exhaustion fails before commit, while existing same-request observations remain readable. Static typed errors contain no caller/provider text. There is no retention deletion because forgetting an operation ID would undermine idempotency.

Tests use actual Git candidate binding without changing its files, private local stores, two independent processes, true process exits before/after atomic replacement, rehashed malformed history, foreign/stale receipts, lock contention/replacement, unsafe paths and quota rejection. Fixture trust labels do not select a production issuer. Schema validation describes data shape only.
