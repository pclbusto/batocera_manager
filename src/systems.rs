use crate::db::{self, SyncFileRecord, SyncStatus};
use crate::hash;
use crate::net::{self, SshConfig};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub use crate::domain::system::SystemConfig;

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
    let mut list = vec![
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
    ];
    for sys in &mut list {
        sys.local_rom_count = count_files_in_dir(&sys.local_folder);
    }
    list
}

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

/// Actualiza y reconcilia el inventario de un sistema con la base de datos local SQLite.
/// GARANTÍA DE SEGURIDAD: Esta operación es estrictamente de solo lectura hacia la consola.
/// NUNCA transfiere, modifica, crea ni elimina archivos en el dispositivo remoto.
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

    // 1. Escanear inventario remoto en /userdata/roms/<id>
    let remote_roms_dir = format!("/userdata/{}", system.remote_folder);
    let remote_items = net::list_remote_files_recursive(&sftp, &remote_roms_dir, "")
        .map_err(|e| format!("Error al consultar inventario remoto de '{}': {e}", system.name))?;

    let mut remote_map = std::collections::HashMap::new();
    for item in &remote_items {
        remote_map.insert(item.relative_path.clone(), item.size);
    }

    // 2. Escanear biblioteca local
    let mut local_files = Vec::new();
    scan_local(&system.local_folder, &system.local_folder, &mut local_files);

    let mut local_map = std::collections::HashMap::new();
    for (rel_name, local_path, local_size) in &local_files {
        local_map.insert(rel_name.clone(), (local_path.clone(), *local_size));
    }

    // 3. Comparar inventarios y actualizar SQLite sin transferir ningún archivo
    let mut all_keys = std::collections::BTreeSet::new();
    for k in local_map.keys() {
        all_keys.insert(k.clone());
    }
    for k in remote_map.keys() {
        all_keys.insert(k.clone());
    }

    let mut in_sync_count = 0;
    let mut only_local_count = 0;
    let mut only_remote_count = 0;
    let mut conflict_count = 0;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    for rel_name in all_keys {
        let relative_db_path = format!("{}/{}", system.remote_folder, rel_name);
        let loc = local_map.get(&rel_name);
        let rem = remote_map.get(&rel_name);

        let (status, l_size, r_size) = match (loc, rem) {
            (Some((_, l_s)), Some(r_s)) => {
                if *l_s == *r_s {
                    in_sync_count += 1;
                    (SyncStatus::InSync, Some(*l_s), Some(*r_s))
                } else {
                    conflict_count += 1;
                    (SyncStatus::Conflict, Some(*l_s), Some(*r_s))
                }
            }
            (Some((_, l_s)), None) => {
                only_local_count += 1;
                (SyncStatus::OnlyLocal, Some(*l_s), None)
            }
            (None, Some(r_s)) => {
                only_remote_count += 1;
                (SyncStatus::OnlyRemote, None, Some(*r_s))
            }
            (None, None) => continue,
        };

        let rec = SyncFileRecord {
            relative_path: relative_db_path,
            category: "roms".to_string(),
            local_size: l_size,
            local_mtime: None,
            local_hash: None,
            remote_size: r_size,
            remote_mtime: None,
            remote_hash: None,
            status,
            last_sync: Some(now),
        };
        let _ = db::upsert_file_record(&conn, &rec);
    }

    Ok(format!(
        "[{}] Inventario actualizado: {} juegos en PC, {} en consola ({} sincronizados, {} solo en PC, {} solo en consola, {} con diferencias). 0 archivos transferidos.",
        system.name, local_files.len(), remote_items.len(), in_sync_count, only_local_count, only_remote_count, conflict_count
    ))
}

pub use crate::domain::game::{format_size, GameItem};


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
                    // CORRECCIÓN CRÍTICA: La selección inicial siempre debe estar vacía (false).
                    // Nunca pre-seleccionar automáticamente para evitar transferencias no deseadas.
                    selected: false,
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
    let selected_games: Vec<&GameItem> = games.iter().filter(|g| g.selected).collect();
    if selected_games.is_empty() {
        return Ok("No hay juegos seleccionados para transferir.".to_string());
    }

    // 1. Validar que ninguna ROM seleccionada tenga 0 bytes
    for game in &selected_games {
        if game.size_bytes == 0 {
            return Err(format!(
                "El juego '{}' tiene un tamaño de 0 bytes. Transferencia cancelada por seguridad.",
                game.filename
            ));
        }
    }

    let sess = net::create_session(ssh_config)?;
    let sftp = sess.sftp().map_err(|e| format!("Error SFTP: {e}"))?;
    let conn = db::init_db(db_path).ok();

    // 2. Comprobar capacidad remota disponible para el lote completo + margen de seguridad
    let total_bytes_needed: u64 = selected_games.iter().map(|g| g.size_bytes).sum();
    let space = net::get_remote_disk_space(&sess, "/userdata")?;
    let required_with_margin = total_bytes_needed.saturating_add(net::SAFETY_MARGIN_BYTES);

    if space.available_bytes < required_with_margin {
        return Err(format!(
            "Espacio insuficiente en la consola: se requieren {} (incluyendo {} de margen de seguridad), pero solo hay {} disponibles en /userdata. Operación cancelada.",
            format_size(required_with_margin),
            format_size(net::SAFETY_MARGIN_BYTES),
            format_size(space.available_bytes)
        ));
    }

    let mut copied = 0;
    let mut errors = Vec::new();

    for game in selected_games {
        // 3. Re-verificar espacio antes de cada archivo del lote
        if let Ok(current_space) = net::get_remote_disk_space(&sess, "/userdata") {
            if current_space.available_bytes < game.size_bytes.saturating_add(net::SAFETY_MARGIN_BYTES) {
                errors.push(format!(
                    "Lote detenido por falta de espacio para '{}': disponibles {}, requeridos {}",
                    game.filename,
                    format_size(current_space.available_bytes),
                    format_size(game.size_bytes.saturating_add(net::SAFETY_MARGIN_BYTES))
                ));
                break;
            }
        }

        let remote_dest = format!("/userdata/{}/{}", remote_folder, game.filename);

        // 4. Transferencia atómica transaccional (.part -> verificación -> rename)
        match net::upload_file_atomic(&sftp, Some(&sess), &game.path, &remote_dest, net::SAFETY_MARGIN_BYTES, true) {
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
            Err(e) => {
                let is_space = e.contains("Espacio insuficiente") || e.contains("No space left");
                errors.push(format!("{}: {e}", game.filename));
                if is_space {
                    errors.push("Lote detenido inmediatamente para proteger la tarjeta SD contra llenado.".to_string());
                    break;
                }
            }
        }
    }

    if !errors.is_empty() {
        Err(format!("Copiados {copied} juego(s). Errores / alertas: {}", errors.join("; ")))
    } else {
        Ok(format!("Se copiaron exitosamente {copied} juego(s) a la consola de forma verificada."))
    }
}
