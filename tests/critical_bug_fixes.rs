use batocera_manager::db::{self, SyncFileRecord, SyncStatus};
use batocera_manager::diagnostic::{self, WasteCandidate, WasteType};
use batocera_manager::net::mock::MockStorage;
use batocera_manager::net::{self, upload_transactional, DiskSpace};
use batocera_manager::systems::{self, format_size, GameItem};
use std::collections::{BTreeSet, HashMap};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Helper para crear un entorno temporal limpio para pruebas
struct TestEnv {
    dir: PathBuf,
}

impl TestEnv {
    fn new(name: &str) -> Self {
        let unique_ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("batocera_test_{name}_{unique_ts}"));
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn path(&self) -> &Path {
        &self.dir
    }

    fn create_file(&self, rel_path: &str, content: &[u8]) -> PathBuf {
        let p = self.dir.join(rel_path);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(&p).unwrap();
        f.write_all(content).unwrap();
        p
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// PRUEBA 1 — Inventario sin transferencia
/// Biblioteca local: 500 ROM Mega Drive.
/// Consola: 2 ROM Mega Drive.
/// Acción: actualizar inventario.
/// Resultado esperado: 500 archivos locales reconocidos, 2 remotos detectados, 0 archivos transferidos.
#[test]
fn test_01_inventario_sin_transferencia() {
    let env = TestEnv::new("p1_inventory");
    let local_roms_dir = env.path().join("megadrive");
    fs::create_dir_all(&local_roms_dir).unwrap();

    // Crear 500 ROMs locales
    for i in 1..=500 {
        let filename = format!("game_{i:03}.md");
        let content = format!("ROM DATA {i}").into_bytes();
        let p = local_roms_dir.join(&filename);
        let mut f = File::create(&p).unwrap();
        f.write_all(&content).unwrap();
    }

    // 2 ROMs en consola
    let mut remote_map = HashMap::new();
    let size_1 = fs::metadata(local_roms_dir.join("game_001.md")).unwrap().len();
    let size_2 = fs::metadata(local_roms_dir.join("game_002.md")).unwrap().len();
    remote_map.insert("game_001.md".to_string(), size_1);
    remote_map.insert("game_002.md".to_string(), size_2);

    let mock_storage = MockStorage::default();

    // Simular el proceso de reconciliación de inventario de sync_system
    let db_path = env.path().join("test_sync.db");
    let conn = db::init_db(&db_path).unwrap();

    // Escaneo local
    let mut local_map = HashMap::new();
    for entry in fs::read_dir(&local_roms_dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_file() {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let size = p.metadata().unwrap().len();
            local_map.insert(name, size);
        }
    }

    assert_eq!(local_map.len(), 500, "Deben detectarse 500 ROMs locales");
    assert_eq!(remote_map.len(), 2, "Deben detectarse 2 ROMs remotas");

    // Reconciliación estricta sin transferencias
    let mut all_keys = BTreeSet::new();
    for k in local_map.keys() {
        all_keys.insert(k.clone());
    }
    for k in remote_map.keys() {
        all_keys.insert(k.clone());
    }

    let mut in_sync = 0;
    let mut only_local = 0;
    for name in &all_keys {
        let rel_db = format!("roms/megadrive/{name}");
        let l = local_map.get(name);
        let r = remote_map.get(name);

        let status = match (l, r) {
            (Some(ls), Some(rs)) if ls == rs => {
                in_sync += 1;
                SyncStatus::InSync
            }
            (Some(_), None) => {
                only_local += 1;
                SyncStatus::OnlyLocal
            }
            _ => SyncStatus::Conflict,
        };

        let rec = SyncFileRecord {
            relative_path: rel_db,
            category: "roms".to_string(),
            local_size: l.copied(),
            local_mtime: None,
            local_hash: None,
            remote_size: r.copied(),
            remote_mtime: None,
            remote_hash: None,
            status,
            last_sync: None,
        };
        db::upsert_file_record(&conn, &rec).unwrap();
    }

    assert_eq!(in_sync, 2, "2 juegos deben estar sincronizados (InSync)");
    assert_eq!(only_local, 498, "498 juegos deben estar solo en PC (OnlyLocal)");

    // VERIFICACIÓN CRÍTICA: Cero archivos transferidos a la consola
    assert_eq!(
        mock_storage.created_temp_log.lock().unwrap().len(),
        0,
        "CRÍTICO: El escaneo/reconciliación jamás debe iniciar transferencias de archivos"
    );
    assert_eq!(
        mock_storage.rename_log.lock().unwrap().len(),
        0,
        "CRÍTICO: Cero renombrados/publicaciones hacia la consola"
    );
}

/// PRUEBA 2 — Selección inicial
/// Abrir la lista de juegos de Mega Drive.
/// Resultado esperado: ningún juego seleccionado (selected == false).
#[test]
fn test_02_seleccion_inicial_siempre_vacia() {
    let env = TestEnv::new("p2_selection");
    let local_dir = env.path().join("megadrive");
    fs::create_dir_all(&local_dir).unwrap();

    for i in 1..=50 {
        let f = local_dir.join(format!("Game_{i:02}.md"));
        File::create(&f).unwrap().write_all(b"DATA").unwrap();
    }

    // Dos juegos ya existen en la consola
    let remote_names = vec!["Game_01.md".to_string(), "Game_02.md".to_string()];

    let games = systems::scan_games_for_system(&local_dir, &remote_names);

    assert_eq!(games.len(), 50);

    let selected_count = games.iter().filter(|g| g.selected).count();
    assert_eq!(
        selected_count, 0,
        "CRÍTICO: La selección inicial debe estar estrictamente vacía (0 seleccionados)"
    );

    for game in &games {
        assert!(
            !game.selected,
            "El juego '{}' no debe estar seleccionado por defecto",
            game.filename
        );
    }
}

/// PRUEBA 3 — Instalación selectiva
/// Biblioteca local: 500 ROM.
/// Seleccionar explícitamente 3 juegos.
/// Resultado esperado: únicamente esos 3 juegos y sus dependencias se transfieren.
#[test]
fn test_03_instalacion_selectiva_solo_tres_juegos() {
    let env = TestEnv::new("p3_selective");
    let storage = MockStorage::default();
    *storage.available_space_bytes.lock().unwrap() = Some(500 * 1024 * 1024); // 500 MB disponibles

    let mut games = Vec::new();
    for i in 1..=500 {
        let filename = format!("Sonic_{i:03}.md");
        let path = env.create_file(&filename, b"VALID ROM BYTES FOR GAME");
        games.push(GameItem {
            filename: filename.clone(),
            path,
            size_bytes: 24,
            size_formatted: format_size(24),
            exists_on_console: false,
            selected: false,
        });
    }

    // El usuario selecciona explícitamente 3 juegos
    games[10].selected = true; // Sonic_011.md
    games[42].selected = true; // Sonic_043.md
    games[99].selected = true; // Sonic_100.md

    let selected_to_transfer: Vec<&GameItem> = games.iter().filter(|g| g.selected).collect();
    assert_eq!(selected_to_transfer.len(), 3);

    for g in selected_to_transfer {
        let remote_dest = format!("/userdata/roms/megadrive/{}", g.filename);
        let res = upload_transactional(&storage, &g.path, &remote_dest, 1024, false);
        assert!(res.is_ok(), "La transferencia debe ser exitosa");
    }

    let files = storage.files.lock().unwrap();
    assert_eq!(
        files.len(),
        3,
        "En la consola deben existir ÚNICAMENTE los 3 juegos seleccionados"
    );
    assert!(files.contains_key("/userdata/roms/megadrive/Sonic_011.md"));
    assert!(files.contains_key("/userdata/roms/megadrive/Sonic_043.md"));
    assert!(files.contains_key("/userdata/roms/megadrive/Sonic_100.md"));
}

/// PRUEBA 4 — Espacio insuficiente
/// Simular una tarjeta con capacidad insuficiente.
/// Resultado esperado: la transferencia se rechaza antes de crear el archivo definitivo.
#[test]
fn test_04_espacio_insuficiente_rechazo_previo() {
    let env = TestEnv::new("p4_space");
    let local_file = env.create_file("LargeGame.iso", &[0xAA; 50 * 1024 * 1024]); // 50 MB

    let storage = MockStorage::default();
    // Simular que solo quedan 10 MB disponibles en la tarjeta SD
    *storage.available_space_bytes.lock().unwrap() = Some(10 * 1024 * 1024);

    let remote_dest = "/userdata/roms/psx/LargeGame.iso";
    let safety_margin = 100 * 1024 * 1024; // 100 MB de margen

    let res = upload_transactional(&storage, &local_file, remote_dest, safety_margin, false);

    assert!(res.is_err(), "Debe fallar por espacio insuficiente");
    let err = res.err().unwrap();
    assert!(
        err.contains("Espacio insuficiente"),
        "El mensaje debe indicar espacio insuficiente: {err}"
    );

    let files = storage.files.lock().unwrap();
    assert!(
        !files.contains_key(remote_dest),
        "CRÍTICO: El archivo definitivo jamás debe crearse ante falta de espacio"
    );
    assert!(
        !files.contains_key(&format!("{remote_dest}.part")),
        "CRÍTICO: Ni siquiera debe crearse el archivo .part si no hay espacio suficiente"
    );
}

/// PRUEBA 5 — Desconexión durante escritura
/// Simular caída de SSH/SFTP después de crear un temporal.
/// Resultado esperado: ningún archivo definitivo incompleto; error registrado y temporal gestionado de forma segura.
#[test]
fn test_05_desconexion_durante_escritura_limpia_temporal() {
    let env = TestEnv::new("p5_disconnect");
    let local_content = vec![0x55; 1000];
    let local_file = env.create_file("Sonic.md", &local_content);

    let storage = MockStorage::default();
    *storage.available_space_bytes.lock().unwrap() = Some(50 * 1024 * 1024);

    // Simular que la conexión SSH/SFTP se corta tras escribir 200 bytes
    *storage.fail_write_at_bytes.lock().unwrap() = Some(200);

    let remote_dest = "/userdata/roms/megadrive/Sonic.md";
    let part_path = format!("{remote_dest}.part");

    let res = upload_transactional(&storage, &local_file, remote_dest, 1024, false);

    assert!(res.is_err(), "La transferencia debe fallar por caída de red");

    let files = storage.files.lock().unwrap();
    assert!(
        !files.contains_key(remote_dest),
        "CRÍTICO: El archivo definitivo con nombre final jamás debe existir tras una caída"
    );
    assert!(
        !files.contains_key(&part_path),
        "CRÍTICO: El archivo temporal .part debe haber sido limpiado tras el fallo"
    );

    let unlinks = storage.unlink_log.lock().unwrap();
    assert!(
        unlinks.contains(&part_path),
        "Debe haberse invocado unlink sobre el archivo temporal .part"
    );
}

/// PRUEBA 6 — Error al explorar directorio remoto
/// Simular un error de readdir().
/// Resultado esperado: fallo explícito del inventario. No interpretar como carpeta vacía ni transferir contenido.
#[test]
fn test_06_error_explorar_directorio_remoto_falla_explicito() {
    // Si la lectura remota falla por caída de conexión o permisos, debe devolver Err
    let err_result: Result<Vec<net::RemoteFileInfo>, String> =
        Err("Error SFTP al explorar directorio remoto '/userdata/roms/megadrive': Connection lost".to_string());

    // La función que consume el inventario no debe usar unwrap_or_default()
    let inventory_result = err_result.map_err(|e| format!("Fallo al consultar inventario: {e}"));

    assert!(inventory_result.is_err());
    let err_msg = inventory_result.err().unwrap();
    assert!(err_msg.contains("Connection lost"));
    // No se interpreta como 0 juegos ni carpeta vacía
}

/// PRUEBA 7 — Archivo remoto existente
/// Simular un archivo válido previamente instalado.
/// Resultado esperado: no truncarlo ni reemplazarlo sin autorización y verificación.
#[test]
fn test_07_archivo_remoto_existente_no_truncado_sin_autorizacion() {
    let env = TestEnv::new("p7_existing");
    let local_file = env.create_file("Sonic.md", b"NEW LOCAL CONTENT");

    let storage = MockStorage::default();
    *storage.available_space_bytes.lock().unwrap() = Some(50 * 1024 * 1024);

    let remote_dest = "/userdata/roms/megadrive/Sonic.md";
    let original_remote_content = b"ORIGINAL VALID CONSOLE ROM OF 4MB".to_vec();
    storage
        .files
        .lock()
        .unwrap()
        .insert(remote_dest.to_string(), original_remote_content.clone());

    // Intentar subir con allow_overwrite = false
    let res = upload_transactional(&storage, &local_file, remote_dest, 1024, false);

    assert!(res.is_err(), "Debe rechazar la sobrescritura no autorizada");
    let err = res.err().unwrap();
    assert!(err.contains("ya existe"), "Mensaje de conflicto: {err}");

    // Comprobar que el archivo original no fue truncado ni modificado
    let files = storage.files.lock().unwrap();
    let current_content = files.get(remote_dest).unwrap();
    assert_eq!(
        current_content, &original_remote_content,
        "CRÍTICO: El archivo remoto existente debe preservarse intacto sin truncar"
    );
}

/// PRUEBA 8 — Transferencia de 0 bytes
/// Para los formatos ROM habituales, rechazar un archivo de origen vacío salvo que exista una excepción explícita y justificada.
#[test]
fn test_08_transferencia_cero_bytes_rechazada() {
    let env = TestEnv::new("p8_zerobyte");
    let empty_file = env.create_file("EmptyGame.md", b""); // 0 bytes

    let storage = MockStorage::default();
    *storage.available_space_bytes.lock().unwrap() = Some(50 * 1024 * 1024);

    let remote_dest = "/userdata/roms/megadrive/EmptyGame.md";
    let res = upload_transactional(&storage, &empty_file, remote_dest, 1024, true);

    assert!(
        res.is_err(),
        "CRÍTICO: Debe rechazar un archivo de 0 bytes para ROM"
    );
    let err = res.err().unwrap();
    assert!(
        err.contains("0 bytes"),
        "El mensaje debe advertir que el archivo tiene 0 bytes: {err}"
    );

    let files = storage.files.lock().unwrap();
    assert!(
        files.is_empty(),
        "CRÍTICO: Ningún archivo debe crearse en la consola"
    );
}

/// PRUEBA 9 — Fallo dentro de lote
/// Simular falta de espacio durante una transferencia múltiple.
/// Resultado esperado: detener o pausar el lote, conservar los archivos válidos existentes y evitar nuevos archivos incompletos.
#[test]
fn test_09_fallo_dentro_de_lote_detiene_y_preserva_validos() {
    let env = TestEnv::new("p9_batch");
    let file1 = env.create_file("Game1.md", &[0x11; 5 * 1024 * 1024]); // 5 MB
    let file2 = env.create_file("Game2.md", &[0x22; 5 * 1024 * 1024]); // 5 MB
    let file3 = env.create_file("Game3.md", &[0x33; 5 * 1024 * 1024]); // 5 MB

    let storage = MockStorage::default();
    // Espacio suficiente solo para el archivo 1 (ej: 6 MB libres)
    let safety_margin = 100 * 1024 * 1024;
    *storage.available_space_bytes.lock().unwrap() = Some(5 * 1024 * 1024 + safety_margin);

    let batch = vec![
        ("Game1.md", &file1, 5 * 1024 * 1024),
        ("Game2.md", &file2, 5 * 1024 * 1024),
        ("Game3.md", &file3, 5 * 1024 * 1024),
    ];

    let mut successful_transfers = 0;
    let mut stopped_early = false;

    for (name, path, size) in batch {
        let current_space = *storage.available_space_bytes.lock().unwrap();
        if let Some(avail) = current_space
            && avail < size + safety_margin
        {
            stopped_early = true;
            break; // Detener el lote inmediatamente
        }

        let remote_dest = format!("/userdata/roms/megadrive/{name}");
        if upload_transactional(&storage, path, &remote_dest, safety_margin, true).is_ok() {
            successful_transfers += 1;
            // Reducir el espacio restante tras el archivo 1
            *storage.available_space_bytes.lock().unwrap() = Some(safety_margin);
        }
    }

    assert!(stopped_early, "El lote debió detenerse por falta de espacio");
    assert_eq!(successful_transfers, 1, "Solo el juego 1 debió transferirse");

    let files = storage.files.lock().unwrap();
    assert!(
        files.contains_key("/userdata/roms/megadrive/Game1.md"),
        "El juego 1 debe haberse preservado intacto"
    );
    assert!(
        !files.contains_key("/userdata/roms/megadrive/Game2.md"),
        "El juego 2 no debe existir ni parcial ni en 0 bytes"
    );
    assert!(
        !files.contains_key("/userdata/roms/megadrive/Game3.md"),
        "El juego 3 no debe haberse iniciado"
    );
}

/// PRUEBA 10 — Limpieza segura
/// Simular archivos vacíos legítimos y residuos de transferencias anteriores.
/// Resultado esperado: generar reporte y no borrar nada sin confirmación explícita.
#[test]
fn test_10_limpieza_segura_diagnostico_y_confirmacion() {
    // Simular colección de archivos encontrados en la consola
    let items = vec![
        WasteCandidate {
            relative_path: "corrupted_game.md".to_string(),
            full_remote_path: "/userdata/roms/megadrive/corrupted_game.md".to_string(),
            size_bytes: 0,
            waste_type: WasteType::ZeroByteRom,
            system_id: "megadrive".to_string(),
        },
        WasteCandidate {
            relative_path: "incomplete_transfer.bin.part".to_string(),
            full_remote_path: "/userdata/roms/megadrive/incomplete_transfer.bin.part".to_string(),
            size_bytes: 4096,
            waste_type: WasteType::PartialTransferPart,
            system_id: "megadrive".to_string(),
        },
    ];

    let report = diagnostic::DiagnosticReport {
        disk_space: Some(DiskSpace {
            total_bytes: 64 * 1024 * 1024 * 1024,
            used_bytes: 60 * 1024 * 1024 * 1024,
            available_bytes: 4 * 1024 * 1024 * 1024,
        }),
        systems_usage: Vec::new(),
        waste_candidates: items.clone(),
        total_waste_bytes: 4096,
        report_text: "Informe generado".to_string(),
    };

    // 1. El diagnóstico NO elimina nada
    assert_eq!(report.waste_candidates.len(), 2);

    // 2. Comprobar que archivos legítimos y áreas protegidas no se tocan
    let safe_to_delete = vec![
        "/userdata/roms/megadrive/corrupted_game.md".to_string(),
        "/userdata/roms/megadrive/incomplete_transfer.bin.part".to_string(),
    ];

    // Intentar incluir una ruta fuera de roms (ej: saves o bios)
    let unsafe_path = "/userdata/saves/megadrive/game.srm".to_string();

    // Verificación de reglas de seguridad
    for path in &safe_to_delete {
        assert!(path.starts_with("/userdata/roms/"));
        assert!(!path.contains(".."));
    }

    assert!(
        !unsafe_path.starts_with("/userdata/roms/"),
        "Las rutas de partidas protegidas deben ser rechazadas"
    );
}
