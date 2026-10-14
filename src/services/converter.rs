use crate::chd;
use std::path::{Path, PathBuf};

pub fn detect_chdman() -> Option<String> {
    chd::detect_chdman()
}

pub fn find_cue_files(dir: &Path) -> Vec<PathBuf> {
    chd::find_cue_files(dir)
}

pub fn convert_single(
    cue: &Path,
    out: &Path,
    force: bool,
    delete_src: bool,
) -> Result<String, String> {
    chd::convert_cue_to_chd(cue, out, force, delete_src)
}

pub fn convert_batch(
    cues: &[PathBuf],
    force: bool,
    delete_src: bool,
) -> Vec<(String, Result<String, String>)> {
    let mut results = Vec::new();
    for cue in cues {
        let mut out = cue.clone();
        out.set_extension("chd");
        let name = cue
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let res = chd::convert_cue_to_chd(cue, &out, force, delete_src);
        results.push((name, res));
    }
    results
}
