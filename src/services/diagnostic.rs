use crate::diagnostic;
use crate::domain::diagnostic::DiagnosticReport;
use crate::domain::system::SystemConfig;
use crate::net::{self, SshConfig};

pub fn run_storage_diagnostic(
    ssh_config: &SshConfig,
    systems: &[SystemConfig],
) -> Result<DiagnosticReport, String> {
    let sess = net::create_session(ssh_config)?;
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Error abriendo SFTP: {e}"))?;
    diagnostic::run_storage_diagnostic(&sess, &sftp, systems)
}

pub fn clean_confirmed_waste(
    ssh_config: &SshConfig,
    paths: &[String],
) -> Result<usize, String> {
    let sess = net::create_session(ssh_config)?;
    let sftp = sess
        .sftp()
        .map_err(|e| format!("Error abriendo SFTP: {e}"))?;
    diagnostic::clean_confirmed_waste(&sftp, paths)
}
