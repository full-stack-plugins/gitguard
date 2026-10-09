# GitGuard MCP local adapter

`mod.rs` is compiled as `gitguard::mcp` and used by the explicit `gitguard mcp-stdio` command. It wraps the existing read-only API and never starts an HTTP service or requests write credentials.

See [versioned local ports](../../docs/read-only-interface-ports.md) for the exact supported MCP profile, framing and capability restrictions. No host registration is performed automatically.
