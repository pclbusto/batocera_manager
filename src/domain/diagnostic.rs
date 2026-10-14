use crate::net::DiskSpace;
use super::system::SystemUsage;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WasteCandidate {
    pub relative_path: String,
    pub full_remote_path: String,
    pub size_bytes: u64,
    pub mtime: u64,
    pub waste_type: WasteType,
    pub system_id: String,
    pub selected: bool,
}

#[derive(Clone, Debug)]
pub struct DiagnosticReport {
    pub disk_space: Option<DiskSpace>,
    pub systems_usage: Vec<SystemUsage>,
    pub waste_candidates: Vec<WasteCandidate>,
    pub total_waste_bytes: u64,
    pub report_text: String,
}
