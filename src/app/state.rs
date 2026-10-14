use cosmic::app::Core;
use cosmic::widget::about::About;
use cosmic::widget::nav_bar;
use std::path::PathBuf;
use crate::{db, diagnostic, services, systems};
use super::page::Page;

pub struct BatoceraManagerApp {
    pub core: Core,
    pub nav_model: nav_bar::Model,
    pub active_page: Page,
    pub about: About,
    pub show_about: bool,

    // Parámetros de red Wi-Fi y Batocera
    pub is_connected: bool,
    pub host_ip: String,
    pub ssh_port: u16,
    pub ssh_port_str: String,
    pub ssh_user: String,
    pub ssh_password: String,
    pub local_base_dir: PathBuf,
    pub local_base_dir_str: String,
    pub auto_sync: bool,
    pub wifi_test_result: Option<Result<String, String>>,

    // Base de datos y Sincronización
    pub db_path: PathBuf,
    pub db_path_str: String,
    pub sync_summary: db::SyncSummary,
    pub sync_records: Vec<db::SyncFileRecord>,
    pub selected_category: String,
    pub is_syncing: bool,

    // Sistemas / Consolas
    pub systems: Vec<systems::SystemConfig>,

    // Explorador de Juegos
    pub selected_system_index: usize,
    pub games_list: Vec<systems::GameItem>,
    pub is_copying_games: bool,
    pub show_copy_confirmation: bool,
    pub rom_search_query: String,

    // Diagnóstico
    pub is_diagnosing: bool,
    pub latest_diagnostic: Option<diagnostic::DiagnosticReport>,
    pub show_waste_confirmation: bool,

    // Conversor CHD
    pub chdman_version: Option<String>,
    pub single_cue_path: String,
    pub single_output_path: String,
    pub batch_folder_path: String,
    pub found_cues: Vec<PathBuf>,
    pub force_overwrite: bool,
    pub delete_source_after: bool,
    pub is_converting: bool,
    pub log_output: String,
}

impl BatoceraManagerApp {
    pub fn reload_sync_db(&mut self) {
        if let Ok(conn) = db::init_db(&self.db_path) {
            self.sync_summary = db::get_sync_summary(&conn).unwrap_or_default();
            let cat_filter = if self.selected_category == "all" {
                None
            } else {
                Some(self.selected_category.as_str())
            };
            self.sync_records = db::get_all_records(&conn, cat_filter).unwrap_or_default();
        }
    }

    pub fn rescan_local_systems(&mut self) {
        for sys in &mut self.systems {
            sys.local_rom_count = services::library::count_files_in_dir(&sys.local_folder);
        }
    }

    pub fn reload_games_for_selected_system(&mut self) {
        if let Some(sys) = self.systems.get(self.selected_system_index) {
            let prefix = format!("{}/", sys.remote_folder);
            let remote_names: Vec<String> = self
                .sync_records
                .iter()
                .filter(|r| r.relative_path.starts_with(&prefix))
                .filter_map(|r| {
                    if r.remote_size.is_some()
                        || r.status == db::SyncStatus::InSync
                        || r.status == db::SyncStatus::OnlyRemote
                    {
                        r.relative_path.strip_prefix(&prefix).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            self.games_list = services::library::scan_games_for_system(&sys.local_folder, &remote_names);
        }
    }

    pub fn ssh_config(&self) -> crate::net::SshConfig {
        crate::net::SshConfig {
            host: self.host_ip.clone(),
            port: self.ssh_port,
            user: self.ssh_user.clone(),
            password: self.ssh_password.clone(),
        }
    }
}

