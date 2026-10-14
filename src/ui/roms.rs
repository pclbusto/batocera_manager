use cosmic::iced::alignment::Horizontal;
use cosmic::iced::Length;
use cosmic::widget::{self, settings};
use cosmic::Element;
use crate::{BatoceraManagerApp, Message};
use crate::systems;

pub fn view(app: &BatoceraManagerApp) -> Element<'_, Message> {
    let title = widget::text::title2("Explorador y Copiado de Juegos a la Consola");
    let desc = widget::text::body(
        "Selecciona tu consola y marca con las casillas los juegos específicos que deseas copiar directamente a la Anbernic por Wi-Fi.\n\
        No necesitas sincronizar toda la colección: aquí tienes control juego a juego.",
    );

    // 1. Selector de Consola mediante botones
    let mut system_buttons = widget::row::with_capacity(app.systems.len()).spacing(8);
    for (idx, sys) in app.systems.iter().enumerate() {
        let btn = if idx == app.selected_system_index {
            widget::button::suggested(sys.name.as_str())
        } else {
            widget::button::standard(sys.name.as_str()).on_press(Message::SelectSystemForGames(idx))
        };
        system_buttons = system_buttons.push(btn);
    }

    let system_scroll = widget::scrollable::horizontal(system_buttons);

    // 2. Información del sistema seleccionado
    let current_sys = &app.systems[app.selected_system_index];
    let sys_info_text = format!(
        "Carpeta en tu PC: {}  ➔  Destino Anbernic: /userdata/{}",
        current_sys.local_folder_str, current_sys.remote_folder
    );

    let browse_btn = widget::button::standard("Cambiar carpeta...")
        .on_press(Message::PickSystemFolder(app.selected_system_index));

    let sys_header_section = settings::section()
        .title(format!("Consola activa: {}", current_sys.name))
        .add(
            settings::item(sys_info_text, browse_btn),
        );

    // 3. Barra de herramientas para selección y copiado
    let selected_count = app.games_list.iter().filter(|g| g.selected).count();
    let total_selected_bytes: u64 = app.games_list.iter().filter(|g| g.selected).map(|g| g.size_bytes).sum();

    let copy_btn = if app.is_copying_games {
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
    let confirmation_section: Option<Element<'_, Message>> = if app.show_copy_confirmation {
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

    let search_bar = widget::text_input::search_input("Buscar por título de juego...", &app.rom_search_query)
        .on_input(Message::SearchRoms);

    // 4. Lista de juegos
    let query_lower = app.rom_search_query.to_lowercase();
    let filtered_games: Vec<(usize, &systems::GameItem)> = app
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
