use cosmic::iced::alignment::Horizontal;
use cosmic::iced::Length;
use cosmic::widget::{self, settings};
use cosmic::Element;
use crate::{BatoceraManagerApp, Message, APP_TITLE};

pub fn view(app: &BatoceraManagerApp) -> Element<'_, Message> {
    let app_badge = widget::text::title2(APP_TITLE);
    let app_subtitle = widget::text::body(
        "Administrador de sistemas por consola, copiado selectivo de juegos, sincronizador Wi-Fi y conversor CHD.",
    );

    let header_box = widget::column::with_capacity(2)
        .push(app_badge)
        .push(app_subtitle)
        .spacing(4);

    // Tarjeta de estado de conexión Wi-Fi
    let status_text = if app.is_connected {
        format!("🟢 Conectado por Wi-Fi a Batocera en {}:{}", app.host_ip, app.ssh_port)
    } else {
        format!("⚪ Sin comprobar / Desconectado ({}:{})", app.host_ip, app.ssh_port)
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
    let enabled_count = app.systems.iter().filter(|s| s.enabled).count();
    let total_local_roms: usize = app.systems.iter().map(|s| s.local_rom_count).sum();

    let systems_overview_card = settings::section()
        .title("Resumen de Consolas y Sistemas")
        .add(
            settings::item(
                "Sistemas activos para sincronizar",
                widget::text::body(format!("{enabled_count} de {} sistemas habilitados", app.systems.len())),
            ),
        )
        .add(
            settings::item(
                "Juegos locales detectados en PC",
                widget::text::body(format!("{total_local_roms} juegos distribuidos por carpetas")),
            ),
        );

    // Tarjeta chdman
    let chdman_label = if let Some(ref ver) = app.chdman_version {
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
    let diag_btn_label = if app.is_diagnosing {
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
