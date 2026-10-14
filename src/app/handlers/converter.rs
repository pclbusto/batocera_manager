use cosmic::app::Task;
use std::path::PathBuf;
use crate::services;
use crate::app::message::Message;
use crate::app::state::BatoceraManagerApp;

pub fn handle_pick_single_cue() -> Task<Message> {
    Task::perform(
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
    )
}

pub fn handle_single_cue_selected(app: &mut BatoceraManagerApp, opt: Option<PathBuf>) -> Task<Message> {
    if let Some(path) = opt {
        app.single_cue_path = path.display().to_string();
        let mut out_path = path.clone();
        out_path.set_extension("chd");
        app.single_output_path = out_path.display().to_string();
    }
    Task::none()
}

pub fn handle_single_cue_path_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    app.single_cue_path = val.clone();
    if val.to_lowercase().ends_with(".cue") {
        let mut path = PathBuf::from(val);
        path.set_extension("chd");
        app.single_output_path = path.display().to_string();
    }
    Task::none()
}

pub fn handle_single_output_path_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    app.single_output_path = val;
    Task::none()
}

pub fn handle_pick_batch_folder() -> Task<Message> {
    Task::perform(
        async {
            tokio::task::spawn_blocking(|| {
                rfd::FileDialog::new().pick_folder()
            })
            .await
            .ok()
            .flatten()
        },
        |res| cosmic::Action::App(Message::BatchFolderSelected(res)),
    )
}

pub fn handle_batch_folder_selected(app: &mut BatoceraManagerApp, opt: Option<PathBuf>) -> Task<Message> {
    if let Some(path) = opt {
        app.batch_folder_path = path.display().to_string();
        app.found_cues = services::converter::find_cue_files(&path);
        app.log_output.push_str(&format!(
            "\n📁 Carpeta seleccionada: {}\nSe encontraron {} archivos .cue.",
            path.display(),
            app.found_cues.len()
        ));
    }
    Task::none()
}

pub fn handle_batch_folder_path_changed(app: &mut BatoceraManagerApp, val: String) -> Task<Message> {
    app.batch_folder_path = val;
    Task::none()
}

pub fn handle_scan_folder_cues(app: &mut BatoceraManagerApp) -> Task<Message> {
    let path = PathBuf::from(&app.batch_folder_path);
    if path.is_dir() {
        app.found_cues = services::converter::find_cue_files(&path);
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
    Task::none()
}

pub fn handle_force_overwrite_toggled(app: &mut BatoceraManagerApp, val: bool) -> Task<Message> {
    app.force_overwrite = val;
    Task::none()
}

pub fn handle_delete_source_toggled(app: &mut BatoceraManagerApp, val: bool) -> Task<Message> {
    app.delete_source_after = val;
    Task::none()
}

pub fn handle_start_single_conversion(app: &mut BatoceraManagerApp) -> Task<Message> {
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

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::converter::convert_single(&cue, &out, force, delete_src)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en hilo de ejecución: {e}")))
        },
        |res| cosmic::Action::App(Message::ConversionFinished(res)),
    )
}

pub fn handle_start_batch_conversion(app: &mut BatoceraManagerApp) -> Task<Message> {
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

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::converter::convert_batch(&cues, force, delete_src)
            })
            .await
            .unwrap_or_default()
        },
        |res| cosmic::Action::App(Message::BatchConversionFinished(res)),
    )
}

pub fn handle_conversion_finished(app: &mut BatoceraManagerApp, result: Result<String, String>) -> Task<Message> {
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
    Task::none()
}

pub fn handle_batch_conversion_finished(
    app: &mut BatoceraManagerApp,
    results: Vec<(String, Result<String, String>)>,
) -> Task<Message> {
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
    Task::none()
}
