# Local CLI / API / MCP read-only ports

The existing `gitguard check < check.json` remains unchanged. Two explicit local commands now reuse the same `BoundCheck` candidate, policy, binding, coverage, projection and GuardEngine verification path:

```
gitguard api < api-request.json
gitguard mcp-stdio
```

`api` accepts one JSON object, at most 1 MiB, with exactly `version`, `capability`, and optional `request`. Version is `gitguard.api/v1alpha1`. For capability `check`, `request` is the existing strict `gitguard.check/v1alpha1` CheckRequest described in [the CLI contract](git-engine-cli-contract.md). For `capabilities`, omit `request` (null is also accepted). The capability result explicitly advertises `write:false`, `credentials:"unavailable"`, `authorization:"not-evaluated"`, and `profile:"local-advisory"`. Unknown versions, capabilities, credential fields and malformed bindings are rejected. No public path accepts a caller-provided authorization flag.

Check results contain `version`, `exitCode`, and the original `bundle`. Exit codes are 0 ALLOW, 2 BLOCK, 3 REQUIRE_APPROVAL, 4 failure/cancellation, with the exact underlying core semantics. Unbound failures contain `diagnostic` and `bundle:null`; no envelope is invented. Bound failures/cancellation retain the core envelope with null decision. Diagnostic strings are fixed and never echo raw requests or Git stderr. The API function `api::dispatch(bytes, &AtomicBool)` is also available in-process. No HTTP service or credentials are required.

## MCP profile

The explicit `mcp-stdio` process implements a conservative local subset of MCP **2025-11-25**, using newline-delimited JSON-RPC 2.0. It does not install/register a host service or open a network listener. Follow initialize → notifications/initialized → tools/list or tools/call. Initialize requires a nonempty client protocol version, empty client capabilities, and nonempty clientInfo name/version. The server always proposes its supported 2025-11-25 version; a client requesting another version must decide whether to accept it or disconnect. Tools remain unavailable until notifications/initialized. Other client capability profiles are explicitly unsupported rather than silently enabling callbacks. Ping is supported. Unsupported methods return protocol errors; notifications never receive responses.

Only `gitguard_check` is advertised. Tool arguments are the same versioned API request with capability `check`. `tools/call` returns the API result in `structuredContent` and equivalent serialized JSON text. `isError` is true only for common exit 4; BLOCK and REQUIRE_APPROVAL are valid analysis results. The MCP process itself exits 0 at clean EOF independently of tool decisions. An over-limit transport or output failure exits 4; malformed JSON-RPC produces a protocol error and leaves the session available. Frames are capped at 1 MiB; oversized frames close the session after one protocol error, without reading/executing subsequent frames. Duplicate object fields, invalid IDs, unknown API versions, pre-initialization tools and unknown/write tools are rejected.

No resources, prompts, sampling, task execution, write capabilities or credential acquisition are advertised. The single tool has readOnlyHint=true and destructiveHint=false. These metadata hints are descriptive, not an authorization boundary; actual behavior is enforced by the shared read-only core and absence of a write dispatcher. Existing optional `privileged-execution` feature still cannot authorize writes. This port does not implement a production MCP identity/provider, remote transport, live multi-request cancellation, or hostile same-UID filesystem isolation. In-process callers can supply the existing cancellation token; the sequential stdio adapter does not claim asynchronous MCP cancellation support.

The core can use private temporary objects and run the bounded local Git process; it must not mutate the source repository refs/index/worktree. ALLOW is advisory analysis and never a grant to mutate a repository. A local caller chooses the explicit repository path, as with the original CLI; this is not a multitenant repository access-control service.

## Verification and compatibility

`tests/interface_parity.rs` runs real local Git fixtures through the legacy CLI, the versioned API, and MCP function/executable. It compares domain scope, binding, coverage, status, diagnostics, engine contract/facts/report and decisions, excluding only distinct run IDs/timestamps outside these fields. It covers enforce/review, allowed/violating/dirty scopes, cancellation, errors, negotiation and budgets and compares exact repository file bytes before/after. Existing tests remain intact. Task 5.3 remains pending independent review until separately registered.

Protocol sources: [lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle), [tools](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), [stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports). These references specify the transport/tool conventions; GitGuard's supported profile and unsupported capabilities are defined above.
