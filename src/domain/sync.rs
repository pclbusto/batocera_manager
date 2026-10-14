#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncStatus {
    InSync,
    OnlyLocal,
    OnlyRemote,
    LocalNewer,
    RemoteNewer,
    Conflict,
}

impl SyncStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SyncStatus::InSync => "in_sync",
            SyncStatus::OnlyLocal => "only_local",
            SyncStatus::OnlyRemote => "only_remote",
            SyncStatus::LocalNewer => "local_newer",
            SyncStatus::RemoteNewer => "remote_newer",
            SyncStatus::Conflict => "conflict",
        }
    }

    #[allow(dead_code)]
    pub fn display_name(&self) -> &'static str {
        match self {
            SyncStatus::InSync => "🟢 Sincronizado",
            SyncStatus::OnlyLocal => "📤 Solo en PC",
            SyncStatus::OnlyRemote => "📥 Solo en Anbernic",
            SyncStatus::LocalNewer => "⬆️ Más reciente en PC",
            SyncStatus::RemoteNewer => "⬇️ Más reciente en Anbernic",
            SyncStatus::Conflict => "⚠️ Conflicto",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "in_sync" => SyncStatus::InSync,
            "only_local" => SyncStatus::OnlyLocal,
            "only_remote" => SyncStatus::OnlyRemote,
            "local_newer" => SyncStatus::LocalNewer,
            "remote_newer" => SyncStatus::RemoteNewer,
            _ => SyncStatus::Conflict,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SyncFileRecord {
    pub relative_path: String,
    pub category: String,
    pub local_size: Option<u64>,
    pub local_mtime: Option<u64>,
    pub local_hash: Option<String>,
    pub remote_size: Option<u64>,
    pub remote_mtime: Option<u64>,
    pub remote_hash: Option<String>,
    pub status: SyncStatus,
    pub last_sync: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct SyncSummary {
    pub in_sync: usize,
    pub only_local: usize,
    pub only_remote: usize,
    pub local_newer: usize,
    pub remote_newer: usize,
    pub total: usize,
}
