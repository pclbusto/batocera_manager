use cosmic::app::Task;
use std::path::PathBuf;
use crate::db;
use crate::domain::system::SystemConfig;
use crate::services;
use crate::app::message::Message;
use crate::app::state::BatoceraManagerApp;

pub fn handle_toggle_system(app: &mut BatoceraManagerApp, idx: usize, enabled: bool) -> Task<Message> {
    if let Some(sys) = app.systems.get_mut(idx) {
        sys.enabled = enabled;
        if let Ok(conn) = db::init_db(&app.db_path) {
            let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
        }
    }
    Task::none()
}

pub fn handle_system_folder_input(app: &mut BatoceraManagerApp, idx: usize, folder_str: String) -> Task<Message> {
    if let Some(sys) = app.systems.get_mut(idx) {
        sys.local_folder = PathBuf::from(&folder_str);
        sys.local_folder_str = folder_str;
        sys.local_rom_count = services::library::count_files_in_dir(&sys.local_folder);
        if let Ok(conn) = db::init_db(&app.db_path) {
            let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
        }
    }
    if idx == app.selected_system_index {
        app.reload_games_for_selected_system();
    }
    Task::none()
}

pub fn handle_pick_system_folder(idx: usize) -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(|| {
                rfd::FileDialog::new().pick_folder()
            })
            .await
            .ok()
            .flatten()
        },
        move |res| cosmic::Action::App(Message::SystemFolderSelected(idx, res)),
    )
}

pub fn handle_system_folder_selected(app: &mut BatoceraManagerApp, idx: usize, opt: Option<PathBuf>) -> Task<Message> {
    if let Some(path) = opt {
        if let Some(sys) = app.systems.get_mut(idx) {
            sys.local_folder_str = path.display().to_string();
            sys.local_folder = path;
            sys.local_rom_count = services::library::count_files_in_dir(&sys.local_folder);
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
            }
        }
        if idx == app.selected_system_index {
            app.reload_games_for_selected_system();
        }
    }
    Task::none()
}

pub fn handle_rescan_systems(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.rescan_local_systems();
    app.reload_games_for_selected_system();
    app.log_output.push_str("\n🔄 Carpetas de sistemas re-escaneadas.\n");
    Task::none()
}

pub fn handle_sync_single_system(app: &mut BatoceraManagerApp, idx: usize) -> Task<Message> {
    if app.is_syncing || idx >= app.systems.len() {
        return Task::none();
    }
    app.is_syncing = true;
    let sys = app.systems[idx].clone();
    let db_path = app.db_path.clone();
    let config = app.ssh_config();

    app.log_output.push_str(&format!(
        "\n▶ Sincronizando sistema '{}'...\n   Carpeta local: {}\n   Destino remoto: /userdata/{}\n",
        sys.name, sys.local_folder.display(), sys.remote_folder
    ));

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::library::sync_system(&sys, &config, &db_path)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
        },
        move |res| cosmic::Action::App(Message::SingleSystemSyncFinished(idx, res)),
    )
}

pub fn handle_single_system_sync_finished(
    app: &mut BatoceraManagerApp,
    idx: usize,
    result: Result<String, String>,
) -> Task<Message> {
    app.is_syncing = false;
    match result {
        Ok(msg) => {
            if let Some(sys) = app.systems.get_mut(idx) {
                sys.status_msg = "Sincronizado".to_string();
            }
            app.reload_sync_db();
            app.log_output.push_str(&format!("✅ {msg}\n"));
        }
        Err(e) => {
            if let Some(sys) = app.systems.get_mut(idx) {
                sys.status_msg = format!("Error: {e}");
            }
            app.log_output.push_str(&format!("❌ {e}\n"));
        }
    }
    Task::none()
}

pub fn handle_sync_all_systems(app: &mut BatoceraManagerApp) -> Task<Message> {
    if app.is_syncing {
        return Task::none();
    }
    app.is_syncing = true;
    let active_systems: Vec<SystemConfig> = app
        .systems
        .iter()
        .filter(|s| s.enabled)
        .cloned()
        .collect();

    let db_path = app.db_path.clone();
    let config = app.ssh_config();

    app.log_output.push_str(&format!(
        "\n▶ Sincronizando {} sistemas habilitados por Wi-Fi...\n",
        active_systems.len()
    ));

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::library::sync_all_systems(&active_systems, &config, &db_path)
            })
            .await
            .unwrap_or_default()
        },
        |res| cosmic::Action::App(Message::AllSystemsSyncFinished(res)),
    )
}

pub fn handle_all_systems_sync_finished(
    app: &mut BatoceraManagerApp,
    results: Vec<(String, Result<String, String>)>,
) -> Task<Message> {
    app.is_syncing = false;
    for (name, res) in results {
        match res {
            Ok(msg) => {
                app.log_output.push_str(&format!("  [OK] {msg}\n"));
            }
            Err(err) => {
                app.log_output.push_str(&format!("  [ERROR] {name}: {err}\n"));
            }
        }
    }
    app.reload_sync_db();
    app.log_output.push_str("🏁 Sincronización de sistemas finalizada.\n");
    Task::none()
}
