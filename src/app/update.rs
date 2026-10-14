use cosmic::app::Task;
use cosmic::{Application, ApplicationExt};
use std::path::PathBuf;
use crate::{chd, db, diagnostic, net, sync, systems};
use super::message::Message;
use super::state::BatoceraManagerApp;

pub fn handle_message(app: &mut BatoceraManagerApp, message: Message) -> Task<Message> {
    match message {
        Message::NavSelect(id) => {
            return app.on_nav_select(id);
        }
        Message::ToggleAbout => {
            app.show_about = !app.show_about;
            app.set_show_context(app.show_about);
        }
        Message::OpenUrl(url) => {
            let _ = open::that_detached(url);
        }
        Message::ToggleConnection => {
            app.is_connected = !app.is_connected;
        }
        Message::HostChanged(val) => {
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_setting(&conn, "host_ip", &val);
            }
            app.host_ip = val;
        }
        Message::PortChanged(val) => {
            app.ssh_port_str = val.clone();
            if let Ok(p) = val.parse::<u16>() {
                app.ssh_port = p;
            }
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_setting(&conn, "ssh_port", &val);
            }
        }
        Message::UserChanged(val) => {
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_setting(&conn, "ssh_user", &val);
            }
            app.ssh_user = val;
        }
        Message::PasswordChanged(val) => {
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_setting(&conn, "ssh_password", &val);
            }
            app.ssh_password = val;
        }
        Message::LocalFolderChanged(val) => {
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_setting(&conn, "local_base_dir", &val);
            }
            app.local_base_dir_str = val.clone();
            app.local_base_dir = PathBuf::from(val);
        }
        Message::AutoSyncToggled(val) => {
            app.auto_sync = val;
        }
        Message::SearchRoms(val) => {
            app.rom_search_query = val;
        }
        Message::RefreshStatus => {
            app.chdman_version = chd::detect_chdman();
            app.reload_sync_db();
            app.rescan_local_systems();
            app.reload_games_for_selected_system();
        }

        // Sincronización por Sistemas
        Message::ToggleSystem(idx, enabled) => {
            if let Some(sys) = app.systems.get_mut(idx) {
                sys.enabled = enabled;
                if let Ok(conn) = db::init_db(&app.db_path) {
                    let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
                }
            }
        }
        Message::SystemFolderInputChanged(idx, folder_str) => {
            if let Some(sys) = app.systems.get_mut(idx) {
                sys.local_folder = PathBuf::from(&folder_str);
                sys.local_folder_str = folder_str;
                sys.local_rom_count = systems::count_files_in_dir(&sys.local_folder);
                if let Ok(conn) = db::init_db(&app.db_path) {
                    let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
                }
            }
            if idx == app.selected_system_index {
                app.reload_games_for_selected_system();
            }
        }
        Message::PickSystemFolder(idx) => {
            return Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new().pick_folder()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                move |res| cosmic::Action::App(Message::SystemFolderSelected(idx, res)),
            );
        }
        Message::SystemFolderSelected(idx, Some(path)) => {
            if let Some(sys) = app.systems.get_mut(idx) {
                sys.local_folder_str = path.display().to_string();
                sys.local_folder = path;
                sys.local_rom_count = systems::count_files_in_dir(&sys.local_folder);
                if let Ok(conn) = db::init_db(&app.db_path) {
                    let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
                }
            }
            if idx == app.selected_system_index {
                app.reload_games_for_selected_system();
            }
        }
        Message::SystemFolderSelected(_idx, None) => {}
        Message::RescanSystems => {
            app.rescan_local_systems();
            app.reload_games_for_selected_system();
            app.log_output.push_str("\n🔄 Carpetas de sistemas re-escaneadas.\n");
        }
        Message::SyncSingleSystem(idx) => {
            if app.is_syncing || idx >= app.systems.len() {
                return Task::none();
            }
            app.is_syncing = true;
            let sys = app.systems[idx].clone();
            let db_path = app.db_path.clone();
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };

            app.log_output.push_str(&format!(
                "\n▶ Sincronizando sistema '{}'...\n   Carpeta local: {}\n   Destino remoto: /userdata/{}\n",
                sys.name, sys.local_folder.display(), sys.remote_folder
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        systems::sync_system(&sys, &config, &db_path)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
                },
                move |res| cosmic::Action::App(Message::SingleSystemSyncFinished(idx, res)),
            );
        }
        Message::SingleSystemSyncFinished(idx, result) => {
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
        }
        Message::SyncAllEnabledSystems => {
            if app.is_syncing {
                return Task::none();
            }
            app.is_syncing = true;
            let active_systems: Vec<systems::SystemConfig> = app
                .systems
                .iter()
                .filter(|s| s.enabled)
                .cloned()
                .collect();

            let db_path = app.db_path.clone();
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };

            app.log_output.push_str(&format!(
                "\n▶ Sincronizando {} sistemas habilitados por Wi-Fi...\n",
                active_systems.len()
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let mut results = Vec::new();
                        for sys in active_systems {
                            let res = systems::sync_system(&sys, &config, &db_path);
                            results.push((sys.name.clone(), res));
                        }
                        results
                    })
                    .await
                    .unwrap_or_default()
                },
                |res| cosmic::Action::App(Message::AllSystemsSyncFinished(res)),
            );
        }
        Message::AllSystemsSyncFinished(results) => {
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
        }

        // Explorador de Juegos (Copiado Selectivo)
        Message::SelectSystemForGames(idx) => {
            app.selected_system_index = idx;
            app.reload_games_for_selected_system();
        }
        Message::ToggleGameSelection(idx, val) => {
            if let Some(game) = app.games_list.get_mut(idx) {
                game.selected = val;
            }
        }
        Message::SelectAllMissingGames => {
            for game in &mut app.games_list {
                game.selected = !game.exists_on_console;
            }
        }
        Message::DeselectAllGames => {
            for game in &mut app.games_list {
                game.selected = false;
            }
        }
        Message::RefreshGamesList => {
            app.reload_games_for_selected_system();
        }
        Message::CopySelectedGamesToConsole => {
            let selected_count = app.games_list.iter().filter(|g| g.selected).count();
            if selected_count > 0 {
                app.show_copy_confirmation = true;
            }
        }
        Message::CancelCopyConfirmation => {
            app.show_copy_confirmation = false;
        }
        Message::ConfirmCopySelectedGames => {
            app.show_copy_confirmation = false;
            if app.is_copying_games || app.selected_system_index >= app.systems.len() {
                return Task::none();
            }
            let selected_games: Vec<systems::GameItem> = app
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
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };

            let total_size: u64 = selected_games.iter().map(|g| g.size_bytes).sum();
            app.log_output.push_str(&format!(
                "\n🚀 Iniciando transferencia verificada de {} juego(s) ({}) de '{}' hacia la Anbernic (/userdata/{}/)...\n",
                selected_games.len(), systems::format_size(total_size), sys.name, sys.remote_folder
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        systems::copy_selected_games_to_console(&selected_games, &sys.remote_folder, &config, &db_path)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en hilo: {e}")))
                },
                |res| cosmic::Action::App(Message::CopySelectedGamesFinished(res)),
            );
        }
        Message::CopySelectedGamesFinished(result) => {
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
        }

        // Diagnóstico de Almacenamiento y Limpieza Segura
        Message::RunDiagnostic => {
            if app.is_diagnosing {
                return Task::none();
            }
            app.is_diagnosing = true;
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };
            let sys_list = app.systems.clone();
            app.log_output.push_str("\n🔍 Iniciando diagnóstico no destructivo de almacenamiento en la consola...\n");

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let sess = net::create_session(&config)?;
                        let sftp = sess.sftp().map_err(|e| format!("Error abriendo SFTP: {e}"))?;
                        diagnostic::run_storage_diagnostic(&sess, &sftp, &sys_list)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
                },
                |res| cosmic::Action::App(Message::DiagnosticFinished(res)),
            );
        }
        Message::DiagnosticFinished(res) => {
            app.is_diagnosing = false;
            match res {
                Ok(report) => {
                    app.log_output.push_str(&report.report_text);
                    app.latest_diagnostic = Some(report);
                }
                Err(e) => {
                    app.log_output.push_str(&format!("❌ Error durante el diagnóstico: {e}\n"));
                }
            }
        }
        Message::ToggleWasteCandidate(idx, val) => {
            if let Some(ref mut diag) = app.latest_diagnostic {
                if let Some(candidate) = diag.waste_candidates.get_mut(idx) {
                    candidate.selected = val;
                }
            }
        }
        Message::SelectAllWaste(val) => {
            if let Some(ref mut diag) = app.latest_diagnostic {
                for c in &mut diag.waste_candidates {
                    c.selected = val;
                }
            }
        }
        Message::RequestCleanWaste => {
            let selected_count = app
                .latest_diagnostic
                .as_ref()
                .map(|d| d.waste_candidates.iter().filter(|w| w.selected).count())
                .unwrap_or(0);
            if selected_count > 0 {
                app.show_waste_confirmation = true;
            } else {
                app.log_output.push_str("⚠️ No hay residuos seleccionados para eliminar. Marca los archivos que deseas limpiar.\n");
            }
        }
        Message::CancelCleanWaste => {
            app.show_waste_confirmation = false;
        }
        Message::ConfirmCleanWaste => {
            app.show_waste_confirmation = false;
            let paths: Vec<String> = app
                .latest_diagnostic
                .as_ref()
                .map(|d| {
                    d.waste_candidates
                        .iter()
                        .filter(|w| w.selected)
                        .map(|w| w.full_remote_path.clone())
                        .collect()
                })
                .unwrap_or_default();

            if paths.is_empty() {
                return Task::none();
            }

            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };
            app.log_output.push_str(&format!(
                "\n🧹 Iniciando eliminación segura de {} residuo(s) seleccionado(s) y confirmado(s)...\n",
                paths.len()
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let sess = net::create_session(&config)?;
                        let sftp = sess.sftp().map_err(|e| format!("Error abriendo SFTP: {e}"))?;
                        diagnostic::clean_confirmed_waste(&sftp, &paths)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
                },
                |res| cosmic::Action::App(Message::CleanWasteFinished(res)),
            );
        }
        Message::CleanWasteFinished(res) => {
            match res {
                Ok(count) => {
                    app.log_output.push_str(&format!("✅ Se eliminaron de forma segura {count} residuo(s).\n"));
                    app.latest_diagnostic = None;
                    app.show_waste_confirmation = false;
                    app.reload_games_for_selected_system();
                }
                Err(e) => {
                    app.log_output.push_str(&format!("⚠️ Resultado de limpieza: {e}\n"));
                    app.latest_diagnostic = None;
                    app.show_waste_confirmation = false;
                    app.reload_games_for_selected_system();
                }
            }
        }

        // Sincronización Wi-Fi
        Message::TestWifiConnection => {
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };
            app.log_output.push_str(&format!("\n📶 Probando conexión Wi-Fi a {}:{}...\n", config.host, config.port));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || net::test_connection(&config))
                        .await
                        .unwrap_or_else(|e| Err(format!("Fallo en hilo: {e}")))
                },
                |res| cosmic::Action::App(Message::WifiConnectionTested(res)),
            );
        }
        Message::WifiConnectionTested(result) => {
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
        }
        Message::StartScanAndReconcile => {
            if app.is_syncing {
                return Task::none();
            }
            app.is_syncing = true;
            let db_path = app.db_path.clone();
            let local_base = app.local_base_dir.clone();
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };

            app.log_output.push_str(&format!(
                "\n🔍 Iniciando escaneo e indexación con hashes BLAKE3...\n   Local: {}\n   Remoto: {}@{}:{}\n",
                local_base.display(), config.user, config.host, config.port
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        sync::scan_and_reconcile(&db_path, &local_base, &config)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
                },
                |res| cosmic::Action::App(Message::ScanAndReconcileFinished(res)),
            );
        }
        Message::ScanAndReconcileFinished(result) => {
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
        }
        Message::StartSyncSaves => {
            if app.is_syncing {
                return Task::none();
            }
            app.is_syncing = true;
            let db_path = app.db_path.clone();
            let local_base = app.local_base_dir.clone();
            let config = net::SshConfig {
                host: app.host_ip.clone(),
                port: app.ssh_port,
                user: app.ssh_user.clone(),
                password: app.ssh_password.clone(),
            };

            app.log_output.push_str("\n💾 Sincronizando partidas guardadas (Saves) bidireccionalmente...\n");

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        sync::sync_saves_bidirectional(&db_path, &local_base, &config)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
                },
                |res| cosmic::Action::App(Message::SyncSavesFinished(res)),
            );
        }
        Message::SyncSavesFinished(result) => {
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
        }

        Message::FilterCategory(cat) => {
            app.selected_category = cat;
            app.reload_sync_db();
        }
        Message::PickLocalFolder => {
            return Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new().pick_folder()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                |res| cosmic::Action::App(Message::LocalFolderSelected(res)),
            );
        }
        Message::LocalFolderSelected(Some(path)) => {
            app.local_base_dir_str = path.display().to_string();
            app.local_base_dir = path;
            if let Ok(conn) = db::init_db(&app.db_path) {
                let _ = db::save_setting(&conn, "local_base_dir", &app.local_base_dir_str);
            }
            app.reload_sync_db();
            app.rescan_local_systems();
            app.reload_games_for_selected_system();
        }
        Message::LocalFolderSelected(None) => {}

        // Conversor CHD
        Message::PickSingleCueFile => {
            return Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new()
                            .add_filter("CUE Sheet (*.cue)", &["cue", "CUE"])
                            .pick_file()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                |res| cosmic::Action::App(Message::SingleCueSelected(res)),
            );
        }
        Message::SingleCueSelected(Some(path)) => {
            app.single_cue_path = path.display().to_string();
            let mut out_path = path.clone();
            out_path.set_extension("chd");
            app.single_output_path = out_path.display().to_string();
        }
        Message::SingleCueSelected(None) => {}
        Message::SingleCuePathChanged(val) => {
            app.single_cue_path = val.clone();
            if val.to_lowercase().ends_with(".cue") {
                let mut path = PathBuf::from(val);
                path.set_extension("chd");
                app.single_output_path = path.display().to_string();
            }
        }
        Message::SingleOutputPathChanged(val) => {
            app.single_output_path = val;
        }
        Message::PickBatchFolder => {
            return Task::perform(
                async {
                    tokio::task::spawn_blocking(|| {
                        rfd::FileDialog::new().pick_folder()
                    })
                    .await
                    .ok()
                    .flatten()
                },
                |res| cosmic::Action::App(Message::BatchFolderSelected(res)),
            );
        }
        Message::BatchFolderSelected(Some(path)) => {
            app.batch_folder_path = path.display().to_string();
            app.found_cues = chd::find_cue_files(&path);
            app.log_output.push_str(&format!(
                "\n📁 Carpeta seleccionada: {}\nSe encontraron {} archivos .cue.",
                path.display(),
                app.found_cues.len()
            ));
        }
        Message::BatchFolderSelected(None) => {}
        Message::BatchFolderPathChanged(val) => {
            app.batch_folder_path = val;
        }
        Message::ScanFolderCues => {
            let path = PathBuf::from(&app.batch_folder_path);
            if path.is_dir() {
                app.found_cues = chd::find_cue_files(&path);
                app.log_output.push_str(&format!(
                    "\n🔍 Escaneo completado en {}: {} archivos .cue encontrados.",
                    path.display(),
                    app.found_cues.len()
                ));
            } else {
                app.log_output.push_str(&format!(
                    "\n⚠️ La ruta no es un directorio válido: {}",
                    app.batch_folder_path
                ));
            }
        }
        Message::ForceOverwriteToggled(val) => {
            app.force_overwrite = val;
        }
        Message::DeleteSourceToggled(val) => {
            app.delete_source_after = val;
        }
        Message::StartSingleConversion => {
            if app.is_converting || app.single_cue_path.trim().is_empty() {
                return Task::none();
            }
            let cue = PathBuf::from(&app.single_cue_path);
            let out = PathBuf::from(&app.single_output_path);
            let force = app.force_overwrite;
            let delete_src = app.delete_source_after;

            app.is_converting = true;
            app.log_output.push_str(&format!(
                "\n▶ Iniciando conversión de:\n   Origen: {}\n   Destino: {}\n",
                cue.display(),
                out.display()
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        chd::convert_cue_to_chd(&cue, &out, force, delete_src)
                    })
                    .await
                    .unwrap_or_else(|e| Err(format!("Fallo en hilo de ejecución: {e}")))
                },
                |res| cosmic::Action::App(Message::ConversionFinished(res)),
            );
        }
        Message::StartBatchConversion => {
            if app.is_converting || app.found_cues.is_empty() {
                return Task::none();
            }
            let cues = app.found_cues.clone();
            let force = app.force_overwrite;
            let delete_src = app.delete_source_after;

            app.is_converting = true;
            app.log_output.push_str(&format!(
                "\n▶ Iniciando conversión por lotes de {} archivos .cue...\n",
                cues.len()
            ));

            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let mut results = Vec::new();
                        for cue in cues {
                            let mut out = cue.clone();
                            out.set_extension("chd");
                            let name = cue.file_name().unwrap_or_default().to_string_lossy().to_string();
                            let res = chd::convert_cue_to_chd(&cue, &out, force, delete_src);
                            results.push((name, res));
                        }
                        results
                    })
                    .await
                    .unwrap_or_default()
                },
                |res| cosmic::Action::App(Message::BatchConversionFinished(res)),
            );
        }
        Message::ConversionFinished(result) => {
            app.is_converting = false;
            match result {
                Ok(stdout) => {
                    app.log_output.push_str(&format!(
                        "✅ Conversión completada con éxito.\n{}\n",
                        stdout.trim()
                    ));
                }
                Err(err) => {
                    app.log_output.push_str(&format!(
                        "❌ Error en la conversión:\n{}\n",
                        err.trim()
                    ));
                }
            }
        }
        Message::BatchConversionFinished(results) => {
            app.is_converting = false;
            let mut success_count = 0;
            let mut fail_count = 0;

            for (name, res) in results {
                match res {
                    Ok(_) => {
                        success_count += 1;
                        app.log_output.push_str(&format!("  [OK] {name}\n"));
                    }
                    Err(e) => {
                        fail_count += 1;
                        app.log_output.push_str(&format!("  [ERROR] {name}: {}\n", e.trim()));
                    }
                }
            }

            app.log_output.push_str(&format!(
                "\n🏁 Lote finalizado: {} convertidos con éxito, {} errores.\n",
                success_count, fail_count
            ));
        }
        Message::ClearLogs => {
            app.log_output.clear();
        }
    }
    Task::none()
}
