use cosmic::app::Task;
use super::handlers;
use super::message::Message;
use super::state::BatoceraManagerApp;

pub fn handle_message(app: &mut BatoceraManagerApp, message: Message) -> Task<Message> {
    match message {
        // Navigation & Settings
        Message::NavSelect(id) => handlers::navigation::handle_nav_select(app, id),
        Message::ToggleAbout => handlers::navigation::handle_toggle_about(app),
        Message::OpenUrl(url) => handlers::navigation::handle_open_url(url),
        Message::ClearLogs => handlers::navigation::handle_clear_logs(app),
        Message::ToggleConnection => handlers::navigation::handle_toggle_connection(app),
        Message::HostChanged(val) => handlers::navigation::handle_host_changed(app, val),
        Message::PortChanged(val) => handlers::navigation::handle_port_changed(app, val),
        Message::UserChanged(val) => handlers::navigation::handle_user_changed(app, val),
        Message::PasswordChanged(val) => handlers::navigation::handle_password_changed(app, val),
        Message::LocalFolderChanged(val) => handlers::navigation::handle_local_folder_changed(app, val),
        Message::AutoSyncToggled(val) => handlers::navigation::handle_auto_sync_toggled(app, val),
        Message::RefreshStatus => handlers::navigation::handle_refresh_status(app),
        Message::PickLocalFolder => handlers::navigation::handle_pick_local_folder(),
        Message::LocalFolderSelected(opt) => handlers::navigation::handle_local_folder_selected(app, opt),

        // Systems
        Message::ToggleSystem(idx, val) => handlers::systems::handle_toggle_system(app, idx, val),
        Message::SystemFolderInputChanged(idx, val) => handlers::systems::handle_system_folder_input(app, idx, val),
        Message::PickSystemFolder(idx) => handlers::systems::handle_pick_system_folder(idx),
        Message::SystemFolderSelected(idx, opt) => handlers::systems::handle_system_folder_selected(app, idx, opt),
        Message::RescanSystems => handlers::systems::handle_rescan_systems(app),
        Message::SyncSingleSystem(idx) => handlers::systems::handle_sync_single_system(app, idx),
        Message::SingleSystemSyncFinished(idx, res) => handlers::systems::handle_single_system_sync_finished(app, idx, res),
        Message::SyncAllEnabledSystems => handlers::systems::handle_sync_all_systems(app),
        Message::AllSystemsSyncFinished(res) => handlers::systems::handle_all_systems_sync_finished(app, res),

        // Games & Transfers
        Message::SearchRoms(val) => handlers::games::handle_search_roms(app, val),
        Message::SelectSystemForGames(idx) => handlers::games::handle_select_system_for_games(app, idx),
        Message::ToggleGameSelection(idx, val) => handlers::games::handle_toggle_game_selection(app, idx, val),
        Message::SelectAllMissingGames => handlers::games::handle_select_all_missing(app),
        Message::DeselectAllGames => handlers::games::handle_deselect_all(app),
        Message::RefreshGamesList => handlers::games::handle_refresh_games_list(app),
        Message::CopySelectedGamesToConsole => handlers::games::handle_copy_selected_games(app),
        Message::CancelCopyConfirmation => handlers::games::handle_cancel_copy_confirmation(app),
        Message::ConfirmCopySelectedGames => handlers::games::handle_confirm_copy_selected_games(app),
        Message::CopySelectedGamesFinished(res) => handlers::games::handle_copy_selected_finished(app, res),

        // SD Diagnostics & Waste Cleanup
        Message::RunDiagnostic => handlers::diagnostics::handle_run_diagnostic(app),
        Message::DiagnosticFinished(res) => handlers::diagnostics::handle_diagnostic_finished(app, res),
        Message::ToggleWasteCandidate(idx, val) => handlers::diagnostics::handle_toggle_waste_candidate(app, idx, val),
        Message::SelectAllWaste(val) => handlers::diagnostics::handle_select_all_waste(app, val),
        Message::RequestCleanWaste => handlers::diagnostics::handle_request_clean_waste(app),
        Message::CancelCleanWaste => handlers::diagnostics::handle_cancel_clean_waste(app),
        Message::ConfirmCleanWaste => handlers::diagnostics::handle_confirm_clean_waste(app),
        Message::CleanWasteFinished(res) => handlers::diagnostics::handle_clean_waste_finished(app, res),

        // WiFi & Sync
        Message::TestWifiConnection => handlers::sync::handle_test_wifi(app),
        Message::WifiConnectionTested(res) => handlers::sync::handle_wifi_tested(app, res),
        Message::StartScanAndReconcile => handlers::sync::handle_start_scan_and_reconcile(app),
        Message::ScanAndReconcileFinished(res) => handlers::sync::handle_scan_and_reconcile_finished(app, res),
        Message::StartSyncSaves => handlers::sync::handle_start_sync_saves(app),
        Message::SyncSavesFinished(res) => handlers::sync::handle_sync_saves_finished(app, res),
        Message::FilterCategory(cat) => handlers::sync::handle_filter_category(app, cat),

        // CHD Converter
        Message::PickSingleCueFile => handlers::converter::handle_pick_single_cue(),
        Message::SingleCueSelected(opt) => handlers::converter::handle_single_cue_selected(app, opt),
        Message::SingleCuePathChanged(val) => handlers::converter::handle_single_cue_path_changed(app, val),
        Message::SingleOutputPathChanged(val) => handlers::converter::handle_single_output_path_changed(app, val),
        Message::PickBatchFolder => handlers::converter::handle_pick_batch_folder(),
        Message::BatchFolderSelected(opt) => handlers::converter::handle_batch_folder_selected(app, opt),
        Message::BatchFolderPathChanged(val) => handlers::converter::handle_batch_folder_path_changed(app, val),
        Message::ScanFolderCues => handlers::converter::handle_scan_folder_cues(app),
        Message::ForceOverwriteToggled(val) => handlers::converter::handle_force_overwrite_toggled(app, val),
        Message::DeleteSourceToggled(val) => handlers::converter::handle_delete_source_toggled(app, val),
        Message::StartSingleConversion => handlers::converter::handle_start_single_conversion(app),
        Message::StartBatchConversion => handlers::converter::handle_start_batch_conversion(app),
        Message::ConversionFinished(res) => handlers::converter::handle_conversion_finished(app, res),
        Message::BatchConversionFinished(res) => handlers::converter::handle_batch_conversion_finished(app, res),
    }
}
