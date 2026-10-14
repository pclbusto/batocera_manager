use cosmic::iced::alignment::Horizontal;
use cosmic::iced::Length;
use cosmic::widget::{self, settings};
use cosmic::Element;
use crate::{BatoceraManagerApp, Message};

pub fn view(app: &BatoceraManagerApp) -> Element<'_, Message> {
    let title = widget::text::title2("Configuración");
    let desc = widget::text::body("Parámetros de conexión Wi-Fi (SSH/SFTP), base de datos y carpetas locales.");

    let host_input = widget::text_input::text_input("batocera.local", &app.host_ip)
        .on_input(Message::HostChanged);

    let port_input = widget::text_input::text_input("22", &app.ssh_port_str)
        .on_input(Message::PortChanged);

    let user_input = widget::text_input::text_input("root", &app.ssh_user)
        .on_input(Message::UserChanged);

    let pass_input = widget::text_input::text_input("linux", &app.ssh_password)
        .on_input(Message::PasswordChanged);

    let folder_input = widget::text_input::text_input("~/Batocera", &app.local_base_dir_str)
        .on_input(Message::LocalFolderChanged);

    let browse_folder_btn = widget::button::standard("Examinar...")
        .on_press(Message::PickLocalFolder);

    let folder_row = widget::row::with_capacity(2)
        .push(folder_input)
        .push(browse_folder_btn)
        .spacing(8)
        .width(Length::Fill);

    let sync_toggler = widget::toggler(app.auto_sync)
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
            settings::item("Base de datos SQLite", widget::text::caption(&app.db_path_str)),
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
