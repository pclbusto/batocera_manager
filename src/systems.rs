use crate::db::{self, SyncFileRecord, SyncStatus};
use crate::hash;
use crate::net::{self, SshConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct SystemConfig {
    pub id: String,
    pub name: String,
    pub local_folder: PathBuf,
    pub local_folder_str: String,
    pub remote_folder: String,
    pub enabled: bool,
    pub local_rom_count: usize,
    #[allow(dead_code)]
    pub remote_rom_count: usize,
    pub status_msg: String,
}

impl SystemConfig {
    pub fn new(id: &str, name: &str, base_roms: &Path, enabled: bool) -> Self {
        let folder = base_roms.join(id);
        let folder_str = folder.display().to_string();
        let local_count = count_files_in_dir(&folder);

        Self {
            id: id.to_string(),
            name: name.to_string(),
            local_folder: folder,
            local_folder_str: folder_str,
            remote_folder: format!("roms/{id}"),
            enabled,
            local_rom_count: local_count,
            remote_rom_count: 0,
            status_msg: "Sin escanear".to_string(),
        }
    }
}

pub fn count_files_in_dir(dir: &Path) -> usize {
    if !dir.exists() || !dir.is_dir() {
        return 0;
    }
    let mut count = 0;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                count += 1;
            } else if path.is_dir() {
                count += count_files_in_dir(&path);
            }
        }
    }
    count
}

/// Lista de sistemas predeterminados populares en Batocera / Anbernic
pub fn default_systems_list(base_roms: &Path) -> Vec<SystemConfig> {
    vec![
        SystemConfig::new("psx", "Sony PlayStation (PS1)", base_roms, true),
        SystemConfig::new("snes", "Super Nintendo (SNES)", base_roms, true),
        SystemConfig::new("gba", "Nintendo Game Boy Advance", base_roms, true),
        SystemConfig::new("megadrive", "Sega Genesis / Mega Drive", base_roms, true),
        SystemConfig::new("nes", "Nintendo Entertainment System", base_roms, true),
        SystemConfig::new("n64", "Nintendo 64", base_roms, false),
        SystemConfig::new("arcade", "Arcade (MAME / FBNeo)", base_roms, false),
        SystemConfig::new("psp", "Sony PlayStation Portable", base_roms, false),
        SystemConfig::new("gb", "Nintendo Game Boy", base_roms, false),
        SystemConfig::new("gbc", "Nintendo Game Boy Color", base_roms, false),
        SystemConfig::new("nds", "Nintendo DS", base_roms, false),
        SystemConfig::new("neogeo", "SNK Neo Geo", base_roms, false),
        SystemConfig::new("saturn", "Sega Saturn", base_roms, false),
        SystemConfig::new("dreamcast", "Sega Dreamcast", base_roms, false),
    ]
}

