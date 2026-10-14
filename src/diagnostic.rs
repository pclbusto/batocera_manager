use crate::net::{self, DiskSpace};
use crate::systems::{format_size, SystemConfig};
use ssh2::{Session, Sftp};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WasteType {
    ZeroByteRom,
    PartialTransferPart,
}

impl WasteType {
    pub fn description(&self) -> &'static str {
        match self {
            WasteType::ZeroByteRom => "Archivo ROM de 0 bytes (corrupto/incompleto)",
            WasteType::PartialTransferPart => "Archivo temporal .part de transferencia interrumpida",
        }
    }
}

#[derive(Clone, Debug)]
pub struct WasteCandidate {
    pub relative_path: String,
    pub full_remote_path: String,
    pub size_bytes: u64,
    pub waste_type: WasteType,
    pub system_id: String,
}

#[derive(Clone, Debug)]
pub struct SystemUsage {
    pub system_id: String,
    pub system_name: String,
    pub remote_folder: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub zero_byte_count: usize,
    pub part_count: usize,
}

#[derive(Clone, Debug)]
pub struct DiagnosticReport {
    pub disk_space: Option<DiskSpace>,
    pub systems_usage: Vec<SystemUsage>,
    pub waste_candidates: Vec<WasteCandidate>,
    pub total_waste_bytes: u64,
    pub report_text: String,
}

/// Ejecuta un diagnóstico exhaustivo y NO destructivo de almacenamiento sobre los directorios ROM en la consola.
/// NUNCA modifica, crea ni elimina ningún archivo durante el análisis.
pub fn run_storage_diagnostic(
    sess: &Session,
    sftp: &Sftp,
    systems: &[SystemConfig],
) -> Result<DiagnosticReport, String> {
    // 1. Consultar espacio libre y total real en el sistema de archivos /userdata
    let disk_space = net::get_remote_disk_space(sess, "/userdata").ok();

    let mut systems_usage = Vec::new();
    let mut waste_candidates = Vec::new();
    let mut total_waste_bytes = 0;

    // 2. Examinar ÚNICAMENTE los directorios de ROM configurados
    for sys in systems {
        let remote_roms_dir = format!("/userdata/{}", sys.remote_folder);

        // Validación de seguridad estricta: sólo directorios bajo /userdata/roms/
        if !remote_roms_dir.starts_with("/userdata/roms/") || remote_roms_dir.contains("..") {
            continue;
        }

        let items = net::list_remote_files_recursive(sftp, &remote_roms_dir, "").unwrap_or_default();

        let mut sys_file_count = 0;
        let mut sys_total_bytes = 0;
        let mut sys_zero_bytes = 0;
        let mut sys_parts = 0;

        for item in items {
            sys_file_count += 1;
            sys_total_bytes += item.size;

            let full_path = format!("{remote_roms_dir}/{}", item.relative_path);
            let filename = Path::new(&item.relative_path)
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| item.relative_path.clone());

            // Detectar archivos temporales .part
            if filename.ends_with(".part") || filename.contains(".part.") {
                sys_parts += 1;
                total_waste_bytes += item.size;
                waste_candidates.push(WasteCandidate {
                    relative_path: item.relative_path.clone(),
                    full_remote_path: full_path,
                    size_bytes: item.size,
                    waste_type: WasteType::PartialTransferPart,
                    system_id: sys.id.clone(),
                });
            }
            // Detectar archivos de 0 bytes, distinguiendo legítimos de residuos
            else if item.size == 0 {
                // Distinción de archivos vacíos legítimos de sistema (.nomedia, .keep, .gitkeep)
                let is_legitimate_empty = filename.starts_with('.')
                    || filename.eq_ignore_ascii_case(".nomedia")
                    || filename.eq_ignore_ascii_case(".keep");

                if !is_legitimate_empty {
                    sys_zero_bytes += 1;
                    waste_candidates.push(WasteCandidate {
                        relative_path: item.relative_path.clone(),
                        full_remote_path: full_path,
                        size_bytes: 0,
                        waste_type: WasteType::ZeroByteRom,
                        system_id: sys.id.clone(),
                    });
                }
            }
        }

        systems_usage.push(SystemUsage {
            system_id: sys.id.clone(),
            system_name: sys.name.clone(),
            remote_folder: sys.remote_folder.clone(),
            file_count: sys_file_count,
            total_bytes: sys_total_bytes,
            zero_byte_count: sys_zero_bytes,
            part_count: sys_parts,
        });
    }

    // 3. Generar reporte estructurado
    let mut text = String::new();
    text.push_str("============================================================\n");
    text.push_str("          INFORME DE DIAGNÓSTICO DE ALMACENAMIENTO\n");
    text.push_str("============================================================\n\n");

    if let Some(ref space) = disk_space {
        text.push_str(&format!(
            "📊 Espacio en /userdata:\n   Total:       {}\n   Usado:       {}\n   Disponible:  {}\n\n",
            format_size(space.total_bytes),
            format_size(space.used_bytes),
            format_size(space.available_bytes)
        ));
    } else {
        text.push_str("⚠️ No se pudo obtener la información de capacidad del disco remoto.\n\n");
    }

    text.push_str("📁 Uso por Directorio de Sistema de ROMs:\n");
    for s in &systems_usage {
        text.push_str(&format!(
            "  • {:<28} ({}): {} archivos, {} ocupados",
            s.system_name, s.system_id, s.file_count, format_size(s.total_bytes)
        ));
        if s.zero_byte_count > 0 || s.part_count > 0 {
            text.push_str(&format!(
                " ➔ [ALERTAS: {} de 0 bytes, {} temporales .part]",
                s.zero_byte_count, s.part_count
            ));
        }
        text.push('\n');
    }

    text.push_str("\n🔍 Residuos Detectados (Candidatos a Limpieza):\n");
    if waste_candidates.is_empty() {
        text.push_str("  ✓ No se encontraron archivos de 0 bytes ni temporales .part huérfanos.\n");
    } else {
        for w in &waste_candidates {
            text.push_str(&format!(
                "  ❌ [{}] {} (Tamaño: {})\n     Ruta: {}\n",
                w.waste_type.description(),
                w.relative_path,
                format_size(w.size_bytes),
                w.full_remote_path
            ));
        }
        text.push_str(&format!(
            "\nTotal candidatos detectados: {} archivos (Espacio recuperable en temporales: {})\n",
            waste_candidates.len(),
            format_size(total_waste_bytes)
        ));
        text.push_str("🔒 Ningún archivo ha sido modificado ni eliminado. Este reporte es solo informativo.\n");
    }
    text.push_str("============================================================\n");

    Ok(DiagnosticReport {
        disk_space,
        systems_usage,
        waste_candidates,
        total_waste_bytes,
        report_text: text,
    })
}

