use ssh2::Session;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: "batocera.local".to_string(),
            port: 22,
            user: "root".to_string(),
            password: "linux".to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct RemoteFileInfo {
    pub relative_path: String,
    pub size: u64,
    pub mtime: u64,
}

pub fn create_session(config: &SshConfig) -> Result<Session, String> {
    let addr_str = format!("{}:{}", config.host, config.port);
    let socket_addrs: Vec<SocketAddr> = addr_str
        .to_socket_addrs()
        .map_err(|e| format!("No se pudo resolver el host '{}': {e}", config.host))?
        .collect();

    if socket_addrs.is_empty() {
        return Err(format!("Host no accesible: {}", config.host));
    }

    let tcp = TcpStream::connect_timeout(&socket_addrs[0], Duration::from_secs(4))
        .map_err(|e| format!("Conexión rechazada a '{}': {e}", config.host))?;

    let mut sess = Session::new().map_err(|e| format!("Error creando sesión SSH: {e}"))?;
    sess.set_tcp_stream(tcp);
    sess.handshake()
        .map_err(|e| format!("Fallo de negociación SSH: {e}"))?;

    sess.userauth_password(&config.user, &config.password)
        .map_err(|e| format!("Fallo de autenticación SSH para usuario '{}': {e}", config.user))?;

    if !sess.authenticated() {
        return Err("Credenciales incorrectas (usuario o contraseña inválidos)".to_string());
    }

    Ok(sess)
}

pub fn test_connection(config: &SshConfig) -> Result<String, String> {
    let sess = create_session(config)?;
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Error abriendo canal SSH: {e}"))?;

    channel
        .exec("cat /etc/batocera-version 2>/dev/null || uname -srm")
        .map_err(|e| format!("Error ejecutando comando de diagnóstico: {e}"))?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(|e| format!("Error leyendo respuesta de Batocera: {e}"))?;

    let _ = channel.wait_close();

    let info = output.trim();
    if info.is_empty() {
        Ok("Conexión exitosa a Batocera".to_string())
    } else {
        Ok(format!("Conectado a Batocera: {info}"))
    }
}

pub fn list_remote_files_recursive(
    sftp: &ssh2::Sftp,
    remote_base_dir: &str,
    sub_folder: &str,
) -> Result<Vec<RemoteFileInfo>, String> {
    let mut results = Vec::new();
    let target_dir = if sub_folder.is_empty() {
        remote_base_dir.to_string()
    } else {
        format!("{remote_base_dir}/{sub_folder}")
    };

    let dir_path = Path::new(&target_dir);
    let entries = match sftp.readdir(dir_path) {
        Ok(entries) => entries,
        Err(_) => return Ok(results),
    };

    for (path, stat) in entries {
        let filename = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        if filename == "." || filename == ".." {
            continue;
        }

        let rel_path = if sub_folder.is_empty() {
            filename.clone()
        } else {
            format!("{sub_folder}/{filename}")
        };

        if stat.is_dir() {
            if let Ok(sub_items) = list_remote_files_recursive(sftp, remote_base_dir, &rel_path) {
                results.extend(sub_items);
            }
        } else if stat.is_file() {
            results.push(RemoteFileInfo {
                relative_path: rel_path,
                size: stat.size.unwrap_or(0),
                mtime: stat.mtime.unwrap_or(0),
            });
        }
    }

    Ok(results)
}

pub fn download_file(
    sftp: &ssh2::Sftp,
    remote_path: &str,
    local_path: &Path,
) -> Result<u64, String> {
    if let Some(parent) = local_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut remote_file = sftp
        .open(Path::new(remote_path))
        .map_err(|e| format!("Error abriendo archivo remoto '{}': {e}", remote_path))?;

    let mut local_file = File::create(local_path)
        .map_err(|e| format!("Error creando archivo local '{}': {e}", local_path.display()))?;

    let mut buffer = [0u8; 65536];
    let mut total_bytes = 0;

    loop {
        let read = remote_file
            .read(&mut buffer)
            .map_err(|e| format!("Error leyendo datos remotos: {e}"))?;
        if read == 0 {
            break;
        }
        local_file
            .write_all(&buffer[..read])
            .map_err(|e| format!("Error escribiendo datos locales: {e}"))?;
        total_bytes += read as u64;
    }

    Ok(total_bytes)
}

pub fn upload_file(
    sftp: &ssh2::Sftp,
    local_path: &Path,
    remote_path: &str,
) -> Result<u64, String> {
    if !local_path.exists() {
        return Err(format!("Archivo local no existe: {}", local_path.display()));
    }

    // Asegurar directorio remoto
    if let Some(parent) = Path::new(remote_path).parent() {
        let parent_str = parent.to_string_lossy();
        if !parent_str.is_empty() {
            let mut current = String::new();
            for comp in parent_str.split('/') {
                if comp.is_empty() {
                    continue;
                }
                current.push('/');
                current.push_str(comp);
                let _ = sftp.mkdir(Path::new(&current), 0o755);
            }
        }
    }

    let mut local_file = File::open(local_path)
        .map_err(|e| format!("Error abriendo archivo local: {e}"))?;

    let mut remote_file = sftp
        .create(Path::new(remote_path))
        .map_err(|e| format!("Error creando archivo remoto '{}': {e}", remote_path))?;

    let mut buffer = [0u8; 65536];
    let mut total_bytes = 0;

    loop {
        let read = local_file
            .read(&mut buffer)
            .map_err(|e| format!("Error leyendo archivo local: {e}"))?;
        if read == 0 {
            break;
        }
        remote_file
            .write_all(&buffer[..read])
            .map_err(|e| format!("Error enviando datos por SFTP: {e}"))?;
        total_bytes += read as u64;
    }

    Ok(total_bytes)
}
