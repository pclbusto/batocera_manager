use cosmic::app::Task;
use crate::domain::game::{format_size, GameItem};
use crate::services;
use crate::app::message::Message;
use crate::app::state::BatoceraManagerApp;

pub fn handle_search_roms(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    app.rom_search_query = val;
    Task::none()
}

pub fn handle_select_system_for_games(app: &mut BatoceraManagerApp, idx: usize) -> Task<Message> {
    app.selected_system_index = idx;
    app.reload_games_for_selected_system();
    Task::none()
}

pub fn handle_toggle_game_selection(app: &mut BatoceraManagerApp, idx: usize, val: bool) -> Task<Message> {
    if let Some(game) = app.games_list.get_mut(idx) {
        game.selected = val;
    }
    Task::none()
}

pub fn handle_select_all_missing(app: &mut BatoceraManagerApp) -> Task<Message> {
    for game in &mut app.games_list {
        game.selected = !game.exists_on_console;
    }
    Task::none()
}

pub fn handle_deselect_all(app: &mut BatoceraManagerApp) -> Task<Message> {
    for game in &mut app.games_list {
        game.selected = false;
    }
    Task::none()
}

pub fn handle_refresh_games_list(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.reload_games_for_selected_system();
    Task::none()
}

pub fn handle_copy_selected_games(app: &mut BatoceraManagerApp) -> Task<Message> {
    let selected_count = app.games_list.iter().filter(|g| g.selected).count();
    if selected_count > 0 {
        app.show_copy_confirmation = true;
    }
    Task::none()
}

pub fn handle_cancel_copy_confirmation(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.show_copy_confirmation = false;
    Task::none()
}

pub fn handle_confirm_copy_selected_games(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.show_copy_confirmation = false;
    if app.is_copying_games || app.selected_system_index >= app.systems.len() {
        return Task::none();
    }
    let selected_games: Vec<GameItem> = app
        .games_list
        .iter()
        .filter(|g| g.selected)
        .cloned()
        .collect();

    if selected_games.is_empty() {
        return Task::none();
    }

    app.is_copying_games = true;
    let sys = app.systems[app.selected_system_index].clone();
    let db_path = app.db_path.clone();
    let config = app.ssh_config();

    let total_size: u64 = selected_games.iter().map(|g| g.size_bytes).sum();
    app.log_output.push_str(&format!(
        "\n🚀 Iniciando transferencia verificada de {} juego(s) ({}) de '{}' hacia la Anbernic (/userdata/{}/)...\n",
        selected_games.len(), format_size(total_size), sys.name, sys.remote_folder
    ));

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::transfer::copy_selected_games_to_console(&selected_games, &sys.remote_folder, &config, &db_path)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en hilo: {e}")))
        },
        |res| cosmic::Action::App(Message::CopySelectedGamesFinished(res)),
    )
}

pub fn handle_copy_selected_finished(app: &mut BatoceraManagerApp, result: Result<String, String>) -> Task<Message> {
    app.is_copying_games = false;
    match result {
        Ok(msg) => {
            app.log_output.push_str(&format!("✅ {msg}\n"));
            app.reload_games_for_selected_system();
            app.reload_sync_db();
        }
        Err(err) => {
            app.log_output.push_str(&format!("❌ {err}\n"));
        }
    }
    Task::none()
}
