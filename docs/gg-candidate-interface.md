# GG-CANDIDATE local advisory interface

Status: implemented local library slice, pending independent review. This is not an authenticated admission service, engine envelope, CLI, or Git writer.

```rust,ignore
use gitguard::{Repository, subject::SubjectRequest, scope::TaskScope,
    candidate::CandidateRequest, preflight::preflight};
let repo = Repository::discover(root, "controller-local-label")?;
let scope = TaskScope::advisory("task", vec!["R1".into()],
    vec![b"src".to_vec()], &policy_sha256, None)?;
let request = CandidateRequest {
    worktree_id: "worktree-1".into(), base_oid,
    merge_group_id: Some("synthetic-group".into()), members: ordered_member_oids,
};
let observed = preflight(&repo, SubjectRequest::Commit(exact_candidate_oid), &scope, &request)?;
```

`Repository::discover` resolves an explicit working-tree root and common Git directory, copies its bounded object store to private metadata, checks object hashes using `git fsck`, and supports tested SHA-1/SHA-256 objects. It never derives authority from origin URLs. The repo ID is a local caller label; a real controller must authenticate registration separately. Only full lowercase immutable OIDs are accepted. Objects added to the source after discovery require a fresh Repository.

`FrozenSubject` distinguishes Commit, TreePreview, Index and Worktree. Only Commit can supply a candidate OID. Digests cover the whole tracked candidate tree, or the whole observed worktree/index for those distinct subjects, not an arbitrary selected-file digest. Worktree files include untracked files; ignored files are conservatively included. Source symlink target bytes are hashed without following the link. The source digest streams sorted byte paths, length prefixes, Git modes, and SHA-256 content digests. Index unmerged entries fail. A commit is clean only when the observed worktree and index both match. Cleanliness is observation-time evidence; revalidation is required after drift.

`TaskScope` freezes sorted unique nonempty requirements, byte path prefixes and a policy digest before analysis. Paths are case-sensitive bytes; JSON uses arrays of byte integers, preserving non-UTF-8 names. Absolute paths, empty/dot/dot-dot components, backslashes and `.git` components are unsupported. Empty allowed paths means no changes are permitted. Candidate policy files are never loaded; deleting them cannot remove frozen obligations. This slice supports only explicit `baseline_digest: null`. Non-null digest-only baselines are rejected because immutable baseline source/authority verification is not implemented. A required baseline therefore cannot be satisfied by this slice.

`scan_scope` uses raw NUL-delimited recursive diff, including both rename endpoints, deletions and modes. Symlinks are observed as link objects. Submodule pointers report missing content coverage. `PreflightResult.complete` means the local candidate is clean, observed changed paths satisfy the frozen local scope, and no submodule content is missing. It is not an ALLOW decision and says nothing about graph, specialist, authenticated policy or approval coverage.

`prepare_candidate` reobserves clean state and freezes candidate/base/group/member order, repo/task/worktree/requirements, source and policy digest, and the actual allowed path prefixes. Paths are sorted lexicographically by unsigned bytes and exact duplicates are removed before binding. There is no case folding, Unicode normalization, or redundant ancestor/descendant-prefix elimination. Thus reordering/repeating identical prefixes preserves identity; any different canonical prefix set changes it. Empty prefixes remain a deny-all scope. The opaque caller-supplied policy digest is bound separately and is never presumed to cover the supplied paths. Reordered group members and different commit parents change the binding even when trees match. `preview_merge` supports only Git's two-parent merge-tree behavior and creates a deterministic preview commit in private storage. It does not install the preview into the source repository. Conflicts/errors do not produce a successful snapshot. Queue event authentication belongs to a future external port; CandidateRequest is the already-resolved local input port.

`CandidateSnapshot` fields are private and serialize under `gitguard.candidate/v1alpha2`, separate from GuardEngine. Call `validate(&repo)` after deserialization: structural serde validation alone is insufficient. Validation checks actual commit/base/member objects, candidate digest, and current observed cleanliness. Getters expose repo_id, task_id, worktree_id, requirement_ids, allowed_paths, candidate_oid, base_oid, merge_group_id, members, source_snapshot_digest, policy_digest, baseline_digest, clean, advisory and binding_digest. `advisory` is always true. This validation does not authenticate the supplied task/policy/group/provider identity and cannot substitute for GE-TRUST.

`worktree::Registry` provides process-local lease registration with private task/requirement/repo/root identity and unique IDs. Cancellation and reuse invalidate previous handles; first completion wins. It never creates/deletes worktrees or refs. It is not a durable cross-process scheduler or append-only history store.

`conflicts::analyze_impacts` consumes explicitly labelled `gitguard.graph-fixture/v1alpha1` graphs with required/covered index names and provider/consumer API/schema/event edges. Shared symbols produce risk observations. Missing or unsupported graphs retain unknown; missing required coverage is incomplete. These fixture observations neither execute compilation nor claim a production graph provider.

Verification: `cargo test --test gg_candidate_acceptance` runs real temporary repositories without FlowGuard, GuardEngine services, credentials, or platform writes. Full test/RED evidence and pending acceptance are recorded in the implementation ledger. Source refs/index/worktree bytes are compared in repository, merge, and parallel fixtures.

Compatibility: v1alpha2 adds required `allowed_paths` to serialized snapshots and binding-digest input. v1alpha1 snapshots are unsupported and must be regenerated; all prior digest/cache bindings must be invalidated. Rust constructor APIs remain unchanged. Snapshot validation requires safe, strictly byte-sorted unique prefixes; it does not silently normalize deserialized input. This change adds local scope identity only, not authentication or production trust.
