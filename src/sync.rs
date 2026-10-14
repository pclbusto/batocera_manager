use crate::db::{self, SyncFileRecord, SyncStatus, SyncSummary};
use crate::hash;
use crate::net::{self, SshConfig};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub fn default_local_base_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join("Batocera");
    let _ = fs::create_dir_all(dir.join("roms"));
    let _ = fs::create_dir_all(dir.join("saves"));
    let _ = fs::create_dir_all(dir.join("bios"));
    dir
}

pub fn default_backup_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    let dir = PathBuf::from(home).join(".local/share/batocera_manager/backups");
    let _ = fs::create_dir_all(&dir);
    dir
}

/// Escanea recursivamente un directorio local y recolecta archivos con su tamaño y mtime
fn scan_local_folder(base_dir: &Path, category: &str) -> HashMap<String, (u64, u64)> {
    let mut files = HashMap::new();
    let cat_dir = base_dir.join(category);
    if !cat_dir.exists() {
        return files;
    }

    fn walk(root: &Path, current: &Path, category: &str, files: &mut HashMap<String, (u64, u64)>) {
        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(root, &path, category, files);
                } else if path.is_file() {
                    if let Ok(rel) = path.strip_prefix(root) {
                        let rel_str = format!("{}/{}", category, rel.to_string_lossy());
                        if let Ok(meta) = path.metadata() {
                            let size = meta.len();
                            let mtime = meta
                                .modified()
                                .ok()
                                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs())
                                .unwrap_or(0);
                            files.insert(rel_str, (size, mtime));
                        }
                    }
                }
            }
        }
    }

    walk(&cat_dir, &cat_dir, category, &mut files);
    files
}

/// Escanea localmente y remotamente vía Wi-Fi, calcula hashes y actualiza la base de datos SQLite
pub fn scan_and_reconcile(
    db_path: &Path,
    local_base: &Path,
    ssh_config: &SshConfig,
) -> Result<SyncSummary, String> {
    let conn = db::init_db(db_path).map_err(|e| format!("Error en base de datos: {e}"))?;

    // 1. Escanear archivos locales
    let mut local_map = HashMap::new();
    local_map.extend(scan_local_folder(local_base, "saves"));
    local_map.extend(scan_local_folder(local_base, "roms"));
    local_map.extend(scan_local_folder(local_base, "bios"));

    // 2. Conectar por SSH/SFTP y escanear archivos en la Anbernic
    let sess = net::create_session(ssh_config)?;
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Error abriendo canal SFTP: {e}"))?;

    let mut remote_map = HashMap::new();
    let categories = ["saves", "roms", "bios"];
    for cat in &categories {
        if let Ok(remote_items) = net::list_remote_files_recursive(&sftp, "/userdata", cat) {
            for item in remote_items {
                let full_rel = format!("{}/{}", cat, item.relative_path);
                remote_map.insert(full_rel, (item.size, item.mtime));
            }
        }
    }

    // 3. Cruzar datos
    let mut all_paths: HashSet<String> = HashSet::new();
    for k in local_map.keys() {
        all_paths.insert(k.clone());
    }
    for k in remote_map.keys() {
        all_paths.insert(k.clone());
    }

    for path in all_paths {
        let cat = if path.starts_with("saves/") {
            "saves"
        } else if path.starts_with("roms/") {
            "roms"
        } else if path.starts_with("bios/") {
            "bios"
        } else {
            "other"
        };

        let local_info = local_map.get(&path).copied();
        let remote_info = remote_map.get(&path).copied();

        // Calcular hash local
        let local_hash = if let Some(_) = local_info {
            let full_local_path = local_base.join(&path);
            hash::compute_file_hash(&full_local_path).ok()
        } else {
            None
        };

        let status = match (local_info, remote_info) {
            (Some(_), None) => SyncStatus::OnlyLocal,
            (None, Some(_)) => SyncStatus::OnlyRemote,
            (Some((l_size, l_mtime)), Some((r_size, r_mtime))) => {
                if cat == "saves" {
                    // En partidas guardadas, la fecha más reciente indica dónde se jugó por última vez
                    if r_mtime > l_mtime + 2 {
                        SyncStatus::RemoteNewer
                    } else if l_mtime > r_mtime + 2 {
                        SyncStatus::LocalNewer
                    } else if l_size == r_size {
                        SyncStatus::InSync
                    } else {
                        SyncStatus::Conflict
                    }
                } else if l_size == r_size {
                    SyncStatus::InSync
                } else {
                    SyncStatus::Conflict
                }
            }
            (None, None) => continue,
        };

        let record = SyncFileRecord {
            relative_path: path,
            category: cat.to_string(),
            local_size: local_info.map(|i| i.0),
            local_mtime: local_info.map(|i| i.1),
            local_hash,
            remote_size: remote_info.map(|i| i.0),
            remote_mtime: remote_info.map(|i| i.1),
            remote_hash: None,
            status,
            last_sync: Some(
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            ),
        };

        let _ = db::upsert_file_record(&conn, &record);
    }

    db::get_sync_summary(&conn).map_err(|e| format!("Error leyendo resumen de BD: {e}"))
}

