//! Usage: CLI config snapshots for MCP sync rollback (best-effort restore on failure).

use crate::mcp_sync;

use super::cli_specs::MCP_CLI_SPECS;

#[derive(Debug)]
pub(super) struct SingleCliBackup {
    target: Option<Vec<u8>>,
    manifest: Option<Vec<u8>>,
}

impl SingleCliBackup {
    pub(super) fn capture(app: &tauri::AppHandle, cli_key: &str) -> Result<Self, String> {
        Ok(Self {
            target: mcp_sync::read_target_bytes(app, cli_key)?,
            manifest: mcp_sync::read_manifest_bytes(app, cli_key)?,
        })
    }

    pub(super) fn restore(self, app: &tauri::AppHandle, cli_key: &str) {
        let _ = mcp_sync::restore_target_bytes(app, cli_key, self.target);
        let _ = mcp_sync::restore_manifest_bytes(app, cli_key, self.manifest);
    }
}

#[derive(Debug)]
pub(super) struct CliBackupSnapshots(Vec<(&'static str, SingleCliBackup)>);

impl CliBackupSnapshots {
    pub(super) fn capture_all(app: &tauri::AppHandle) -> Result<Self, String> {
        let mut out = Vec::with_capacity(MCP_CLI_SPECS.len());
        for spec in MCP_CLI_SPECS {
            out.push((spec.cli_key, SingleCliBackup::capture(app, spec.cli_key)?));
        }
        Ok(Self(out))
    }

    pub(super) fn restore_all(self, app: &tauri::AppHandle) {
        for (cli_key, backup) in self.0 {
            backup.restore(app, cli_key);
        }
    }
}
