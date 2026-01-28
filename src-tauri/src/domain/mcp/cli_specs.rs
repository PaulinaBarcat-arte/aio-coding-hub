//! Usage: Table-driven MCP CLI mappings (cli_key -> db columns).

#[derive(Debug, Clone, Copy)]
pub(super) struct McpCliSpec {
    pub(super) cli_key: &'static str,
    pub(super) enabled_column: &'static str,
}

pub(super) const MCP_CLI_SPECS: [McpCliSpec; 3] = [
    McpCliSpec {
        cli_key: "claude",
        enabled_column: "enabled_claude",
    },
    McpCliSpec {
        cli_key: "codex",
        enabled_column: "enabled_codex",
    },
    McpCliSpec {
        cli_key: "gemini",
        enabled_column: "enabled_gemini",
    },
];

pub(super) fn spec_for_cli_key(cli_key: &str) -> Result<McpCliSpec, String> {
    crate::shared::cli_key::validate_cli_key(cli_key)?;
    MCP_CLI_SPECS
        .iter()
        .copied()
        .find(|spec| spec.cli_key == cli_key)
        .ok_or_else(|| format!("SEC_INVALID_INPUT: unknown cli_key={cli_key}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn mcp_cli_specs_cover_all_supported_cli_keys() {
        let spec_keys: HashSet<&'static str> = MCP_CLI_SPECS.iter().map(|s| s.cli_key).collect();
        for cli_key in crate::shared::cli_key::SUPPORTED_CLI_KEYS {
            assert!(spec_keys.contains(cli_key));
            let spec = spec_for_cli_key(cli_key).expect("spec_for_cli_key");
            assert_eq!(spec.cli_key, cli_key);
            assert_eq!(spec.enabled_column, format!("enabled_{cli_key}"));
        }
    }

    #[test]
    fn mcp_cli_specs_have_unique_cli_keys_and_columns() {
        let mut keys = HashSet::new();
        let mut cols = HashSet::new();
        for spec in MCP_CLI_SPECS {
            assert!(keys.insert(spec.cli_key));
            assert!(cols.insert(spec.enabled_column));
        }
    }
}
