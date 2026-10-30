# Install owns guidance; init owns XML initialization

The runtime `install` command is the sole writer of its `GITGUARD_START/END` AGENTS block. The block identifies `owner: gitguard-runtime-install/v1`; the existing installation receipt owns its exact content and MCP nodes. Repeating installation is idempotent. Unknown, malformed or user-modified managed blocks are rejected without partial writes. Outside user text remains untouched. Explicit `instructions sync` can refresh a known receipt-owned old block.

The plugin `init` command only selects a GitFlow profile and initializes `<git-common-dir>/gitflow.xml`. It does not create, read, validate, migrate or update AGENTS as a configuration step. An absent file remains absent; arbitrary existing bytes, malformed markers, symlinks and nonregular AGENTS paths are not prerequisites. The abandoned init-owned marker and guidance migration flags are removed. Explicit legacy JSON policy migration also concerns XML only.

`tests/guidance_coexistence.py` executes the actual Python and Rust entrypoints for both orders, repeated execution, absent guidance, unknown/edited/broken blocks and known receipt refresh. Init must leave AGENTS bytes unchanged; installation alone creates one managed block. It never calls a real Agent host or accepts trust.

The exported runtime context is a versioned XML reference (status/profile/revision/digest/location), not a duplicated role or merge-rule body. Policy checks still evaluate the complete XML. Installation before XML initialization can leave a stale reference summary after init; explicit installation/instructions sync refreshes that summary. This does not block XML initialization or authorize a Git write.

`tests/run_installation_xml.py` preserves all ten pinned shared test method names and adds a responsibility test. Only the old JSON fixture is overridden: real init in staging repositories generates XML without AGENTS, the independent runtime fixture uses that XML, and its ordinary install/refresh/MCP checks remain. Same-project and bidirectional tests separately verify normal usage; staging is not a workaround for a retained conflict.

Entrypoints are intentionally explicit in current documentation: the Python `bin/gitguard install --bin-dir` helper bootstraps a launcher only; the Rust executable's `install --agents ... --project ...` performs project installation and writes guidance. Neither success is advertised as actual host activation or automatic Git-hook enforcement.