/// Elimina de forma estrictamente segura los residuos confirmados por el usuario.
/// Requisitos de seguridad obligatorios:
/// 1. Las rutas deben estar bajo /userdata/roms/
/// 2. No se permiten '..'
/// 3. Jamás se tocan saves, bios ni configuraciones.
/// 4. El archivo debe ser de 0 bytes o terminar en .part. Si tiene contenido real de ROM, se rechaza.
pub fn clean_confirmed_waste(sftp: &Sftp, paths_to_delete: &[String]) -> Result<usize, String> {
    let mut deleted_count = 0;
    let mut errors = Vec::new();

    for remote_path in paths_to_delete {
        let p = Path::new(remote_path);

        // Seguridad 1: Debe comenzar con /userdata/roms/
        if !remote_path.starts_with("/userdata/roms/") {
            errors.push(format!("Rechazado por seguridad: '{remote_path}' no está en /userdata/roms/"));
            continue;
        }

        // Seguridad 2: No debe contener navegación relativa
        if remote_path.contains("..") {
            errors.push(format!("Rechazado por seguridad: ruta inválida '{remote_path}'"));
            continue;
        }

        // Seguridad 3: Prohibido tocar saves, bios o configuraciones
        if remote_path.starts_with("/userdata/saves")
            || remote_path.starts_with("/userdata/bios")
            || remote_path.starts_with("/userdata/system")
        {
            errors.push(format!("Prohibido: intento de eliminar archivo protegido en '{remote_path}'"));
            continue;
        }

        // Seguridad 4: Verificar en el servidor remoto que realmente sea 0 bytes o .part
        match sftp.stat(p) {
            Ok(stat) => {
                let size = stat.size.unwrap_or(0);
                let is_part = remote_path.ends_with(".part") || remote_path.contains(".part.");
                if size != 0 && !is_part {
                    errors.push(format!(
                        "Rechazado: '{remote_path}' no es de 0 bytes ({size} bytes) ni temporal .part"
                    ));
                    continue;
                }

                if let Err(e) = sftp.unlink(p) {
                    errors.push(format!("Error eliminando '{remote_path}': {e}"));
                } else {
                    deleted_count += 1;
                }
            }
            Err(e) => {
                errors.push(format!("No se pudo consultar '{remote_path}': {e}"));
            }
        }
    }

    if !errors.is_empty() {
        Err(format!(
            "Se eliminaron {deleted_count} residuo(s). Alertas/Errores: {}",
            errors.join("; ")
        ))
    } else {
        Ok(deleted_count)
    }
}
