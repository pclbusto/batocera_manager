use cosmic::widget::nav_bar;
use std::path::PathBuf;
use crate::{db, diagnostic};

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
