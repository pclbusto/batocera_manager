use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameItem {
    pub filename: String,
    pub path: PathBuf,
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
