pub mod message;
pub mod page;
pub mod state;
pub mod update;

pub use message::Message;
pub use page::Page;
pub use state::BatoceraManagerApp;

use cosmic::app::context_drawer::{self, ContextDrawer};
use cosmic::app::{Core, Task};
use cosmic::iced::Length;
use cosmic::prelude::*;
use cosmic::widget::about::About;
use cosmic::widget::{self, nav_bar};
use std::path::PathBuf;

use crate::{chd, db, sync, systems, ui};

pub const APP_ID: &str = "com.github.batocera_manager";
pub const APP_TITLE: &str = "batocera_manager 0.0.1";

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
        let (sync_summary, sync_records, systems, saved_host, saved_port, saved_user, saved_pass, custom_folder) =
            if let Ok(conn) = db::init_db(&db_path) {
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

        let effective_base_dir = custom_folder.map(PathBuf::from).unwrap_or(local_base_dir);

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

        vec![conn_button.into(), about_button.into()]
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
        update::handle_message(self, message)
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
