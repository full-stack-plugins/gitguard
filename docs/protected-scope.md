# Controller-frozen local scope

Task 1.4 adds `ProtectedTaskScope::freeze(&Repository, &ProtectedScopeRequest)` and `preflight_protected`. This is a **local controller source-only profile**, not authenticated approval, a production provider, or an SG baseline consumer. The caller must supply its protected request independently of the candidate. Rust ownership cannot authenticate that caller. No CLI operation or existing wire contract changes.

The request pins repository label, task, sorted unique requirements and byte path prefixes, and a policy source `(full commit OID, byte path, expected raw SHA-256)`. An optional baseline pins another such tuple. Full OIDs only: symbolic refs, trees and blobs are rejected. Controllers resolving a ref must resolve it before this API; no later ref read occurs. `read_commit_files` verifies actual regular committed bytes in the repository's bounded isolated object store; this profile inherits its unsupported symlink/gitlink and whole-tree read restrictions. No mutable worktree policy is read. Repository binding includes canonical root/common directory, object format and local registered label; it does not authenticate origin or a hosting provider.

Policy bytes must be strict JSON:

```json
{"schema_version":"gitguard.scope-policy/v1alpha1","task_id":"task","requirement_ids":["R1"],"allowed_paths":[[97]]}
```

The policy must exactly match the controller's preselected task/requirements/paths. Unknown/duplicate fields, malformed JSON and Markdown `accepted` are rejected. Baseline bytes are opaque provenance only: even `accepted` inside them cannot approve anything. Both source references, actual blob OIDs/modes/digests, full scope and local repository association enter the versioned descriptor hash. Any baseline/policy revision or byte change requires a new scope digest, including identical bytes at a different commit.

`preflight_protected` revalidates immutable sources and repository association before using the frozen scope in existing real diff/candidate checks. Candidate deletion or weakening does not remove requirements; rename and missing coverage semantics remain those of the existing preflight. The produced v1alpha2 candidate remains `advisory=true`, `baseline_digest=null`; its `policy_digest` commits the entire protected descriptor. It must not be interpreted as an authenticated SG baseline digest. The old `TaskScope` schema still rejects non-null baseline, and old consumers are unchanged. Audit descriptor JSON is not deserializable into a protected value; neither copying that JSON nor calling the legacy API with its hash confers trust.

Admission precedes hashing, cloning and Git reads: 128-byte repo ID; 256-byte task/requirement ID; at most 256 requirements/paths; 4096-byte paths/root/common directory; 64-byte OID/digest; 64 KiB aggregate metadata. Policy and baseline bytes each have a 64 KiB cap checked after the existing bounded reader and before hash/parse/clone. Preflight bounds worktree/group IDs to 256 bytes, candidate/base/member OIDs to 64 bytes and members to 64 before old candidate preparation. These small finite counts bound the entire request; existing Git process/tree budgets remain active. Parsing is bounded by source bytes and serde recursion limits. This does not claim constant-time operation or hostile same-UID filesystem isolation.

`schemas/task-scope.schema.json` preserves its old root and adds `$defs/sourcePolicy` and `$defs/protectedDescriptor` for audit tooling. Semantic validation also enforces byte-path safety, sorted sets, exact expected digests and actual Git objects. There is deliberately no audit-descriptor restore/approval endpoint.
