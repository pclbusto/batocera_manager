use cosmic::app::Task;
use crate::domain::sync::SyncSummary;
use crate::services;
use crate::app::message::Message;
use crate::app::state::BatoceraManagerApp;

pub fn handle_test_wifi(app: &mut BatoceraManagerApp) -> Task<Message> {
    let config = app.ssh_config();
    app.log_output.push_str(&format!("\n📶 Probando conexión Wi-Fi a {}:{}...\n", config.host, config.port));

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || services::connection::test_connection(&config))
                .await
                .unwrap_or_else(|e| Err(format!("Fallo en hilo: {e}")))
        },
        |res| cosmic::Action::App(Message::WifiConnectionTested(res)),
    )
}

pub fn handle_wifi_tested(app: &mut BatoceraManagerApp, result: Result<String, String>) -> Task<Message> {
    match &result {
        Ok(info) => {
            app.is_connected = true;
            app.log_output.push_str(&format!("✅ Conexión establecida: {info}\n"));
        }
        Err(err) => {
            app.is_connected = false;
            app.log_output.push_str(&format!("❌ Fallo de conexión: {err}\n"));
        }
    }
    app.wifi_test_result = Some(result);
    Task::none()
}

pub fn handle_start_scan_and_reconcile(app: &mut BatoceraManagerApp) -> Task<Message> {
    if app.is_syncing {
        return Task::none();
    }
    app.is_syncing = true;
    let db_path = app.db_path.clone();
    let local_base = app.local_base_dir.clone();
    let config = app.ssh_config();

    app.log_output.push_str(&format!(
        "\n🔍 Iniciando escaneo e indexación con hashes BLAKE3...\n   Local: {}\n   Remoto: {}@{}:{}\n",
        local_base.display(), config.user, config.host, config.port
    ));

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::transfer::scan_and_reconcile(&db_path, &local_base, &config)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
        },
        |res| cosmic::Action::App(Message::ScanAndReconcileFinished(res)),
    )
}

pub fn handle_scan_and_reconcile_finished(app: &mut BatoceraManagerApp, result: Result<SyncSummary, String>) -> Task<Message> {
    app.is_syncing = false;
    match result {
        Ok(summary) => {
            app.sync_summary = summary.clone();
            app.reload_sync_db();
            app.log_output.push_str(&format!(
                "✅ Escaneo completado. Total: {}, Sincronizados: {}, Nuevos en Anbernic: {}, Nuevos en PC: {}\n",
                summary.total, summary.in_sync, summary.remote_newer + summary.only_remote, summary.local_newer + summary.only_local
            ));
        }
        Err(e) => {
            app.log_output.push_str(&format!("❌ Error durante el escaneo: {e}\n"));
        }
    }
    Task::none()
}

pub fn handle_start_sync_saves(app: &mut BatoceraManagerApp) -> Task<Message> {
    if app.is_syncing {
        return Task::none();
    }
    app.is_syncing = true;
    let db_path = app.db_path.clone();
    let local_base = app.local_base_dir.clone();
    let config = app.ssh_config();

    app.log_output.push_str("\n💾 Sincronizando partidas guardadas (Saves) bidireccionalmente...\n");

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::transfer::sync_saves_bidirectional(&db_path, &local_base, &config)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
        },
        |res| cosmic::Action::App(Message::SyncSavesFinished(res)),
    )
}

pub fn handle_sync_saves_finished(app: &mut BatoceraManagerApp, result: Result<String, String>) -> Task<Message> {
    app.is_syncing = false;
    match result {
        Ok(msg) => {
            app.reload_sync_db();
            app.log_output.push_str(&format!("✅ {msg}\n"));
        }
        Err(e) => {
            app.log_output.push_str(&format!("❌ Error sincronizando partidas: {e}\n"));
        }
    }
    Task::none()
}

pub fn handle_filter_category(app: &mut BatoceraManagerApp, cat: String) -> Task<Message> {
    app.selected_category = cat;
    app.reload_sync_db();
    Task::none()
}
