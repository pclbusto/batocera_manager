use std::path::{Path, PathBuf};

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

        Self {
            id: id.to_string(),
            name: name.to_string(),
            local_folder: folder,
            local_folder_str: folder_str,
            remote_folder: format!("roms/{id}"),
            enabled,
            local_rom_count: 0,
            remote_rom_count: 0,
            status_msg: "Sin escanear".to_string(),
        }
    }
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