/// Sincroniza únicamente un sistema específico por Wi-Fi
pub fn sync_system(
    system: &SystemConfig,
    ssh_config: &SshConfig,
    db_path: &Path,
) -> Result<String, String> {
    if !system.local_folder.exists() {
        let _ = fs::create_dir_all(&system.local_folder);
    }

    let conn = db::init_db(db_path).map_err(|e| format!("Error en base de datos: {e}"))?;
    let sess = net::create_session(ssh_config)?;
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Error abriendo canal SFTP: {e}"))?;

    // 1. Escanear archivos remotos en /userdata/roms/<id>
    let remote_roms_dir = format!("/userdata/{}", system.remote_folder);
    let remote_items = net::list_remote_files_recursive(&sftp, &remote_roms_dir, "")
        .unwrap_or_default();

    let mut remote_map = std::collections::HashMap::new();
    for item in remote_items {
        remote_map.insert(item.relative_path.clone(), item.size);
    }

    // 2. Escanear archivos locales
    let mut local_files = Vec::new();
    fn scan_local(root: &Path, cur: &Path, list: &mut Vec<(String, PathBuf, u64)>) {
        if let Ok(entries) = fs::read_dir(cur) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    scan_local(root, &p, list);
                } else if p.is_file() {
                    if let Ok(rel) = p.strip_prefix(root) {
                        let rel_str = rel.to_string_lossy().to_string();
                        if let Ok(meta) = p.metadata() {
                            list.push((rel_str, p, meta.len()));
                        }
                    }
                }
            }
        }
    }
    scan_local(&system.local_folder, &system.local_folder, &mut local_files);

    let mut uploaded_count = 0;
    let mut synced_count = 0;

    for (rel_name, local_path, local_size) in local_files {
        let remote_full_path = format!("{remote_roms_dir}/{rel_name}");
        let relative_db_path = format!("{}/{}", system.remote_folder, rel_name);

        let needs_upload = match remote_map.get(&rel_name) {
            Some(&rem_size) => {
                if rem_size == local_size {
                    synced_count += 1;
                    false
                } else {
                    true
                }
            }
            None => true,
        };

        if needs_upload {
            if let Ok(bytes) = net::upload_file(&sftp, &local_path, &remote_full_path) {
                uploaded_count += 1;

                let hash_val = hash::compute_file_hash(&local_path).ok();
                let rec = SyncFileRecord {
                    relative_path: relative_db_path,
                    category: "roms".to_string(),
                    local_size: Some(local_size),
                    local_mtime: Some(
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                    local_hash: hash_val,
                    remote_size: Some(bytes),
                    remote_mtime: None,
                    remote_hash: None,
                    status: SyncStatus::InSync,
                    last_sync: Some(
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                };
                let _ = db::upsert_file_record(&conn, &rec);
            }
        } else {
            let rec = SyncFileRecord {
                relative_path: relative_db_path,
                category: "roms".to_string(),
                local_size: Some(local_size),
                local_mtime: None,
                local_hash: None,
                remote_size: Some(local_size),
                remote_mtime: None,
                remote_hash: None,
                status: SyncStatus::InSync,
                last_sync: Some(
                    SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                ),
            };
            let _ = db::upsert_file_record(&conn, &rec);
        }
    }

    Ok(format!(
        "[{}] Sincronización finalizada: {} juegos subidos a Anbernic, {} ya estaban sincronizados.",
        system.name, uploaded_count, synced_count
    ))
}

#[derive(Clone, Debug)]
pub struct GameItem {
    pub filename: String,
    pub path: PathBuf,
    #[allow(dead_code)]
    pub size_bytes: u64,
    pub size_formatted: String,
    pub exists_on_console: bool,
    pub selected: bool,
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

pub fn scan_games_for_system(local_folder: &Path, remote_names: &[String]) -> Vec<GameItem> {
    let mut games = Vec::new();
    if let Ok(entries) = fs::read_dir(local_folder) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') {
                    continue;
                }
                let meta = p.metadata().ok();
                let size = meta.map(|m| m.len()).unwrap_or(0);
                let exists = remote_names.iter().any(|r| r == &name);

                games.push(GameItem {
                    filename: name,
                    path: p,
                    size_bytes: size,
                    size_formatted: format_size(size),
                    exists_on_console: exists,
                    selected: !exists, // Pre-seleccionar si aún no está en la consola para facilitar el copiado
                });
            }
        }
    }
    games.sort_by(|a, b| a.filename.to_lowercase().cmp(&b.filename.to_lowercase()));
    games
}

pub fn copy_selected_games_to_console(
    games: &[GameItem],
    remote_folder: &str,
    ssh_config: &SshConfig,
    db_path: &Path,
) -> Result<String, String> {
    let sess = net::create_session(ssh_config)?;
    let sftp = sess.sftp().map_err(|e| format!("Error SFTP: {e}"))?;
    let conn = db::init_db(db_path).ok();

    let mut copied = 0;
    let mut errors = Vec::new();

    for game in games.iter().filter(|g| g.selected) {
        let remote_dest = format!("/userdata/{}/{}", remote_folder, game.filename);
        match net::upload_file(&sftp, &game.path, &remote_dest) {
            Ok(bytes) => {
                copied += 1;
                if let Some(ref c) = conn {
                    let hash_val = hash::compute_file_hash(&game.path).ok();
                    let rec = SyncFileRecord {
                        relative_path: format!("{}/{}", remote_folder, game.filename),
                        category: "roms".to_string(),
                        local_size: Some(game.size_bytes),
                        local_mtime: Some(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                        local_hash: hash_val,
                        remote_size: Some(bytes),
                        remote_mtime: None,
                        remote_hash: None,
                        status: SyncStatus::InSync,
                        last_sync: Some(
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        ),
                    };
                    let _ = db::upsert_file_record(c, &rec);
                }
            }
            Err(e) => errors.push(format!("{}: {e}", game.filename)),
        }
    }

    if !errors.is_empty() {
        Err(format!("Copiados {copied} juegos. Errores: {}", errors.join("; ")))
    } else {
        Ok(format!("Se copiaron exitosamente {copied} juego(s) a la consola."))
    }
}
