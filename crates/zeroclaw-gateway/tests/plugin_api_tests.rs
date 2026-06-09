//! Gateway plugin API contract tests (#7320).
//!
//! Run with: cargo test -p zeroclaw-gateway --features plugins-wasm

#[cfg(all(test, feature = "plugins-wasm"))]
mod plugin_api_contract {
    // TODO: spin up a test gateway with a temp plugin dir and assert:
    // - GET /api/plugins returns 200 with correct JSON schema.
    // - Response contains plugins_enabled, plugins_dir, plugins array.
    // - Each plugin entry has name, version, description, capabilities, loaded.
}

#[cfg(test)]
mod mcp_api_contract {
    // TODO: spin up a test gateway and assert:
    // - GET /api/mcp/servers returns 200 and matches McpServerEntry schema.
    // - GET /api/mcp/bundles returns 200 and matches McpBundleEntry schema.
}
