mod chd;
mod db;
mod diagnostic;
mod hash;
mod net;
mod sync;
mod systems;
mod ui;

use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{Length, Size};
use cosmic::prelude::*;
use cosmic::widget::about::About;
use cosmic::widget::{self, nav_bar};
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
    ToggleWasteCandidate(usize, bool),
    SelectAllWaste(bool),
    RequestCleanWaste,
    CancelCleanWaste,
    ConfirmCleanWaste,
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
            show_waste_confirmation: false,

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
            Message::ToggleWasteCandidate(idx, val) => {
                if let Some(ref mut diag) = self.latest_diagnostic {
                    if let Some(candidate) = diag.waste_candidates.get_mut(idx) {
                        candidate.selected = val;
                    }
                }
            }
            Message::SelectAllWaste(val) => {
                if let Some(ref mut diag) = self.latest_diagnostic {
                    for c in &mut diag.waste_candidates {
                        c.selected = val;
                    }
                }
            }
            Message::RequestCleanWaste => {
                let selected_count = self
                    .latest_diagnostic
                    .as_ref()
                    .map(|d| d.waste_candidates.iter().filter(|w| w.selected).count())
                    .unwrap_or(0);
                if selected_count > 0 {
                    self.show_waste_confirmation = true;
                } else {
                    self.log_output.push_str("⚠️ No hay residuos seleccionados para eliminar. Marca los archivos que deseas limpiar.\n");
                }
            }
            Message::CancelCleanWaste => {
                self.show_waste_confirmation = false;
            }
            Message::ConfirmCleanWaste => {
                self.show_waste_confirmation = false;
                let paths: Vec<String> = self
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
                    host: self.host_ip.clone(),
                    port: self.ssh_port,
                    user: self.ssh_user.clone(),
                    password: self.ssh_password.clone(),
                };
                self.log_output.push_str(&format!(
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
                        self.log_output.push_str(&format!("✅ Se eliminaron de forma segura {count} residuo(s).\n"));
                        self.latest_diagnostic = None;
                        self.show_waste_confirmation = false;
                        self.reload_games_for_selected_system();
                    }
                    Err(e) => {
                        self.log_output.push_str(&format!("⚠️ Resultado de limpieza: {e}\n"));
                        self.latest_diagnostic = None;
                        self.show_waste_confirmation = false;
                        self.reload_games_for_selected_system();
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
            Page::Dashboard => ui::dashboard::view(self),
            Page::Sync => ui::sync::view(self),
            Page::Roms => ui::roms::view(self),
            Page::ChdConverter => ui::chd::view(self),
            Page::Settings => ui::settings::view(self),
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








}
