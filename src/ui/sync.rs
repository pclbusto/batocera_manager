use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::Length;
use cosmic::widget::{self, settings};
use cosmic::Element;
use crate::{BatoceraManagerApp, Message};
use crate::systems;

pub fn view(app: &BatoceraManagerApp) -> Element<'_, Message> {
    let title = widget::text::title2("Gestión e Inventario por Sistemas (Anbernic ⇄ PC)");
    let desc = widget::text::body(
        "Configura la carpeta local en tu PC para cada consola y mantén actualizado el inventario por Wi-Fi.\n\
        ¿Qué hace 'Actualizar Inventario'? Compara la consola contra tu PC y registra el estado en la base de datos local sin transferir ni borrar ningún archivo.\n\
        Para copiar juegos específicos a la consola, ve a la pestaña 'Explorador y Copiado de Juegos'.",
    );

    // 1. Barra de estado y acciones globales
    let enabled_count = app.systems.iter().filter(|s| s.enabled).count();
    let total_local_roms: usize = app.systems.iter().map(|s| s.local_rom_count).sum();

    let sync_all_btn = if app.is_syncing {
        widget::button::suggested("Reconciliando inventarios...").width(Length::Shrink)
    } else {
        widget::button::suggested(format!("🔄 Actualizar Inventario ({enabled_count} Sistemas)"))
            .on_press(Message::SyncAllEnabledSystems)
            .width(Length::Shrink)
    };

    let diag_btn = if app.is_diagnosing {
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
                widget::text::body(format!("{enabled_count} activados de {} totales  |  {total_local_roms} juegos en PC", app.systems.len())),
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
    let waste_section: Option<Element<'_, Message>> = if let Some(ref diag) = app.latest_diagnostic {
        if !diag.waste_candidates.is_empty() {
            let selected_count = diag.waste_candidates.iter().filter(|w| w.selected).count();
            let selected_bytes: u64 = diag
                .waste_candidates
                .iter()
                .filter(|w| w.selected)
                .map(|w| w.size_bytes)
                .sum();

            let mut section = settings::section()
                .title("⚠️ Residuos Detectados en Tarjeta SD (Archivos 0B / .part)");

            // Diálogo de confirmación explícita previa a la eliminación
            if app.show_waste_confirmation {
                let confirm_btn = widget::button::destructive("✓ Confirmar Eliminación")
                    .on_press(Message::ConfirmCleanWaste);
                let cancel_btn = widget::button::standard("✕ Cancelar")
                    .on_press(Message::CancelCleanWaste);
                let btn_row = widget::row::with_capacity(2)
                    .push(confirm_btn)
                    .push(cancel_btn)
                    .spacing(8);

                section = section.add(settings::item(
                    format!(
                        "¿Confirmas eliminar permanentemente {} archivo(s) seleccionado(s) ({}) de la consola? Esta acción no se puede deshacer.",
                        selected_count,
                        systems::format_size(selected_bytes)
                    ),
                    btn_row,
                ));
            }

            // Barra de herramientas: seleccionar / deseleccionar y botón de limpieza
            let select_all_btn = widget::button::standard("Seleccionar todos")
                .on_press(Message::SelectAllWaste(true));
            let deselect_all_btn = widget::button::standard("Deseleccionar todos")
                .on_press(Message::SelectAllWaste(false));

            let clean_btn = if selected_count > 0 {
                widget::button::destructive(format!(
                    "Limpiar {} seleccionados ({})",
                    selected_count,
                    systems::format_size(selected_bytes)
                ))
                .on_press(Message::RequestCleanWaste)
            } else {
                widget::button::destructive("Limpiar seleccionados (0)")
            };

            let toolbar = widget::row::with_capacity(3)
                .push(clean_btn)
                .push(select_all_btn)
                .push(deselect_all_btn)
                .spacing(8);

            section = section.add(settings::item(
                format!(
                    "Selecciona individualmente los archivos a eliminar ({} detectados, {} seleccionados):",
                    diag.waste_candidates.len(),
                    selected_count
                ),
                toolbar,
            ));

            // Lista de candidatos individuales con checkboxes
            for (idx, w) in diag.waste_candidates.iter().enumerate() {
                let desc = format!(
                    "Consola: {}  |  {}  |  {}",
                    w.system_id,
                    w.waste_type.description(),
                    systems::format_size(w.size_bytes)
                );
                let item = settings::item::builder(w.relative_path.as_str())
                    .description(desc)
                    .checkbox(w.selected, move |val| Message::ToggleWasteCandidate(idx, val));
                section = section.add(item);
            }

            Some(section.into())
        } else {
            None
        }
    } else {
        None
    };

    // 2. Tarjetas de cada sistema / consola
    let mut systems_column = widget::column::with_capacity(app.systems.len() + 1)
        .spacing(16);

    systems_column = systems_column.push(widget::text::title3("Consolas y Sistemas Configurados"));

    for (idx, sys) in app.systems.iter().enumerate() {
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

        let sync_btn = if app.is_syncing {
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
