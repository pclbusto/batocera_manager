use cosmic::app::Task;
use crate::domain::diagnostic::DiagnosticReport;
use crate::services;
use crate::app::message::Message;
use crate::app::state::BatoceraManagerApp;

pub fn handle_run_diagnostic(app: &mut BatoceraManagerApp) -> Task<Message> {
    if app.is_diagnosing {
        return Task::none();
    }
    app.is_diagnosing = true;
    let config = app.ssh_config();
    let sys_list = app.systems.clone();
    app.log_output.push_str("\n🔍 Iniciando diagnóstico no destructivo de almacenamiento en la consola...\n");

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::diagnostic::run_storage_diagnostic(&config, &sys_list)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
        },
        |res| cosmic::Action::App(Message::DiagnosticFinished(res)),
    )
}

pub fn handle_diagnostic_finished(app: &mut BatoceraManagerApp, res: Result<DiagnosticReport, String>) -> Task<Message> {
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
    Task::none()
}

pub fn handle_toggle_waste_candidate(app: &mut BatoceraManagerApp, idx: usize, val: bool) -> Task<Message> {
    if let Some(ref mut diag) = app.latest_diagnostic {
        if let Some(candidate) = diag.waste_candidates.get_mut(idx) {
            candidate.selected = val;
        }
    }
    Task::none()
}

pub fn handle_select_all_waste(app: &mut BatoceraManagerApp, val: bool) -> Task<Message> {
    if let Some(ref mut diag) = app.latest_diagnostic {
        for c in &mut diag.waste_candidates {
            c.selected = val;
        }
    }
    Task::none()
}

pub fn handle_request_clean_waste(app: &mut BatoceraManagerApp) -> Task<Message> {
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
    Task::none()
}

pub fn handle_cancel_clean_waste(app: &mut BatoceraManagerApp) -> Task<Message> {
    app.show_waste_confirmation = false;
    Task::none()
}

pub fn handle_confirm_clean_waste(app: &mut BatoceraManagerApp) -> Task<Message> {
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

    let config = app.ssh_config();
    app.log_output.push_str(&format!(
        "\n🧹 Iniciando eliminación segura de {} residuo(s) seleccionado(s) y confirmado(s)...\n",
        paths.len()
    ));

    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                services::diagnostic::clean_confirmed_waste(&config, &paths)
            })
            .await
            .unwrap_or_else(|e| Err(format!("Fallo en tarea: {e}")))
        },
        |res| cosmic::Action::App(Message::CleanWasteFinished(res)),
    )
}

pub fn handle_clean_waste_finished(app: &mut BatoceraManagerApp, res: Result<usize, String>) -> Task<Message> {
    match res {
        Ok(count) => {
            app.log_output.push_str(&format!("✅ Se eliminaron de forma segura {count} residuo(s).\n"));
        }
        Err(e) => {
            app.log_output.push_str(&format!("⚠️ Resultado de limpieza: {e}\n"));
        }
    }
    app.latest_diagnostic = None;
    app.show_waste_confirmation = false;
    app.reload_games_for_selected_system();
    Task::none()
}
