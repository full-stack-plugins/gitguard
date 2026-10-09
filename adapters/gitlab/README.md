# GitLab read-only response profile

`gitguard.gitlab-rest-v4-train/v1` is a bounded local adapter profile of the
[GitLab REST v4 merge trains response](https://docs.gitlab.com/api/merge_trains/),
checked 2026-10-09. It is not a GitLab server version qualification. The tests use
local schema fixtures and actual local Git commits, not hosted API captures.

`prepare` accepts one train-car response, not the list endpoint's array. The
adapter requires an opened MR, a fresh car and successful merge-request-event
pipeline. Null pipelines, unsupported statuses, absent required fields and
ambiguous duplicate JSON keys fail closed. Unknown metadata is ignored. This is
an intentionally narrow observation profile; pipeline success does not grant
merge admission. The ref is compared as an opaque exact controller expectation;
the documented example `refs/merge-requests/59/train` is not inferred from MR ID.

The embedding controller supplies `CurrentTrain` through an independently
configured `CurrentLookup`. It must authenticate the server and bind project,
MR, car, pipeline, target/ref, raw response digest, generation, local repository,
base and ordered members. `merge_train_pipeline` is a controller attestation of
pipeline mode, **not a REST field or authentication switch**. The controller
must distinguish ordinary MR fallback from a real merge-train pipeline; the
REST `source=merge_request_event` alone cannot establish this. No default lookup,
HTTP credentials, response-provided authentication, or network calls exist here.
An untrusted caller must not be allowed to configure this controller port.

The candidate SHA must be a locally available commit different from base and
member heads. Core `CandidateRequest` validation freezes its exact bytes, scope,
base and ordered members. Input is capped at 1 MiB, members at 128, SHA-1 only.
The port is checked before and after local preparation; `is_current` rechecks the
entire controller binding whenever the observation is consumed. Unavailable or
missing state cannot pass. The result is advisory and carries no grant,
AuthorizedOperation, execution permission, or platform eligibility. Consumers
must still perform their own current lookup at the actual decision boundary.

Remaining host work: select and qualify the real GitLab instance/version and
project permissions, implement an authenticated live lookup with authoritative
base/member/pipeline-mode information, and test real train rebuild/cancellation.
No platform write, MR admission, or production authentication is claimed.
