use crate::domain::game::GameItem;
use crate::domain::system::SystemConfig;
use crate::net::SshConfig;
use crate::systems;
use std::path::Path;

pub fn count_files_in_dir(dir: &Path) -> usize {
    systems::count_files_in_dir(dir)
}

pub fn default_systems_list(base_roms: &Path) -> Vec<SystemConfig> {
    systems::default_systems_list(base_roms)
}

pub fn scan_games_for_system(local_folder: &Path, remote_names: &[String]) -> Vec<GameItem> {
    systems::scan_games_for_system(local_folder, remote_names)
}

pub fn sync_system(
    system: &SystemConfig,
    ssh_config: &SshConfig,
    db_path: &Path,
) -> Result<String, String> {
    systems::sync_system(system, ssh_config, db_path)
}

pub fn sync_all_systems(
    systems: &[SystemConfig],
    ssh_config: &SshConfig,
    db_path: &Path,
) -> Vec<(String, Result<String, String>)> {
    let mut results = Vec::new();
    for sys in systems {
        let res = systems::sync_system(sys, ssh_config, db_path);
        results.push((sys.name.clone(), res));
    }
    results
}
