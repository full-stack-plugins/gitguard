# Explicit local bare-reference CAS

This is a **partial task4.3 implementation for independent review**. It adds an explicitly invoked library writer for CreateBranch and fast-forward Merge in a protected local bare repository. Push and CreateWorktree remain Unsupported before any intent claim. The full four-action task remains unchecked. There is no CLI writer, remote transport, fetch, credential loader or production authorization provider. The historical generic `execution::request` still always denies; default builds and default configuration cannot dispatch this new writer either.

## Opt-in and trust boundary

`apply(&ApplyContext, &ProtectedBareTarget)` requires both Cargo feature `privileged-execution` and `ExecutionConfig { enabled: true }`. Configuration alone, ALLOW, a FlowGuard gate, `approved=true` or a deserialized grant does not authorize a write. The controller must protect its repository-ID-to-target mapping, candidate/expected policy, GrantAuthority and GrantClock implementations, grant reference and local filesystem permissions. Adapter IDs and repository labels are selectors, not identity proofs. No built-in production authority is selected. Tests supply explicitly fixture-only authority; the actual native Git write is real, but the fixture authority is not a deployment certification.

`ProtectedBareTarget::open` pins an owner-private directory descriptor and freezes its repository label, filesystem device/inode, supported config bytes and object format into a platform digest. Symlinks, multiply linked files, wrong owners, group/world writable entries, config includes/unknown settings, alternates, shallow/graft/replace and worktree metadata reject. Only canonical Git-init bare config is supported, with SHA1 or SHA256 format. There is no automatic normalization or config rewrite. Objects must already exist in the target; no implicit fetch or transfer occurs.

Linux ext-family/XFS/Btrfs/overlayfs/tmpfs form the local filesystem profile; unknown/network filesystems reject. Tmpfs targets are ephemeral. This is not a hostile-same-UID filesystem sandbox: the controller must protect the directory and ancestors against concurrent administrative replacement, object/config edits, snapshot rollback and copying. Concurrent supported ref updates are handled by Git CAS. Directory descriptor paths anchor the selected inode across ordinary path renames. There is no remote/server required-check protection claim; hooks are intentionally disabled for this dedicated controller-owned local target.

## Dispatch sequence and exact objects

1. Reject feature/config disabled, unsupported action and pre-dispatch cancellation. Bounded admission precedes revalidating the actual CandidateSnapshot against its Repository and matching every field through the original frozen binding digest and exact candidate OID. Advisory candidate facts do not independently authorize execution; the separate externally authenticated grant binds the full candidate context.
2. Recheck bounded target metadata/config, validate all target objects using actual `git fsck --full --strict --no-reflogs`, require candidate/old objects to be commits, inspect the exact literal target ref and reject symbolic refs. Source and target object formats/repository labels must agree.
3. For Merge, the **same old OID used by CAS** must be an ancestor of the exact candidate. No merge commit is synthesized. Same tree/different commit cannot replace the frozen candidate. Branch creation requires absence.
4. Perform fresh GrantSession validation. Prepare an intent whose request digest additionally binds the physical platform identity; the same operation cannot be prepared for another bare directory under the same label. Public intent `prepare` semantics remain unchanged. Claim the durable single-attempt reservation, then validate grant/clock/revocation again immediately before dispatch. A past GrantObservation is never accepted in place of fresh validation.
5. Execute fixed `git update-ref --no-deref <refs/heads/name> <exact-candidate> <exact-old-or-zero>`. Existing Git runner clears inherited environment, disables hooks/helpers/network and applies its Linux process/output/deadline limits. The old-value condition is enforced by Git's ref lock/CAS, not a read followed by an unconditional write. There is no force/reset/delete fallback. Direct local bare CAS is **not** ordinary remote push.

The target scan admits at most10000 entries, depth32,1MiB cumulative path bytes and64MiB stored file bytes, with a2-second cooperative scan deadline. Config is at most64KiB, candidate serialization at most1MiB. Individual native calls retain the existing10-second/8MiB process profile. No external error strings or command output are exposed by ApplyError. These are bounded local controller operations, not hard realtime scheduling or zero upstream allocation guarantees.

## Results, failure and future reconciliation

Success yields a private `LocalCasReceipt` with exact candidate, operation ID and platform-bound request digest. It means native Git acknowledged that historical atomic update, not that the ref remains unchanged afterward, that hardware persisted it, or that future execution is authorized. The durable journal retains AttemptRecorded; a later restart conservatively requires task4.5 reconciliation. No task4.5 platform-success record is fabricated here.

Any post-reservation authorization failure, cancellation, native rejection/timeout or delivery fault returns RecoveryRequired and tries to append the existing uncertainty transition. If that append fails, AttemptRecorded already prohibits another claim. No successful receipt is returned on uncertainty and no rollback occurs. Cancellation is checked before dispatch and after the bounded native call; it cannot retract a write that already occurred or promise immediate interruption. Expiry is validated by the protected controller clock at dispatch; atomic server-side time/authority enforcement is not part of this local profile.

The tests perform real temporary Git/bare mutations only. They cover exact branch/fast-forward success and source preservation; two independently reserved/prevalidated writers released at a barrier into the same Git CAS; same-tree/different-commit fully validated negative; unrelated old commit; default disablement and unsupported actions; revoked-after-claim authority; cancellation before claim and physical-target redirection; hostile hook/config/alternate/symlink/storage bounds; and an actual successful native CAS followed by a private test-only delivery failure, which leaves the new ref intact and records uncertainty. That final injection is not a remote-network failure claim.

## Run the isolated end-to-end example

From the GitGuard repository on the supported Linux filesystem:

```sh
cargo run --locked --offline --features privileged-execution --example local_bare_apply
```

The example accepts **no arguments or user repository path**. It creates a new private temporary child of the current directory, constructs deterministic fixture commits and local bare targets, and removes only those generated repositories on exit. It never calls push, fetch, merge, reset or force-update. Fixture setup preprovisions objects; the library then performs the authorized local CAS.

Stdout is structured JSON containing the actual frozen candidate, requests, fixture-only grants/time/identity, old/new OIDs, local results and persisted intent events for six scenarios: branch creation, fast-forward Merge, non-fast-forward rejection, fully valid same-tree/different-candidate rejection, authority rejection and two independently reserved writers contending on one CAS. The actual Git version and source before/after SHA256 are included. Successful results explicitly say `reconciled: false`; task4.5 remains absent.

Commit dates/content/identities are fixed, so candidate OIDs are reproducible. Physical-target request digests intentionally vary with the temporary directory inode, and either race participant may win. Those fields should not be compared as deterministic golden strings. Ordinary execution without the opt-in feature, or with any argument, exits4 before creating a fixture.
