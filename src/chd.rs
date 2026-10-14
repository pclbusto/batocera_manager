use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Detecta si `chdman` está instalado y devuelve la cadena de versión.
pub fn detect_chdman() -> Option<String> {
    let output = Command::new("chdman")
        .output()
        .ok()?;

    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text.lines().next().unwrap_or("chdman");
    if first_line.contains("chdman") {
        Some(first_line.trim().to_string())
    } else {
        None
    }
}

/// Busca todos los archivos `.cue` dentro de un directorio (recursivo).
pub fn find_cue_files(dir: &Path) -> Vec<PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                results.extend(find_cue_files(&path));
            } else if let Some(ext) = path.extension() {
                if ext.to_string_lossy().eq_ignore_ascii_case("cue") {
                    results.push(path);
                }
            }
        }
    }
    results.sort();
    results
}

/// Lee un archivo .cue y extrae los nombres de los archivos .bin u otros asociados
pub fn parse_cue_referenced_files(cue_path: &Path) -> Vec<PathBuf> {
    let parent = cue_path.parent().unwrap_or_else(|| Path::new("."));
    let mut files = Vec::new();

    if let Ok(content) = fs::read_to_string(cue_path) {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.to_ascii_uppercase().starts_with("FILE") {
                // Formato habitual: FILE "archivo.bin" BINARY
                if let Some(start) = trimmed.find('"') {
                    if let Some(end) = trimmed[start + 1..].find('"') {
                        let filename = &trimmed[start + 1..start + 1 + end];
                        files.push(parent.join(filename));
                    }
                } else {
                    // Formato sin comillas: FILE archivo.bin BINARY
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 2 {
                        files.push(parent.join(parts[1]));
                    }
                }
            }
        }
    }

    files
}

/// Convierte un único archivo .cue a .chd usando `chdman createcd`
pub fn convert_cue_to_chd(
    cue_path: &Path,
    out_chd: &Path,
    force: bool,
    delete_source: bool,
) -> Result<String, String> {
    if !cue_path.exists() {
        return Err(format!("El archivo de origen no existe: {}", cue_path.display()));
    }

    // Asegurar directorio padre de salida
    if let Some(parent) = out_chd.parent() {
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }
    }

    let mut cmd = Command::new("chdman");
    cmd.arg("createcd")
        .arg("-i")
        .arg(cue_path)
        .arg("-o")
        .arg(out_chd);

    if force {
        cmd.arg("-f");
    }

    let output = cmd.output().map_err(|e| format!("Error al ejecutar chdman: {e}"))?;

    let stdout_str = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_str = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() {
        return Err(if !stderr_str.trim().is_empty() {
            stderr_str
        } else if !stdout_str.trim().is_empty() {
            stdout_str
        } else {
            format!("chdman falló con código de salida {:?}", output.status.code())
        });
    }

    // Si la conversión fue exitosa y se solicitó eliminar originales
    if delete_source {
        let referenced = parse_cue_referenced_files(cue_path);
        for file in referenced {
            if file.exists() {
                let _ = fs::remove_file(&file);
            }
        }
        if cue_path.exists() {
            let _ = fs::remove_file(cue_path);
        }
    }

    Ok(stdout_str)
}
