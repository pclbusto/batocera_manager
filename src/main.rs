mod chd;
mod db;
mod diagnostic;
mod hash;
mod net;
mod sync;
mod systems;

use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::alignment::Horizontal;
use cosmic::iced::{Length, Size};
use cosmic::prelude::*;
use cosmic::widget::about::About;
use cosmic::widget::{self, nav_bar, settings};
use std::path::PathBuf;

const APP_ID: &str = "com.github.batocera_manager";
const APP_TITLE: &str = "batocera_manager 0.0.1";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let settings = Settings::default()
        .size(Size::new(1150.0, 780.0));

    cosmic::app::run::<BatoceraManagerApp>(settings, ())?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Sync,
    Roms,
    ChdConverter,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Dashboard => "Panel Principal",
            Page::Sync => "Sincronización por Sistemas",
            Page::Roms => "Explorador y Copiado de Juegos",
            Page::ChdConverter => "Conversor CHD",
            Page::Settings => "Configuración",
        }
    }

    fn icon_name(self) -> &'static str {
        match self {
            Page::Dashboard => "speedometer-symbolic",
            Page::Sync => "emblem-synchronizing-symbolic",
            Page::Roms => "applications-games-symbolic",
            Page::ChdConverter => "media-optical-symbolic",
            Page::Settings => "preferences-system-symbolic",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Message {
    NavSelect(nav_bar::Id),
    ToggleAbout,
    OpenUrl(String),
    ToggleConnection,
    HostChanged(String),
    UserChanged(String),
    PasswordChanged(String),
    PortChanged(String),
    LocalFolderChanged(String),
    AutoSyncToggled(bool),
    SearchRoms(String),
    RefreshStatus,

    // Sincronización Wi-Fi & Base de Datos
    TestWifiConnection,
    WifiConnectionTested(Result<String, String>),
    StartScanAndReconcile,
    ScanAndReconcileFinished(Result<db::SyncSummary, String>),
    StartSyncSaves,
    SyncSavesFinished(Result<String, String>),
    StartSyncRoms,
    SyncRomsFinished(Result<String, String>),
    FilterCategory(String),
    PickLocalFolder,
    LocalFolderSelected(Option<PathBuf>),

    // Sincronización e Inventario por Sistemas
    ToggleSystem(usize, bool),
    SystemFolderInputChanged(usize, String),
    PickSystemFolder(usize),
    SystemFolderSelected(usize, Option<PathBuf>),
    SyncSingleSystem(usize),
    SingleSystemSyncFinished(usize, Result<String, String>),
    SyncAllEnabledSystems,
    AllSystemsSyncFinished(Vec<(String, Result<String, String>)>),
    RescanSystems,

    // Explorador y Copiado Selectivo de Juegos
    SelectSystemForGames(usize),
    ToggleGameSelection(usize, bool),
    SelectAllMissingGames,
    DeselectAllGames,
    CopySelectedGamesToConsole,
    ConfirmCopySelectedGames,
    CancelCopyConfirmation,
    CopySelectedGamesFinished(Result<String, String>),
    RefreshGamesList,

    // Diagnóstico y Limpieza Segura de Residuos
    RunDiagnostic,
    DiagnosticFinished(Result<diagnostic::DiagnosticReport, String>),
    CleanWasteFiles(Vec<String>),
    CleanWasteFinished(Result<usize, String>),

    // Conversor CHD
    PickSingleCueFile,
    SingleCueSelected(Option<PathBuf>),
    SingleCuePathChanged(String),
    SingleOutputPathChanged(String),
    PickBatchFolder,
    BatchFolderSelected(Option<PathBuf>),
    BatchFolderPathChanged(String),
    ScanFolderCues,
    ForceOverwriteToggled(bool),
    DeleteSourceToggled(bool),
    StartSingleConversion,
    StartBatchConversion,
    ConversionFinished(Result<String, String>),
    BatchConversionFinished(Vec<(String, Result<String, String>)>),
    ClearLogs,
}

pub struct BatoceraManagerApp {
    core: Core,
    nav_model: nav_bar::Model,
    active_page: Page,
    about: About,
    show_about: bool,

    // Parámetros de red Wi-Fi y Batocera
    is_connected: bool,
    host_ip: String,
    ssh_port: u16,
    ssh_port_str: String,
    ssh_user: String,
    ssh_password: String,
    local_base_dir: PathBuf,
    local_base_dir_str: String,
    auto_sync: bool,
    wifi_test_result: Option<Result<String, String>>,

    // Base de datos y Sincronización
    db_path: PathBuf,
    db_path_str: String,
    sync_summary: db::SyncSummary,
    sync_records: Vec<db::SyncFileRecord>,
    selected_category: String,
    is_syncing: bool,

    // Sistemas / Consolas
    systems: Vec<systems::SystemConfig>,

    // Explorador de Juegos
    selected_system_index: usize,
    games_list: Vec<systems::GameItem>,
    is_copying_games: bool,
    show_copy_confirmation: bool,
    rom_search_query: String,

    // Diagnóstico
    is_diagnosing: bool,
    latest_diagnostic: Option<diagnostic::DiagnosticReport>,

    // Conversor CHD
    chdman_version: Option<String>,
    single_cue_path: String,
    single_output_path: String,
    batch_folder_path: String,
    found_cues: Vec<PathBuf>,
    force_overwrite: bool,
    delete_source_after: bool,
    is_converting: bool,
    log_output: String,
}

impl cosmic::Application for BatoceraManagerApp {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut nav_model = nav_bar::Model::default();

        let pages = [
            Page::Dashboard,
            Page::Sync,
            Page::Roms,
            Page::ChdConverter,
            Page::Settings,
        ];

        for page in pages {
            nav_model
                .insert()
                .text(page.title())
                .icon(widget::icon::from_name(page.icon_name()))
                .data(page);
        }
        nav_model.activate_position(0);

        let chdman_ver = chd::detect_chdman();
        let db_path = db::default_db_path();
        let local_base_dir = sync::default_local_base_dir();
        let roms_root = local_base_dir.join("roms");

        // Inicializar DB local, Configuración guardada y Sistemas
        let (sync_summary, sync_records, systems, saved_host, saved_port, saved_user, saved_pass, custom_folder) = if let Ok(conn) = db::init_db(&db_path) {
            let sum = db::get_sync_summary(&conn).unwrap_or_default();
            let recs = db::get_all_records(&conn, None).unwrap_or_default();

            let host = db::load_setting(&conn, "host_ip").unwrap_or_else(|| "batocera.local".to_string());
            let port = db::load_setting(&conn, "ssh_port").unwrap_or_else(|| "22".to_string());
            let user = db::load_setting(&conn, "ssh_user").unwrap_or_else(|| "root".to_string());
            let pass = db::load_setting(&conn, "ssh_password").unwrap_or_else(|| "linux".to_string());
            let folder = db::load_setting(&conn, "local_base_dir");

            let current_roms_root = folder
                .as_ref()
                .map(|f| PathBuf::from(f).join("roms"))
                .unwrap_or_else(|| roms_root.clone());

            let sys_list = if let Ok(saved) = db::load_system_settings(&conn) {
                if saved.is_empty() {
                    let defaults = systems::default_systems_list(&current_roms_root);
                    for s in &defaults {
                        let _ = db::save_system_setting(&conn, &s.id, &s.name, &s.local_folder_str, s.enabled);
                    }
                    defaults
                } else {
                    saved
                        .into_iter()
                        .map(|(id, name, folder_str, enabled)| {
                            let folder = PathBuf::from(&folder_str);
                            let count = systems::count_files_in_dir(&folder);
                            systems::SystemConfig {
                                id: id.clone(),
                                name,
                                local_folder: folder,
                                local_folder_str: folder_str,
                                remote_folder: format!("roms/{id}"),
                                enabled,
                                local_rom_count: count,
                                remote_rom_count: 0,
                                status_msg: "Sin escanear".to_string(),
                            }
                        })
                        .collect()
                }
            } else {
                systems::default_systems_list(&current_roms_root)
            };

            (sum, recs, sys_list, host, port, user, pass, folder)
        } else {
            (
                db::SyncSummary::default(),
                Vec::new(),
                systems::default_systems_list(&roms_root),
                "batocera.local".to_string(),
                "22".to_string(),
                "root".to_string(),
                "linux".to_string(),
                None,
            )
        };

        let effective_base_dir = custom_folder
            .map(PathBuf::from)
            .unwrap_or(local_base_dir);

        let selected_system_index = 0;
        let games_list = if let Some(sys) = systems.first() {
            systems::scan_games_for_system(&sys.local_folder, &[])
        } else {
            Vec::new()
        };

        let about = About::default()
            .name("batocera_manager")
            .version("0.0.1")
            .author("Equipo Batocera Manager")
            .license("GPL-3.0-only")
            .license_url("https://www.gnu.org/licenses/gpl-3.0.html")
            .comments(
                "Gestor integral para consolas Anbernic con Batocera / Knulli.\n\
                Incluye sincronización por sistemas independientes con SQLite, copiado selectivo de ROMs y conversor CHD.",
            )
            .links([
                ("Sitio Web de Batocera", "https://batocera.org"),
                ("Documentación chdman", "https://docs.mamedev.org"),
                ("Documentación libcosmic", "https://github.com/pop-os/libcosmic"),
            ]);

        let db_path_str = db_path.display().to_string();
        let local_base_dir_str = effective_base_dir.display().to_string();
        let port_u16 = saved_port.parse::<u16>().unwrap_or(22);

        let initial_log = if let Some(ref ver) = chdman_ver {
            format!("ℹ️ Herramienta detectada: {ver}\nBase de datos SQLite: {}\nHost Anbernic guardado: {saved_host}", db_path_str)
        } else {
            format!("ℹ️ Base de datos SQLite: {}\nHost Anbernic guardado: {saved_host}", db_path_str)
        };

        let mut app = BatoceraManagerApp {
            core,
            nav_model,
            active_page: Page::Dashboard,
            about,
            show_about: false,

            is_connected: false,
            host_ip: saved_host,
            ssh_port: port_u16,
            ssh_port_str: saved_port,
            ssh_user: saved_user,
            ssh_password: saved_pass,
            local_base_dir: effective_base_dir,
            local_base_dir_str,
            auto_sync: true,
            wifi_test_result: None,

            db_path,
            db_path_str,
            sync_summary,
            sync_records,
            selected_category: "all".to_string(),
            is_syncing: false,

            systems,
            selected_system_index,
            games_list,
            is_copying_games: false,
            show_copy_confirmation: false,

            rom_search_query: String::new(),

            is_diagnosing: false,
            latest_diagnostic: None,

            chdman_version: chdman_ver,
            single_cue_path: String::new(),
            single_output_path: String::new(),
            batch_folder_path: String::new(),
            found_cues: Vec::new(),
            force_overwrite: true,
            delete_source_after: false,
            is_converting: false,
            log_output: initial_log,
        };

        app.set_header_title(APP_TITLE.to_string());

        let title_task = if let Some(id) = app.core.main_window_id() {
            app.set_window_title(APP_TITLE.to_string(), id)
        } else {
            Task::none()
        };

        (app, title_task)
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<Self::Message> {
        self.nav_model.activate(id);
        if let Some(&page) = self.nav_model.data::<Page>(id) {
            self.active_page = page;
            self.set_header_title(format!("{} — {}", page.title(), APP_TITLE));
            if page == Page::Roms {
                self.reload_games_for_selected_system();
            }
        }
        Task::none()
    }

    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        let (status_label, status_suggested) = if self.is_connected {
            ("Wi-Fi Conectado", true)
        } else {
            ("Probar Wi-Fi", false)
        };

        let conn_button = if status_suggested {
            widget::button::suggested(status_label).on_press(Message::TestWifiConnection)
        } else {
            widget::button::standard(status_label).on_press(Message::TestWifiConnection)
        };

        let about_button = widget::button::icon(widget::icon::from_name("help-about-symbolic"))
            .on_press(Message::ToggleAbout);

        vec![
            conn_button.into(),
            about_button.into(),
        ]
    }

    fn context_drawer(&self) -> Option<ContextDrawer<'_, Self::Message>> {
        self.show_about.then(|| {
            context_drawer::about(
                &self.about,
                |url| Message::OpenUrl(url.to_owned()),
                Message::ToggleAbout,
            )
        })
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::NavSelect(id) => {
                return self.on_nav_select(id);
            }
            Message::ToggleAbout => {
                self.show_about = !self.show_about;
                self.set_show_context(self.show_about);
            }
            Message::OpenUrl(url) => {
                let _ = open::that_detached(url);
            }
            Message::ToggleConnection => {
                self.is_connected = !self.is_connected;
            }
            Message::HostChanged(val) => {
                if let Ok(conn) = db::init_db(&self.db_path) {
                    let _ = db::save_setting(&conn, "host_ip", &val);
                }
                self.host_ip = val;
            }
            Message::PortChanged(val) => {
                self.ssh_port_str = val.clone();
                if let Ok(p) = val.parse::<u16>() {
                    self.ssh_port = p;
                }
                if let Ok(conn) = db::init_db(&self.db_path) {
                    let _ = db::save_setting(&conn, "ssh_port", &val);
                }
            }
            Message::UserChanged(val) => {
                if let Ok(conn) = db::init_db(&self.db_path) {
                    let _ = db::save_setting(&conn, "ssh_user", &val);
                }
                self.ssh_user = val;
            }
            Message::PasswordChanged(val) => {
                if let Ok(conn) = db::init_db(&self.db_path) {
                    let _ = db::save_setting(&conn, "ssh_password", &val);
                }
                self.ssh_password = val;
            }
            Message::LocalFolderChanged(val) => {
                if let Ok(conn) = db::init_db(&self.db_path) {
                    let _ = db::save_setting(&conn, "local_base_dir", &val);
                }
                self.local_base_dir_str = val.clone();
                self.local_base_dir = PathBuf::from(val);
            }
            Message::AutoSyncToggled(val) => {
                self.auto_sync = val;
            }
            Message::SearchRoms(val) => {
                self.rom_search_query = val;
            }
            Message::RefreshStatus => {
                self.chdman_version = chd::detect_chdman();
                self.reload_sync_db();
                self.rescan_local_systems();
                self.reload_games_for_selected_system();
            }

            // Sincronización por Sistemas
            Message::ToggleSystem(idx, enabled) => {
                if let Some(sys) = self.systems.get_mut(idx) {
                    sys.enabled = enabled;
                    if let Ok(conn) = db::init_db(&self.db_path) {
                        let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
                    }
                }
            }
            Message::SystemFolderInputChanged(idx, folder_str) => {
                if let Some(sys) = self.systems.get_mut(idx) {
                    sys.local_folder = PathBuf::from(&folder_str);
                    sys.local_folder_str = folder_str;
                    sys.local_rom_count = systems::count_files_in_dir(&sys.local_folder);
                    if let Ok(conn) = db::init_db(&self.db_path) {
                        let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
                    }
                }
                if idx == self.selected_system_index {
                    self.reload_games_for_selected_system();
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
                if let Some(sys) = self.systems.get_mut(idx) {
                    sys.local_folder_str = path.display().to_string();
                    sys.local_folder = path;
                    sys.local_rom_count = systems::count_files_in_dir(&sys.local_folder);
                    if let Ok(conn) = db::init_db(&self.db_path) {
                        let _ = db::save_system_setting(&conn, &sys.id, &sys.name, &sys.local_folder_str, sys.enabled);
                    }
                }
                if idx == self.selected_system_index {
                    self.reload_games_for_selected_system();
                }
            }
            Message::SystemFolderSelected(_idx, None) => {}
            Message::RescanSystems => {
                self.rescan_local_systems();
                self.reload_games_for_selected_system();
                self.log_output.push_str("\n🔄 Carpetas de sistemas re-escaneadas.\n");
            }
            Message::SyncSingleSystem(idx) => {
                if self.is_syncing || idx >= self.systems.len() {
                    return Task::none();
                }
                self.is_syncing = true;
                let sys = self.systems[idx].clone();
                let db_path = self.db_path.clone();
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };

                self.log_output.push_str(&format!(
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
                self.is_syncing = false;
                match result {
                    Ok(msg) => {
                        if let Some(sys) = self.systems.get_mut(idx) {
                            sys.status_msg = "Sincronizado".to_string();
                        }
                        self.reload_sync_db();
                        self.log_output.push_str(&format!("✅ {msg}\n"));
                    }
                    Err(e) => {
                        if let Some(sys) = self.systems.get_mut(idx) {
                            sys.status_msg = format!("Error: {e}");
                        }
                        self.log_output.push_str(&format!("❌ {e}\n"));
                    }
                }
            }
            Message::SyncAllEnabledSystems => {
                if self.is_syncing {
                    return Task::none();
                }
                self.is_syncing = true;
                let active_systems: Vec<systems::SystemConfig> = self
                    .systems
                    .iter()
                    .filter(|s| s.enabled)
                    .cloned()
                    .collect();

                let db_path = self.db_path.clone();
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };

                self.log_output.push_str(&format!(
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
                self.is_syncing = false;
                for (name, res) in results {
                    match res {
                        Ok(msg) => {
                            self.log_output.push_str(&format!("  [OK] {msg}\n"));
                        }
                        Err(err) => {
                            self.log_output.push_str(&format!("  [ERROR] {name}: {err}\n"));
                        }
                    }
                }
                self.reload_sync_db();
                self.log_output.push_str("🏁 Sincronización de sistemas finalizada.\n");
            }

            // Explorador de Juegos (Copiado Selectivo)
            Message::SelectSystemForGames(idx) => {
                self.selected_system_index = idx;
                self.reload_games_for_selected_system();
            }
            Message::ToggleGameSelection(idx, val) => {
                if let Some(game) = self.games_list.get_mut(idx) {
                    game.selected = val;
                }
            }
            Message::SelectAllMissingGames => {
                for game in &mut self.games_list {
                    game.selected = !game.exists_on_console;
                }
            }
            Message::DeselectAllGames => {
                for game in &mut self.games_list {
                    game.selected = false;
                }
            }
            Message::RefreshGamesList => {
                self.reload_games_for_selected_system();
            }
            Message::CopySelectedGamesToConsole => {
                let selected_count = self.games_list.iter().filter(|g| g.selected).count();
                if selected_count > 0 {
                    self.show_copy_confirmation = true;
                }
            }
            Message::CancelCopyConfirmation => {
                self.show_copy_confirmation = false;
            }
            Message::ConfirmCopySelectedGames => {
                self.show_copy_confirmation = false;
                if self.is_copying_games || self.selected_system_index >= self.systems.len() {
                    return Task::none();
                }
                let selected_games: Vec<systems::GameItem> = self
                    .games_list
                    .iter()
                    .filter(|g| g.selected)
                    .cloned()
                    .collect();

                if selected_games.is_empty() {
                    return Task::none();
                }

                self.is_copying_games = true;
                let sys = self.systems[self.selected_system_index].clone();
                let db_path = self.db_path.clone();
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };

                let total_size: u64 = selected_games.iter().map(|g| g.size_bytes).sum();
                self.log_output.push_str(&format!(
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
                self.is_copying_games = false;
                match result {
                    Ok(msg) => {
                        self.log_output.push_str(&format!("✅ {msg}\n"));
                        self.reload_games_for_selected_system();
                        self.reload_sync_db();
                    }
                    Err(err) => {
                        self.log_output.push_str(&format!("❌ {err}\n"));
                    }
                }
            }

            // Diagnóstico de Almacenamiento y Limpieza Segura
            Message::RunDiagnostic => {
                if self.is_diagnosing {
                    return Task::none();
                }
                self.is_diagnosing = true;
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };
                let sys_list = self.systems.clone();
                self.log_output.push_str("\n🔍 Iniciando diagnóstico no destructivo de almacenamiento en la consola...\n");

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
                self.is_diagnosing = false;
                match res {
                    Ok(report) => {
                        self.log_output.push_str(&report.report_text);
                        self.latest_diagnostic = Some(report);
                    }
                    Err(e) => {
                        self.log_output.push_str(&format!("❌ Error durante el diagnóstico: {e}\n"));
                    }
                }
            }
            Message::CleanWasteFiles(paths) => {
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };
                self.log_output.push_str(&format!(
                    "\n🧹 Iniciando eliminación segura de {} residuo(s) confirmado(s)...\n",
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
                        self.log_output.push_str(&format!("✅ Se eliminaron de forma segura {count} residuo(s).\n"));
                        self.latest_diagnostic = None;
                        self.reload_games_for_selected_system();
                    }
                    Err(e) => {
                        self.log_output.push_str(&format!("⚠️ Resultado de limpieza: {e}\n"));
                    }
                }
            }

            // Sincronización Wi-Fi
            Message::TestWifiConnection => {
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };
                self.log_output.push_str(&format!("\n📶 Probando conexión Wi-Fi a {}:{}...\n", config.host, config.port));

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
                        self.is_connected = true;
                        self.log_output.push_str(&format!("✅ Conexión establecida: {info}\n"));
                    }
                    Err(err) => {
                        self.is_connected = false;
                        self.log_output.push_str(&format!("❌ Fallo de conexión: {err}\n"));
                    }
                }
                self.wifi_test_result = Some(result);
            }
            Message::StartScanAndReconcile => {
                if self.is_syncing {
                    return Task::none();
                }
                self.is_syncing = true;
                let db_path = self.db_path.clone();
                let local_base = self.local_base_dir.clone();
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };

                self.log_output.push_str(&format!(
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
                self.is_syncing = false;
                match result {
                    Ok(summary) => {
                        self.sync_summary = summary.clone();
                        self.reload_sync_db();
                        self.log_output.push_str(&format!(
                            "✅ Escaneo completado. Total: {}, Sincronizados: {}, Nuevos en Anbernic: {}, Nuevos en PC: {}\n",
                            summary.total, summary.in_sync, summary.remote_newer + summary.only_remote, summary.local_newer + summary.only_local
                        ));
                    }
                    Err(e) => {
                        self.log_output.push_str(&format!("❌ Error durante el escaneo: {e}\n"));
                    }
                }
            }
            Message::StartSyncSaves => {
                if self.is_syncing {
                    return Task::none();
                }
                self.is_syncing = true;
                let db_path = self.db_path.clone();
                let local_base = self.local_base_dir.clone();
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };

                self.log_output.push_str("\n💾 Sincronizando partidas guardadas (Saves) bidireccionalmente...\n");

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
                self.is_syncing = false;
                match result {
                    Ok(msg) => {
                        self.reload_sync_db();
                        self.log_output.push_str(&format!("✅ {msg}\n"));
                    }
                    Err(e) => {
                        self.log_output.push_str(&format!("❌ Error sincronizando partidas: {e}\n"));
                    }
                }
            }
            Message::StartSyncRoms => {
                if self.is_syncing {
                    return Task::none();
                }
                self.is_syncing = true;
                let db_path = self.db_path.clone();
                let local_base = self.local_base_dir.clone();
                let config = net::SshConfig {
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };

                self.log_output.push_str("\n🎮 Transfiriendo nuevas ROMs hacia la Anbernic...\n");

                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            sync::sync_roms_to_anbernic(&db_path, &local_base, &config)
                        })
                        .await
                        .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
                    },
                    |res| cosmic::Action::App(Message::SyncRomsFinished(res)),
                );
            }
            Message::SyncRomsFinished(result) => {
                self.is_syncing = false;
                match result {
                    Ok(msg) => {
                        self.reload_sync_db();
                        self.log_output.push_str(&format!("✅ {msg}\n"));
                    }
                    Err(e) => {
                        self.log_output.push_str(&format!("❌ Error sincronizando ROMs: {e}\n"));
                    }
                }
            }
            Message::FilterCategory(cat) => {
                self.selected_category = cat;
                self.reload_sync_db();
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
                self.local_base_dir_str = path.display().to_string();
                self.local_base_dir = path;
                if let Ok(conn) = db::init_db(&self.db_path) {
                    let _ = db::save_setting(&conn, "local_base_dir", &self.local_base_dir_str);
                }
                self.reload_sync_db();
                self.rescan_local_systems();
                self.reload_games_for_selected_system();
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
                self.single_cue_path = path.display().to_string();
                let mut out_path = path.clone();
                out_path.set_extension("chd");
                self.single_output_path = out_path.display().to_string();
            }
            Message::SingleCueSelected(None) => {}
            Message::SingleCuePathChanged(val) => {
                self.single_cue_path = val.clone();
                if val.to_lowercase().ends_with(".cue") {
                    let mut path = PathBuf::from(val);
                    path.set_extension("chd");
                    self.single_output_path = path.display().to_string();
                }
            }
            Message::SingleOutputPathChanged(val) => {
                self.single_output_path = val;
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
                self.batch_folder_path = path.display().to_string();
                self.found_cues = chd::find_cue_files(&path);
                self.log_output.push_str(&format!(
                    "\n📁 Carpeta seleccionada: {}\nSe encontraron {} archivos .cue.",
                    path.display(),
                    self.found_cues.len()
                ));
            }
            Message::BatchFolderSelected(None) => {}
            Message::BatchFolderPathChanged(val) => {
                self.batch_folder_path = val;
            }
            Message::ScanFolderCues => {
                let path = PathBuf::from(&self.batch_folder_path);
                if path.is_dir() {
                    self.found_cues = chd::find_cue_files(&path);
                    self.log_output.push_str(&format!(
                        "\n🔍 Escaneo completado en {}: {} archivos .cue encontrados.",
                        path.display(),
                        self.found_cues.len()
                    ));
                } else {
                    self.log_output.push_str(&format!(
                        "\n⚠️ La ruta no es un directorio válido: {}",
                        self.batch_folder_path
                    ));
                }
            }
            Message::ForceOverwriteToggled(val) => {
                self.force_overwrite = val;
            }
            Message::DeleteSourceToggled(val) => {
                self.delete_source_after = val;
            }
            Message::StartSingleConversion => {
                if self.is_converting || self.single_cue_path.trim().is_empty() {
                    return Task::none();
                }
                let cue = PathBuf::from(&self.single_cue_path);
                let out = PathBuf::from(&self.single_output_path);
                let force = self.force_overwrite;
                let delete_src = self.delete_source_after;

                self.is_converting = true;
                self.log_output.push_str(&format!(
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
                if self.is_converting || self.found_cues.is_empty() {
                    return Task::none();
                }
                let cues = self.found_cues.clone();
                let force = self.force_overwrite;
                let delete_src = self.delete_source_after;

                self.is_converting = true;
                self.log_output.push_str(&format!(
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
                self.is_converting = false;
                match result {
                    Ok(stdout) => {
                        self.log_output.push_str(&format!(
                            "✅ Conversión completada con éxito.\n{}\n",
                            stdout.trim()
                        ));
                    }
                    Err(err) => {
                        self.log_output.push_str(&format!(
                            "❌ Error en la conversión:\n{}\n",
                            err.trim()
                        ));
                    }
                }
            }
            Message::BatchConversionFinished(results) => {
                self.is_converting = false;
                let mut success_count = 0;
                let mut fail_count = 0;

                for (name, res) in results {
                    match res {
                        Ok(_) => {
                            success_count += 1;
                            self.log_output.push_str(&format!("  [OK] {name}\n"));
                        }
                        Err(e) => {
                            fail_count += 1;
                            self.log_output.push_str(&format!("  [ERROR] {name}: {}\n", e.trim()));
                        }
                    }
                }

                self.log_output.push_str(&format!(
                    "\n🏁 Lote finalizado: {} convertidos con éxito, {} errores.\n",
                    success_count, fail_count
                ));
            }
            Message::ClearLogs => {
                self.log_output.clear();
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let page_element = match self.active_page {
            Page::Dashboard => self.view_dashboard(),
            Page::Sync => self.view_sync(),
            Page::Roms => self.view_roms(),
            Page::ChdConverter => self.view_chd_converter(),
            Page::Settings => self.view_settings(),
        };

        let scroll = widget::scrollable(page_element)
            .width(Length::Fill)
            .height(Length::Fill);

        Element::from(scroll)
    }
}

impl BatoceraManagerApp {
    fn reload_sync_db(&mut self) {
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

    fn rescan_local_systems(&mut self) {
        for sys in &mut self.systems {
            sys.local_rom_count = systems::count_files_in_dir(&sys.local_folder);
        }
    }

    fn reload_games_for_selected_system(&mut self) {
        if let Some(sys) = self.systems.get(self.selected_system_index) {
            let prefix = format!("{}/", sys.remote_folder);
            let remote_names: Vec<String> = self.sync_records
                .iter()
                .filter(|r| r.relative_path.starts_with(&prefix))
                .filter_map(|r| {
                    if r.remote_size.is_some() || r.status == db::SyncStatus::InSync || r.status == db::SyncStatus::OnlyRemote {
                        r.relative_path.strip_prefix(&prefix).map(|s| s.to_string())
                    } else {
                        None
                    }
                })
                .collect();
            self.games_list = systems::scan_games_for_system(&sys.local_folder, &remote_names);
        }
    }

    fn view_dashboard(&self) -> Element<'_, Message> {
        let app_badge = widget::text::title2(APP_TITLE);
        let app_subtitle = widget::text::body(
            "Administrador de sistemas por consola, copiado selectivo de juegos, sincronizador Wi-Fi y conversor CHD.",
        );

        let header_box = widget::column::with_capacity(2)
            .push(app_badge)
            .push(app_subtitle)
            .spacing(4);

        // Tarjeta de estado de conexión Wi-Fi
        let status_text = if self.is_connected {
            format!("🟢 Conectado por Wi-Fi a Batocera en {}:{}", self.host_ip, self.ssh_port)
        } else {
            format!("⚪ Sin comprobar / Desconectado ({}:{})", self.host_ip, self.ssh_port)
        };

        let connect_btn = widget::button::suggested("Probar enlace Wi-Fi")
            .on_press(Message::TestWifiConnection);

        let connection_card = settings::section()
            .title("Conexión con Anbernic")
            .add(
                settings::item("Estado Wi-Fi / SSH", widget::text::body(status_text)),
            )
            .add(
                settings::item("Diagnóstico de red", connect_btn),
            );

        // Tarjeta resumen de sistemas
        let enabled_count = self.systems.iter().filter(|s| s.enabled).count();
        let total_local_roms: usize = self.systems.iter().map(|s| s.local_rom_count).sum();

        let systems_overview_card = settings::section()
            .title("Resumen de Consolas y Sistemas")
            .add(
                settings::item(
                    "Sistemas activos para sincronizar",
                    widget::text::body(format!("{enabled_count} de {} sistemas habilitados", self.systems.len())),
                ),
            )
            .add(
                settings::item(
                    "Juegos locales detectados en PC",
                    widget::text::body(format!("{total_local_roms} juegos distribuidos por carpetas")),
                ),
            );

        // Tarjeta chdman
        let chdman_label = if let Some(ref ver) = self.chdman_version {
            format!("🟢 {ver} instalado")
        } else {
            "🔴 No detectado (requiere paquete mame-tools)".to_string()
        };

        let tools_card = settings::section()
            .title("Herramientas del Sistema")
            .add(
                settings::item(
                    "Utilidad chdman",
                    widget::text::body(chdman_label),
                ),
            );

        // Acciones rápidas
        let diag_btn_label = if self.is_diagnosing {
            "🔍 Analizando SD..."
        } else {
            "🔍 Diagnóstico Tarjeta SD"
        };
        let diag_btn = widget::button::standard(diag_btn_label)
            .on_press(Message::RunDiagnostic);

        let quick_actions = widget::row::with_capacity(5)
            .push(
                widget::button::suggested("🔄 Reconciliar Inventarios")
                    .on_press(Message::SyncAllEnabledSystems),
            )
            .push(diag_btn)
            .push(
                widget::button::standard("💾 Sincronizar Partidas")
                    .on_press(Message::StartSyncSaves),
            )
            .push(
                widget::button::standard("ℹ️ Acerca de")
                    .on_press(Message::ToggleAbout),
            )
            .spacing(12);

        let content = widget::column::with_capacity(6)
            .push(header_box)
            .push(connection_card)
            .push(systems_overview_card)
            .push(tools_card)
            .push(quick_actions)
            .spacing(24)
            .padding(24)
            .max_width(940);

        widget::container(content)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
    }

    fn view_sync(&self) -> Element<'_, Message> {
        let title = widget::text::title2("Gestión e Inventario por Sistemas (Anbernic ⇄ PC)");
        let desc = widget::text::body(
            "Configura la carpeta local en tu PC para cada consola y mantén actualizado el inventario por Wi-Fi.\n\
            ¿Qué hace 'Actualizar Inventario'? Compara la consola contra tu PC y registra el estado en la base de datos local sin transferir ni borrar ningún archivo.\n\
            Para copiar juegos específicos a la consola, ve a la pestaña 'Explorador y Copiado de Juegos'.",
        );

        // 1. Barra de estado y acciones globales
        let enabled_count = self.systems.iter().filter(|s| s.enabled).count();
        let total_local_roms: usize = self.systems.iter().map(|s| s.local_rom_count).sum();

        let sync_all_btn = if self.is_syncing {
            widget::button::suggested("Reconciliando inventarios...").width(Length::Shrink)
        } else {
            widget::button::suggested(format!("🔄 Actualizar Inventario ({enabled_count} Sistemas)"))
                .on_press(Message::SyncAllEnabledSystems)
                .width(Length::Shrink)
        };

        let diag_btn = if self.is_diagnosing {
            widget::button::standard("Ejecutando diagnóstico...").width(Length::Shrink)
        } else {
            widget::button::standard("🔍 Diagnóstico de Residuos SD")
                .on_press(Message::RunDiagnostic)
                .width(Length::Shrink)
        };

        let rescan_btn = widget::button::standard("🔄 Re-escanear Carpetas")
            .on_press(Message::RescanSystems)
            .width(Length::Shrink);

        let sync_saves_btn = widget::button::standard("💾 Sincronizar Partidas")
            .on_press(Message::StartSyncSaves)
            .width(Length::Shrink);

        let overview_section = settings::section()
            .title("Control Global de Sincronización")
            .add(
                settings::item(
                    "Estado de Sistemas",
                    widget::text::body(format!("{enabled_count} activados de {} totales  |  {total_local_roms} juegos en PC", self.systems.len())),
                ),
            )
            .add(
                settings::item("Inventario por Wi-Fi (solo lectura)", sync_all_btn),
            )
            .add(
                settings::item("Inspección de residuos (0 bytes / .part)", diag_btn),
            )
            .add(
                settings::item("Comprobar cambios locales", rescan_btn),
            )
            .add(
                settings::item("Partidas guardadas (/userdata/saves)", sync_saves_btn),
            );

        // Sección de residuos si se detectaron
        let waste_section: Option<Element<'_, Message>> = if let Some(ref diag) = self.latest_diagnostic {
            if !diag.waste_candidates.is_empty() {
                let paths: Vec<String> = diag.waste_candidates.iter().map(|w| w.full_remote_path.clone()).collect();
                let clean_btn = widget::button::destructive(format!(
                    "Limpiar {} residuos detectados ({})",
                    diag.waste_candidates.len(),
                    systems::format_size(diag.total_waste_bytes)
                ))
                .on_press(Message::CleanWasteFiles(paths));

                Some(
                    settings::section()
                        .title("⚠️ Residuos Detectados en Tarjeta SD (Archivos 0B / .part)")
                        .add(settings::item(
                            format!(
                                "Se detectaron {} archivos de 0 bytes o temporales .part huérfanos. Ninguno ha sido borrado.",
                                diag.waste_candidates.len()
                            ),
                            clean_btn,
                        ))
                        .into()
                )
            } else {
                None
            }
        } else {
            None
        };

        // 2. Tarjetas de cada sistema / consola
        let mut systems_column = widget::column::with_capacity(self.systems.len() + 1)
            .spacing(16);

        systems_column = systems_column.push(widget::text::title3("Consolas y Sistemas Configurados"));

        for (idx, sys) in self.systems.iter().enumerate() {
            let toggle = widget::toggler(sys.enabled)
                .on_toggle(move |val| Message::ToggleSystem(idx, val));

            let folder_input = widget::text_input::text_input("Ruta local en PC...", &sys.local_folder_str)
                .on_input(move |val| Message::SystemFolderInputChanged(idx, val));

            let browse_btn = widget::button::standard("Examinar...")
                .on_press(Message::PickSystemFolder(idx));

            let folder_row = widget::row::with_capacity(2)
                .push(folder_input)
                .push(browse_btn)
                .spacing(8)
                .width(Length::Fill);

            let sync_btn = if self.is_syncing {
                widget::button::standard("Ocupado...").width(Length::Shrink)
            } else {
                widget::button::standard(format!("Actualizar {}", sys.id))
                    .on_press(Message::SyncSingleSystem(idx))
                    .width(Length::Shrink)
            };

            let rom_status_label = format!(
                "📁 {} juegos locales  |  Destino Anbernic: /userdata/{}  |  Estado: {}",
                sys.local_rom_count, sys.remote_folder, sys.status_msg
            );

            let sys_section = settings::section()
                .title(format!("🎮 {} ({})", sys.name, sys.id))
                .add(
                    settings::item("Habilitar para esta consola", toggle),
                )
                .add(
                    settings::item("Carpeta local en el PC", folder_row),
                )
                .add(
                    settings::item(rom_status_label, sync_btn),
                );

            systems_column = systems_column.push(sys_section);
        }

        // 3. Consola de registros
        let clear_log_btn = widget::button::standard("Limpiar registro")
            .on_press(Message::ClearLogs);

        let log_header = widget::row::with_capacity(3)
            .push(widget::text::title3("Registro de Sincronización"))
            .push(cosmic::iced::widget::space::horizontal().width(Length::Fill))
            .push(clear_log_btn)
            .align_y(cosmic::iced::alignment::Vertical::Center);

        let log_display = widget::container(
            widget::scrollable(
                widget::text(&self.log_output)
                    .size(13)
                    .width(Length::Fill),
            )
            .height(Length::Fixed(180.0)),
        )
        .padding(12)
        .width(Length::Fill);

        let mut content = widget::column::with_capacity(7)
            .push(title)
            .push(desc)
            .push(overview_section);

        if let Some(ws) = waste_section {
            content = content.push(ws);
        }

        content = content
            .push(systems_column)
            .push(log_header)
            .push(log_display)
            .spacing(20)
            .padding(24)
            .max_width(940);

        widget::container(content)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
    }

    fn view_roms(&self) -> Element<'_, Message> {
        let title = widget::text::title2("Explorador y Copiado de Juegos a la Consola");
        let desc = widget::text::body(
            "Selecciona tu consola y marca con las casillas los juegos específicos que deseas copiar directamente a la Anbernic por Wi-Fi.\n\
            No necesitas sincronizar toda la colección: aquí tienes control juego a juego.",
        );

        // 1. Selector de Consola mediante botones
        let mut system_buttons = widget::row::with_capacity(self.systems.len()).spacing(8);
        for (idx, sys) in self.systems.iter().enumerate() {
            let btn = if idx == self.selected_system_index {
                widget::button::suggested(sys.name.as_str())
            } else {
                widget::button::standard(sys.name.as_str()).on_press(Message::SelectSystemForGames(idx))
            };
            system_buttons = system_buttons.push(btn);
        }

        let system_scroll = widget::scrollable::horizontal(system_buttons);

        // 2. Información del sistema seleccionado
        let current_sys = &self.systems[self.selected_system_index];
        let sys_info_text = format!(
            "Carpeta en tu PC: {}  ➔  Destino Anbernic: /userdata/{}",
            current_sys.local_folder_str, current_sys.remote_folder
        );

        let browse_btn = widget::button::standard("Cambiar carpeta...")
            .on_press(Message::PickSystemFolder(self.selected_system_index));

        let sys_header_section = settings::section()
            .title(format!("Consola activa: {}", current_sys.name))
            .add(
                settings::item(sys_info_text, browse_btn),
            );

        // 3. Barra de herramientas para selección y copiado
        let selected_count = self.games_list.iter().filter(|g| g.selected).count();
        let total_selected_bytes: u64 = self.games_list.iter().filter(|g| g.selected).map(|g| g.size_bytes).sum();

        let copy_btn = if self.is_copying_games {
            widget::button::suggested("Copiando juegos por Wi-Fi...").width(Length::Shrink)
        } else if selected_count == 0 {
            widget::button::standard("Copiar a la Anbernic (0 seleccionados)").width(Length::Shrink)
        } else {
            widget::button::suggested(format!("🚀 Copiar a la Anbernic ({selected_count} seleccionados)"))
                .on_press(Message::CopySelectedGamesToConsole)
                .width(Length::Shrink)
        };

        let select_missing_btn = widget::button::standard("Seleccionar los que faltan")
            .on_press(Message::SelectAllMissingGames);

        let deselect_btn = widget::button::standard("Deseleccionar todos")
            .on_press(Message::DeselectAllGames);

        let refresh_btn = widget::button::standard("🔄 Refrescar")
            .on_press(Message::RefreshGamesList);

        let toolbar = widget::row::with_capacity(5)
            .push(copy_btn)
            .push(select_missing_btn)
            .push(deselect_btn)
            .push(refresh_btn)
            .spacing(10);

        // Sección de confirmación previa a la transferencia
        let confirmation_section: Option<Element<'_, Message>> = if self.show_copy_confirmation {
            let confirm_btn = widget::button::suggested("✓ Confirmar e Iniciar Copiado")
                .on_press(Message::ConfirmCopySelectedGames);
            let cancel_btn = widget::button::standard("✕ Cancelar")
                .on_press(Message::CancelCopyConfirmation);
            let btn_row = widget::row::with_capacity(2).push(confirm_btn).push(cancel_btn).spacing(8);

            Some(
                settings::section()
                    .title("⚠️ Confirmar Transferencia hacia la Consola")
                    .add(settings::item(
                        format!(
                            "Se transferirán {} juego(s) ({} total) hacia /userdata/{}. ¿Proceder?",
                            selected_count,
                            systems::format_size(total_selected_bytes),
                            current_sys.remote_folder
                        ),
                        btn_row,
                    ))
                    .into()
            )
        } else {
            None
        };

        let search_bar = widget::text_input::search_input("Buscar por título de juego...", &self.rom_search_query)
            .on_input(Message::SearchRoms);

        // 4. Lista de juegos
        let query_lower = self.rom_search_query.to_lowercase();
        let filtered_games: Vec<(usize, &systems::GameItem)> = self
            .games_list
            .iter()
            .enumerate()
            .filter(|(_, g)| {
                if query_lower.is_empty() {
                    true
                } else {
                    g.filename.to_lowercase().contains(&query_lower)
                }
            })
            .collect();

        let mut games_section = settings::section()
            .title(format!("Juegos encontrados ({} disponibles)", filtered_games.len()));

        if filtered_games.is_empty() {
            games_section = games_section.add(
                settings::item(
                    "No se encontraron juegos en esta carpeta",
                    widget::text::caption(
                        "Copia archivos de juegos en la carpeta local o haz clic en 'Cambiar carpeta' para apuntar a donde guardas tus ROMs.",
                    ),
                ),
            );
        } else {
            for (idx, game) in filtered_games {
                let status_label = if game.exists_on_console {
                    format!("{}  |  🟢 Ya en la Anbernic", game.size_formatted)
                } else {
                    format!("{}  |  📥 Listo para copiar a Anbernic", game.size_formatted)
                };

                let item = settings::item::builder(game.filename.as_str())
                    .description(status_label)
                    .checkbox(game.selected, move |val| Message::ToggleGameSelection(idx, val));

                games_section = games_section.add(item);
            }
        }

        let mut content = widget::column::with_capacity(9)
            .push(title)
            .push(desc)
            .push(system_scroll)
            .push(sys_header_section);

        if let Some(conf) = confirmation_section {
            content = content.push(conf);
        }

        content = content
            .push(toolbar)
            .push(search_bar)
            .push(games_section)
            .spacing(20)
            .padding(24)
            .max_width(940);

        widget::container(content)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
    }

    fn view_chd_converter(&self) -> Element<'_, Message> {
        let title = widget::text::title2("Conversor CHD (CUE/BIN ➔ CHD)");
        let desc = widget::text::body(
            "Convierte y comprime imágenes de CD/DVD (PlayStation, Saturn, Sega CD, etc.) \
            al formato CHD utilizando la herramienta chdman integrada.",
        );

        let chdman_badge = if let Some(ref ver) = self.chdman_version {
            widget::container(
                widget::text::caption(format!("✓ {ver} disponible"))
            )
            .padding([4, 8])
        } else {
            widget::container(
                widget::text::caption("⚠ chdman no encontrado en /usr/bin. Instala 'mame-tools'.")
            )
            .padding([4, 8])
        };

        // Sección 1: Archivo individual
        let cue_input = widget::text_input::text_input("Ruta a tu archivo .cue...", &self.single_cue_path)
            .on_input(Message::SingleCuePathChanged);

        let browse_cue_btn = widget::button::standard("Examinar .cue...")
            .on_press(Message::PickSingleCueFile);

        let cue_row = widget::row::with_capacity(2)
            .push(cue_input)
            .push(browse_cue_btn)
            .spacing(8)
            .width(Length::Fill);

        let out_input = widget::text_input::text_input("Ruta del archivo de salida .chd...", &self.single_output_path)
            .on_input(Message::SingleOutputPathChanged)
            .width(Length::Fill);

        let single_convert_btn = if self.is_converting {
            widget::button::suggested("Convirtiendo...").width(Length::Shrink)
        } else if self.single_cue_path.trim().is_empty() {
            widget::button::standard("Convertir a CHD").width(Length::Shrink)
        } else {
            widget::button::suggested("Convertir a CHD")
                .on_press(Message::StartSingleConversion)
                .width(Length::Shrink)
        };

        let single_section = settings::section()
            .title("Conversión de Archivo Individual")
            .add(
                settings::item("Archivo CUE de entrada", cue_row),
            )
            .add(
                settings::item("Archivo CHD de salida", out_input),
            )
            .add(
                settings::item("Iniciar conversión", single_convert_btn),
            );

        // Sección 2: Conversión por lotes
        let folder_input = widget::text_input::text_input("Ruta de la carpeta con juegos...", &self.batch_folder_path)
            .on_input(Message::BatchFolderPathChanged);

        let browse_folder_btn = widget::button::standard("Examinar carpeta...")
            .on_press(Message::PickBatchFolder);

        let scan_folder_btn = widget::button::standard("Buscar .cue")
            .on_press(Message::ScanFolderCues);

        let folder_row = widget::row::with_capacity(3)
            .push(folder_input)
            .push(browse_folder_btn)
            .push(scan_folder_btn)
            .spacing(8)
            .width(Length::Fill);

        let batch_info = format!("{} archivo(s) .cue listado(s)", self.found_cues.len());

        let batch_convert_btn = if self.is_converting {
            widget::button::suggested("Procesando lote...").width(Length::Shrink)
        } else if self.found_cues.is_empty() {
            widget::button::standard("Convertir todos a CHD").width(Length::Shrink)
        } else {
            widget::button::suggested(format!("Convertir {} juego(s) a CHD", self.found_cues.len()))
                .on_press(Message::StartBatchConversion)
                .width(Length::Shrink)
        };

        let batch_section = settings::section()
            .title("Conversión por Lotes (Múltiples Juegos)")
            .add(
                settings::item("Carpeta de origen", folder_row),
            )
            .add(
                settings::item("Juegos encontrados", widget::text::body(batch_info)),
            )
            .add(
                settings::item("Procesar lote completo", batch_convert_btn),
            );

        // Sección 3: Opciones
        let force_toggler = widget::toggler(self.force_overwrite)
            .on_toggle(Message::ForceOverwriteToggled);

        let delete_toggler = widget::toggler(self.delete_source_after)
            .on_toggle(Message::DeleteSourceToggled);

        let options_section = settings::section()
            .title("Opciones de chdman")
            .add(
                settings::item(
                    "Sobreescribir destino si ya existe (-f / --force)",
                    force_toggler,
                ),
            )
            .add(
                settings::item(
                    "Eliminar archivos .cue y .bin originales tras finalizar",
                    delete_toggler,
                ),
            );

        // Sección 4: Logs
        let clear_log_btn = widget::button::standard("Limpiar registro")
            .on_press(Message::ClearLogs);

        let log_header = widget::row::with_capacity(3)
            .push(widget::text::title3("Registro de Actividad"))
            .push(cosmic::iced::widget::space::horizontal().width(Length::Fill))
            .push(clear_log_btn)
            .align_y(cosmic::iced::alignment::Vertical::Center);

        let log_display = widget::container(
            widget::scrollable(
                widget::text(&self.log_output)
                    .size(13)
                    .width(Length::Fill),
            )
            .height(Length::Fixed(180.0)),
        )
        .padding(12)
        .width(Length::Fill);

        let content = widget::column::with_capacity(8)
            .push(title)
            .push(desc)
            .push(chdman_badge)
            .push(single_section)
            .push(batch_section)
            .push(options_section)
            .push(log_header)
            .push(log_display)
            .spacing(20)
            .padding(24)
            .max_width(940);

        widget::container(content)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let title = widget::text::title2("Configuración");
        let desc = widget::text::body("Parámetros de conexión Wi-Fi (SSH/SFTP), base de datos y carpetas locales.");

        let host_input = widget::text_input::text_input("batocera.local", &self.host_ip)
            .on_input(Message::HostChanged);

        let port_input = widget::text_input::text_input("22", &self.ssh_port_str)
            .on_input(Message::PortChanged);

        let user_input = widget::text_input::text_input("root", &self.ssh_user)
            .on_input(Message::UserChanged);

        let pass_input = widget::text_input::text_input("linux", &self.ssh_password)
            .on_input(Message::PasswordChanged);

        let folder_input = widget::text_input::text_input("~/Batocera", &self.local_base_dir_str)
            .on_input(Message::LocalFolderChanged);

        let browse_folder_btn = widget::button::standard("Examinar...")
            .on_press(Message::PickLocalFolder);

        let folder_row = widget::row::with_capacity(2)
            .push(folder_input)
            .push(browse_folder_btn)
            .spacing(8)
            .width(Length::Fill);

        let sync_toggler = widget::toggler(self.auto_sync)
            .on_toggle(Message::AutoSyncToggled);

        let net_section = settings::section()
            .title("Conexión Wi-Fi / SSH (Anbernic)")
            .add(
                settings::item("Dirección IP o Host (ej. batocera.local)", host_input),
            )
            .add(
                settings::item("Puerto SSH", port_input),
            )
            .add(
                settings::item("Usuario SSH (por defecto: root)", user_input),
            )
            .add(
                settings::item("Contraseña SSH (por defecto: linux)", pass_input),
            );

        let storage_section = settings::section()
            .title("Almacenamiento Local y Base de Datos")
            .add(
                settings::item("Carpeta local de juegos y partidas", folder_row),
            )
            .add(
                settings::item("Base de datos SQLite", widget::text::caption(&self.db_path_str)),
            )
            .add(
                settings::item("Sincronización automática de partidas", sync_toggler),
            );

        let app_info_section = settings::section()
            .title("Información de la Aplicación")
            .add(
                settings::item("Nombre", widget::text::body("batocera_manager")),
            )
            .add(
                settings::item("Versión", widget::text::body("0.0.1")),
            )
            .add(
                settings::item("Plataforma", widget::text::body("libcosmic / COSMIC Desktop")),
            );

        let content = widget::column::with_capacity(5)
            .push(title)
            .push(desc)
            .push(net_section)
            .push(storage_section)
            .push(app_info_section)
            .spacing(20)
            .padding(24)
            .max_width(940);

        widget::container(content)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .into()
    }
}
