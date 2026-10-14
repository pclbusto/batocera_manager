use crate::domain::game::GameItem;
use crate::domain::sync::SyncSummary;
use crate::net::SshConfig;
use crate::sync;
use crate::systems;
use std::path::Path;

pub fn copy_selected_games_to_console(
    games: &[GameItem],
    remote_folder: &str,
    ssh_config: &SshConfig,
    db_path: &Path,
) -> Result<String, String> {
    systems::copy_selected_games_to_console(games, remote_folder, ssh_config, db_path)
}

pub fn scan_and_reconcile(
    db_path: &Path,
    local_base: &Path,
    ssh_config: &SshConfig,
) -> Result<SyncSummary, String> {
    sync::scan_and_reconcile(db_path, local_base, ssh_config)
}

pub fn sync_saves_bidirectional(
    db_path: &Path,
    local_base: &Path,
    ssh_config: &SshConfig,
) -> Result<String, String> {
    sync::sync_saves_bidirectional(db_path, local_base, ssh_config)
}