/// Sincroniza partidas guardadas (Saves) en ambas direcciones de forma segura
pub fn sync_saves_bidirectional(
    db_path: &Path,
    local_base: &Path,
    ssh_config: &SshConfig,
) -> Result<String, String> {
    let conn = db::init_db(db_path).map_err(|e| format!("Error en base de datos: {e}"))?;
    let records = db::get_all_records(&conn, Some("saves"))
        .map_err(|e| format!("Error consultando registros: {e}"))?;

    let sess = net::create_session(ssh_config)?;
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Error abriendo canal SFTP: {e}"))?;

    let mut downloaded = 0;
    let mut uploaded = 0;
    let backup_dir = default_backup_dir();

    for mut rec in records {
        let local_file = local_base.join(&rec.relative_path);
        let remote_file = format!("/userdata/{}", rec.relative_path);

        match rec.status {
            SyncStatus::RemoteNewer | SyncStatus::OnlyRemote => {
                // Hacer backup local si ya existía una versión anterior en el PC
                if local_file.exists() {
                    let ts = SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let bkp_name = format!(
                        "{}.bkp.{}",
                        local_file.file_name().unwrap_or_default().to_string_lossy(),
                        ts
                    );
                    let _ = fs::copy(&local_file, backup_dir.join(bkp_name));
                }

                // Descargar desde la Anbernic al PC
                if let Ok(bytes) = net::download_file(&sftp, &remote_file, &local_file) {
                    downloaded += 1;
                    rec.status = SyncStatus::InSync;
                    rec.local_size = Some(bytes);
                    rec.local_hash = hash::compute_file_hash(&local_file).ok();
                    let _ = db::upsert_file_record(&conn, &rec);
                }
            }
            SyncStatus::LocalNewer | SyncStatus::OnlyLocal => {
                // Subir desde el PC a la Anbernic
                if let Ok(bytes) = net::upload_file(&sftp, &local_file, &remote_file) {
                    uploaded += 1;
                    rec.status = SyncStatus::InSync;
                    rec.remote_size = Some(bytes);
                    let _ = db::upsert_file_record(&conn, &rec);
                }
            }
            _ => {}
        }
    }

    Ok(format!(
        "Sincronización de partidas completada: {} descargadas de Anbernic, {} subidas desde PC.",
        downloaded, uploaded
    ))
}

/// Sincroniza ROMs desde el PC hacia la Anbernic
pub fn sync_roms_to_anbernic(
    db_path: &Path,
    local_base: &Path,
    ssh_config: &SshConfig,
) -> Result<String, String> {
    let conn = db::init_db(db_path).map_err(|e| format!("Error en base de datos: {e}"))?;
    let records = db::get_all_records(&conn, Some("roms"))
        .map_err(|e| format!("Error consultando registros: {e}"))?;

    let sess = net::create_session(ssh_config)?;
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Error abriendo canal SFTP: {e}"))?;

    let mut transferred = 0;

    for mut rec in records {
        if rec.status == SyncStatus::OnlyLocal || rec.status == SyncStatus::LocalNewer {
            let local_file = local_base.join(&rec.relative_path);
            let remote_file = format!("/userdata/{}", rec.relative_path);

            if let Ok(bytes) = net::upload_file(&sftp, &local_file, &remote_file) {
                transferred += 1;
                rec.status = SyncStatus::InSync;
                rec.remote_size = Some(bytes);
                let _ = db::upsert_file_record(&conn, &rec);
            }
        }
    }

    Ok(format!(
        "Sincronización de ROMs completada: {} juegos transferidos a la Anbernic.",
        transferred
    ))
}
