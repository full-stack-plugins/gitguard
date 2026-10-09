# Retained CodeGraph JavaScript export profile

`analyze` consumes frozen export bytes produced by actual `@colbymchenry/codegraph` 1.6.0 source `1072f82ce24db3d133258d30165cef6b74d108b2`. The tested runtime is the JavaScript grammar, Node 24.19.0, TypeScript 5.9.3 ES2022/CommonJS transpilation, schema rows 1/11, extraction version 27. It is not a full upstream build/typecheck or other-language qualification.

The controller supplies source revision, exact export SHA256, candidate and file-to-task mapping through `Profile`. These are frozen transport/provenance expectations, **not authenticated producer identity**. Do not derive the expected digest from untrusted input at consumption time. The adapter requires caller-created candidate snapshots, validates their bindings, matches the indexed candidate, rejects dirty/error/unresolved state, verifies committed source blob SHA256/size and explicit task paths, then resolves actual call edge endpoints to cross-task risks. Unsupported tables/version/language or incomplete mapped coverage fail closed. It does not translate the old `gitguard.graph-fixture/v1alpha1` covered labels.

Successful static observation always returns `unknown=true`, `complete=false` and missing `semantic-completeness`. Empty edges, no unresolved references and the producer's `index_state=complete` cannot establish semantic coverage or satisfy a required complete graph. No producer authentication, hosted admission, merge authority or write capability is added.

`tests/fixtures/codegraph-1072/query.json` is byte-for-byte retained actual output, SHA256 `6ff2cc68626c9a9a63ac7f40b0d17e0c133e78d91c08fee1085c15d45f84cb77`. `source.bundle` preserves its actual Git candidate `ebadafa7d34a3b8974d6df1beb271deded7c731a`. The test reconstructs these objects and consumes the original export without rewriting it. Negative cases explicitly re-freeze changed transport bytes to exercise schema/source checks separately from digest mismatch.

Full original command streams, source/runtime hashes, tool provenance, database and capture manifest are in ledger `codegraph-1072-capture/`; manifest SHA256 `1b20ea5ee66b66c316e469e72afe08858052fdc575e026a06516e50349752d1d`. Actual read-only queries preserved source and main DB bytes but created SQLite WAL/SHM sidecars. This importer reads retained bytes and Git objects only; it never opens the live CodeGraph database.

Run `cargo test --locked --offline --test extension_capabilities`. This is a partial task 5.2 local interoperability slice, not independent complete semantics or production deployment.
