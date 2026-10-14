use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::Length;
use cosmic::widget::{self, settings};
use cosmic::Element;
use crate::{BatoceraManagerApp, Message};

pub fn view(app: &BatoceraManagerApp) -> Element<'_, Message> {
    let title = widget::text::title2("Conversor CHD (CUE/BIN ➔ CHD)");
    let desc = widget::text::body(
        "Convierte y comprime imágenes de CD/DVD (PlayStation, Saturn, Sega CD, etc.) \
        al formato CHD utilizando la herramienta chdman integrada.",
    );

    let chdman_badge = if let Some(ref ver) = app.chdman_version {
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
    let cue_input = widget::text_input::text_input("Ruta a tu archivo .cue...", &app.single_cue_path)
        .on_input(Message::SingleCuePathChanged);

    let browse_cue_btn = widget::button::standard("Examinar .cue...")
        .on_press(Message::PickSingleCueFile);

    let cue_row = widget::row::with_capacity(2)
        .push(cue_input)
        .push(browse_cue_btn)
        .spacing(8)
        .width(Length::Fill);

    let out_input = widget::text_input::text_input("Ruta del archivo de salida .chd...", &app.single_output_path)
        .on_input(Message::SingleOutputPathChanged)
        .width(Length::Fill);

    let single_convert_btn = if app.is_converting {
        widget::button::suggested("Convirtiendo...").width(Length::Shrink)
    } else if app.single_cue_path.trim().is_empty() {
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
    let folder_input = widget::text_input::text_input("Ruta de la carpeta con juegos...", &app.batch_folder_path)
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

    let batch_info = format!("{} archivo(s) .cue listado(s)", app.found_cues.len());

    let batch_convert_btn = if app.is_converting {
        widget::button::suggested("Procesando lote...").width(Length::Shrink)
    } else if app.found_cues.is_empty() {
        widget::button::standard("Convertir todos a CHD").width(Length::Shrink)
    } else {
        widget::button::suggested(format!("Convertir {} juego(s) a CHD", app.found_cues.len()))
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
    let force_toggler = widget::toggler(app.force_overwrite)
        .on_toggle(Message::ForceOverwriteToggled);

    let delete_toggler = widget::toggler(app.delete_source_after)
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
        .align_y(Vertical::Center);

    let log_display = widget::container(
        widget::scrollable(
            widget::text(&app.log_output)
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
