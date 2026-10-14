use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// Calcula el hash BLAKE3 de un archivo local en fragmentos de 64 KB.
pub fn compute_file_hash(path: &Path) -> Result<String, io::Error> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0u8; 65536];

    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }

    Ok(hasher.finalize().to_hex().to_string())
}
