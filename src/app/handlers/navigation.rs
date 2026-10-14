use cosmic::app::Task;
use cosmic::{Application, ApplicationExt};
use std::path::PathBuf;
use crate::db;
use crate::services;
use crate::app::message::Message;
use crate::app::state::BatoceraManagerApp;

pub fn handle_nav_select(app: &mut BatoceraManagerApp, id: cosmic::widget::nav_bar::Id) -> Task<Message> {
    app.on_nav_select(id)
}

pub fn handle_toggle_about(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.show_about = !app.show_about;
    app.set_show_context(app.show_about);
    Task::none()
}

pub fn handle_open_url(url: String) -> Task<Message> {
    let _ = open::that_detached(url);
    Task::none()
}

pub fn handle_clear_logs(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.log_output.clear();
    Task::none()
}

pub fn handle_toggle_connection(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.is_connected = !app.is_connected;
    Task::none()
}

pub fn handle_host_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    if let Ok(conn) = db::init_db(&app.db_path) {
        let _ = db::save_setting(&conn, "host_ip", &val);
    }
    app.host_ip = val;
    Task::none()
}

pub fn handle_port_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    app.ssh_port_str = val.clone();
    if let Ok(p) = val.parse::<u16>() {
        app.ssh_port = p;
    }
    if let Ok(conn) = db::init_db(&app.db_path) {
        let _ = db::save_setting(&conn, "ssh_port", &val);
    }
    Task::none()
}

pub fn handle_user_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    if let Ok(conn) = db::init_db(&app.db_path) {
        let _ = db::save_setting(&conn, "ssh_user", &val);
    }
    app.ssh_user = val;
    Task::none()
}

pub fn handle_password_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    if let Ok(conn) = db::init_db(&app.db_path) {
        let _ = db::save_setting(&conn, "ssh_password", &val);
    }
    app.ssh_password = val;
    Task::none()
}

pub fn handle_local_folder_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    if let Ok(conn) = db::init_db(&app.db_path) {
        let _ = db::save_setting(&conn, "local_base_dir", &val);
    }
    app.local_base_dir_str = val.clone();
    app.local_base_dir = PathBuf::from(val);
    Task::none()
}

pub fn handle_auto_sync_toggled(app: &mut BatoceraManagerApp, val: bool) -> Task<Message> {
    app.auto_sync = val;
    Task::none()
}

pub fn handle_refresh_status(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.chdman_version = services::converter::detect_chdman();
    app.reload_sync_db();
    app.rescan_local_systems();
    app.reload_games_for_selected_system();
    Task::none()
}

pub fn handle_pick_local_folder() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(|| {
                rfd::FileDialog::new().pick_folder()
            })
            .await
            .ok()
            .flatten()
        },
        |res| cosmic::Action::App(Message::LocalFolderSelected(res)),
    )
}

pub fn handle_local_folder_selected(app: &mut BatoceraManagerApp, opt: Option<PathBuf>) -> Task<Message> {
    if let Some(path) = opt {
        app.local_base_dir_str = path.display().to_string();
        app.local_base_dir = path;
        if let Ok(conn) = db::init_db(&app.db_path) {
            let _ = db::save_setting(&conn, "local_base_dir", &app.local_base_dir_str);
        }
        app.reload_sync_db();
        app.rescan_local_systems();
        app.reload_games_for_selected_system();
    }
    Task::none()
}
