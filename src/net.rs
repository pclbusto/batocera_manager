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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct DiskSpace {
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub available_bytes: u64,
}

pub const SAFETY_MARGIN_BYTES: u64 = 100 * 1024 * 1024; // 100 MB de margen de seguridad

pub fn get_remote_disk_space(sess: &Session, remote_path: &str) -> Result<DiskSpace, String> {
    let mut channel = sess
        .channel_session()
        .map_err(|e| format!("Error abriendo canal SSH para consultar espacio: {e}"))?;

    let cmd = format!("df -k -P \"{remote_path}\" 2>/dev/null || df -k \"{remote_path}\"");
    channel
        .exec(&cmd)
        .map_err(|e| format!("Error ejecutando comando df en la consola: {e}"))?;

    let mut output = String::new();
    channel
        .read_to_string(&mut output)
        .map_err(|e| format!("Error leyendo respuesta de df: {e}"))?;

    let _ = channel.wait_close();

    let lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
    if lines.len() < 2 {
        return Err(format!("Salida inesperada de df en la consola: '{output}'"));
    }

    let data_line = lines[1];
    let parts: Vec<&str> = data_line.split_whitespace().collect();
    if parts.len() < 4 {
        return Err(format!("Formato inesperado en línea de df: '{data_line}'"));
    }

    let total_kb = parts[1]
        .parse::<u64>()
        .map_err(|e| format!("Error parseando total en df ('{}'): {e}", parts[1]))?;
    let used_kb = parts[2]
        .parse::<u64>()
        .map_err(|e| format!("Error parseando usado en df ('{}'): {e}", parts[2]))?;
    let avail_kb = parts[3]
        .parse::<u64>()
        .map_err(|e| format!("Error parseando disponible en df ('{}'): {e}", parts[3]))?;

    Ok(DiskSpace {
        total_bytes: total_kb * 1024,
        used_bytes: used_kb * 1024,
        available_bytes: avail_kb * 1024,
    })
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
        Err(e) => {
            let io_err: std::io::Error = e.into();
            if io_err.kind() == std::io::ErrorKind::NotFound {
                // El directorio no existe en la consola
                return Ok(results);
            } else if io_err.kind() == std::io::ErrorKind::PermissionDenied {
                return Err(format!("Permiso denegado al leer directorio remoto '{target_dir}'"));
            } else if io_err.kind() == std::io::ErrorKind::TimedOut {
                return Err(format!("Tiempo de espera agotado al explorar directorio remoto '{target_dir}'"));
            } else {
                return Err(format!("Error SFTP al explorar directorio remoto '{target_dir}': {io_err}"));
            }
        }
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
            let sub_items = list_remote_files_recursive(sftp, remote_base_dir, &rel_path)?;
            results.extend(sub_items);
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
        .map_err(|e| format!("Error abriendo archivo remoto '{remote_path}': {e}"))?;

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

pub trait StorageBackend {
    fn check_disk_space(&self, path: &str, required_bytes: u64) -> Result<(), String>;
    fn file_stat(&self, path: &str) -> Result<Option<u64>, String>;
    fn create_temp(&self, temp_path: &str) -> Result<Box<dyn Write>, String>;
    fn verify_size(&self, path: &str, expected_size: u64) -> Result<(), String>;
    fn rename_atomic(&self, src: &str, dst: &str) -> Result<(), String>;
    fn unlink(&self, path: &str) -> Result<(), String>;
    fn make_parent_dirs(&self, path: &str) -> Result<(), String>;
}

pub struct SftpStorage<'a> {
    pub sftp: &'a ssh2::Sftp,
    pub sess: Option<&'a ssh2::Session>,
}

impl<'a> StorageBackend for SftpStorage<'a> {
    fn check_disk_space(&self, path: &str, required_bytes: u64) -> Result<(), String> {
        if let Some(s) = self.sess {
            let parent_remote = Path::new(path)
                .parent()
                .and_then(|p| p.to_str())
                .unwrap_or("/userdata");
            let space = get_remote_disk_space(s, parent_remote)?;
            if space.available_bytes < required_bytes {
                return Err(format!(
                    "Espacio insuficiente en disco remoto: disponibles {}, necesarios {}.",
                    crate::systems::format_size(space.available_bytes),
                    crate::systems::format_size(required_bytes)
                ));
            }
        }
        Ok(())
    }

    fn file_stat(&self, path: &str) -> Result<Option<u64>, String> {
        match self.sftp.stat(Path::new(path)) {
            Ok(stat) => Ok(Some(stat.size.unwrap_or(0))),
            Err(e) => {
                let io_err: std::io::Error = e.into();
                if io_err.kind() == std::io::ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(format!("Error consultando archivo '{path}': {io_err}"))
                }
            }
        }
    }

    fn create_temp(&self, temp_path: &str) -> Result<Box<dyn Write>, String> {
        let f = self.sftp
            .create(Path::new(temp_path))
            .map_err(|e| format!("Error creando archivo temporal remoto '{temp_path}': {e}"))?;
        Ok(Box::new(f))
    }

    fn verify_size(&self, path: &str, expected_size: u64) -> Result<(), String> {
        let stat = self.sftp
            .stat(Path::new(path))
            .map_err(|e| format!("No se pudo verificar el archivo temporal remoto: {e}"))?;
        let size = stat.size.unwrap_or(0);
        if size != expected_size {
            return Err(format!(
                "Verificación de tamaño remoto fallida: el archivo en consola tiene {size} bytes, pero el original tiene {expected_size} bytes."
            ));
        }
        Ok(())
    }

    fn rename_atomic(&self, src: &str, dst: &str) -> Result<(), String> {
        let _ = self.sftp.unlink(Path::new(dst));
        self.sftp
            .rename(Path::new(src), Path::new(dst), None)
            .map_err(|e| format!("Error al renombrar archivo temporal a definitivo: {e}"))
    }

    fn unlink(&self, path: &str) -> Result<(), String> {
        let _ = self.sftp.unlink(Path::new(path));
        Ok(())
    }

    fn make_parent_dirs(&self, path: &str) -> Result<(), String> {
        if let Some(parent) = Path::new(path).parent() {
            let parent_str = parent.to_string_lossy();
            if !parent_str.is_empty() {
                let mut current = String::new();
                for comp in parent_str.split('/') {
                    if comp.is_empty() {
                        continue;
                    }
                    current.push('/');
                    current.push_str(comp);
                    let _ = self.sftp.mkdir(Path::new(&current), 0o755);
                }
            }
        }
        Ok(())
    }
}

pub fn upload_transactional<S: StorageBackend>(
    storage: &S,
    local_path: &Path,
    remote_path: &str,
    safety_margin_bytes: u64,
    allow_overwrite: bool,
) -> Result<u64, String> {
    // 1. Validar que el archivo de origen existe y es un archivo regular
    if !local_path.exists() {
        return Err(format!("Archivo local no existe: {}", local_path.display()));
    }
    let metadata = fs::metadata(local_path)
        .map_err(|e| format!("No se pudo leer metadatos de '{}': {e}", local_path.display()))?;
    if !metadata.is_file() {
        return Err(format!("La ruta local '{}' no es un archivo regular", local_path.display()));
    }

    // 2. Obtener su tamaño y rechazar archivos de 0 bytes
    let local_size = metadata.len();
    if local_size == 0 {
        return Err(format!(
            "El archivo local '{}' tiene 0 bytes. Transferencia rechazada por seguridad.",
            local_path.display()
        ));
    }

    // 3. Comprobar espacio disponible en el sistema de archivos remoto
    let required = local_size.saturating_add(safety_margin_bytes);
    storage.check_disk_space(remote_path, required)?;

    // 4. Asegurar que los directorios padre remotos existan
    storage.make_parent_dirs(remote_path)?;

    // 5. Detectar posibles conflictos o archivos existentes
    if let Ok(Some(existing_size)) = storage.file_stat(remote_path)
        && !allow_overwrite
    {
        return Err(format!(
            "El archivo remoto '{remote_path}' ya existe (tamaño: {}). Sobrescritura no autorizada.",
            crate::systems::format_size(existing_size)
        ));
    }

    // 6. Crear un archivo temporal controlado: {remote_path}.part
    let part_path = format!("{remote_path}.part");

    // Limpiar residuos previos de .part para este archivo si existieran
    let _ = storage.unlink(&part_path);

    let transfer_res: Result<u64, String> = (|| {
        let mut local_file = File::open(local_path)
            .map_err(|e| format!("Error abriendo archivo local: {e}"))?;

        // 7. Transferir contenido al temporal
        let mut remote_part = storage.create_temp(&part_path)?;
        let mut buffer = [0u8; 65536];
        let mut total_bytes = 0;

        loop {
            let read = local_file
                .read(&mut buffer)
                .map_err(|e| format!("Error leyendo archivo local: {e}"))?;
            if read == 0 {
                break;
            }
            remote_part
                .write_all(&buffer[..read])
                .map_err(|e| format!("Error enviando datos por SFTP al temporal: {e}"))?;
            total_bytes += read as u64;
        }

        // 8. Cerrar correctamente el archivo remoto
        drop(remote_part);

        if total_bytes != local_size {
            return Err(format!(
                "Transferencia incompleta: se enviaron {total_bytes} bytes de {local_size} bytes."
            ));
        }

        // 9. Verificar el tamaño transferido contra el original consultando al servidor remoto
        storage.verify_size(&part_path, local_size)?;

        // 10 & 11. Publicar el archivo mediante renombrado atómico en el mismo sistema de archivos
        storage.rename_atomic(&part_path, remote_path)?;

        Ok(total_bytes)
    })();

    match transfer_res {
        Ok(bytes) => Ok(bytes),
        Err(err) => {
            // Limpiar solamente el temporal creado por esta operación
            let _ = storage.unlink(&part_path);
            Err(err)
        }
    }
}

pub fn upload_file_atomic(
    sftp: &ssh2::Sftp,
    sess: Option<&ssh2::Session>,
    local_path: &Path,
    remote_path: &str,
    safety_margin_bytes: u64,
    allow_overwrite: bool,
) -> Result<u64, String> {
    let storage = SftpStorage { sftp, sess };
    upload_transactional(&storage, local_path, remote_path, safety_margin_bytes, allow_overwrite)
}

#[allow(dead_code)]
pub fn upload_file(
    sftp: &ssh2::Sftp,
    local_path: &Path,
    remote_path: &str,
) -> Result<u64, String> {
    upload_file_atomic(sftp, None, local_path, remote_path, SAFETY_MARGIN_BYTES, true)
}

#[allow(dead_code)]
pub mod mock {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::collections::HashMap;
    use std::io::{self, Write};

    #[derive(Clone, Default)]
    pub struct MockStorage {
        pub files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        pub available_space_bytes: Arc<Mutex<Option<u64>>>,
        pub fail_write_at_bytes: Arc<Mutex<Option<usize>>>,
        pub unlink_log: Arc<Mutex<Vec<String>>>,
        pub rename_log: Arc<Mutex<Vec<(String, String)>>>,
        pub created_temp_log: Arc<Mutex<Vec<String>>>,
    }

    struct MockWriter {
        path: String,
        files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        fail_at_bytes: Option<usize>,
        written_so_far: usize,
    }

    impl Write for MockWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if let Some(limit) = self.fail_at_bytes {
                if self.written_so_far + buf.len() > limit {
                    return Err(io::Error::new(io::ErrorKind::ConnectionReset, "Simulated network drop"));
                }
            }
            let mut map = self.files.lock().unwrap();
            let entry = map.entry(self.path.clone()).or_default();
            entry.extend_from_slice(buf);
            self.written_so_far += buf.len();
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl StorageBackend for MockStorage {
        fn check_disk_space(&self, _path: &str, required_bytes: u64) -> Result<(), String> {
            if let Some(avail) = *self.available_space_bytes.lock().unwrap() {
                if avail < required_bytes {
                    return Err(format!("Espacio insuficiente en disco remoto: disponibles {avail}, necesarios {required_bytes}."));
                }
            }
            Ok(())
        }

        fn file_stat(&self, path: &str) -> Result<Option<u64>, String> {
            let files = self.files.lock().unwrap();
            Ok(files.get(path).map(|f| f.len() as u64))
        }

        fn create_temp(&self, temp_path: &str) -> Result<Box<dyn Write>, String> {
            self.created_temp_log.lock().unwrap().push(temp_path.to_string());
            self.files.lock().unwrap().insert(temp_path.to_string(), Vec::new());
            Ok(Box::new(MockWriter {
                path: temp_path.to_string(),
                files: Arc::clone(&self.files),
                fail_at_bytes: *self.fail_write_at_bytes.lock().unwrap(),
                written_so_far: 0,
            }))
        }

        fn verify_size(&self, path: &str, expected_size: u64) -> Result<(), String> {
            let files = self.files.lock().unwrap();
            match files.get(path) {
                Some(content) => {
                    let actual = content.len() as u64;
                    if actual == expected_size {
                        Ok(())
                    } else {
                        Err(format!("Verificación de tamaño remoto fallida: {actual} vs {expected_size}"))
                    }
                }
                None => Err(format!("Archivo temporal '{path}' no existe")),
            }
        }

        fn rename_atomic(&self, src: &str, dst: &str) -> Result<(), String> {
            self.rename_log.lock().unwrap().push((src.to_string(), dst.to_string()));
            let mut files = self.files.lock().unwrap();
            if let Some(content) = files.remove(src) {
                files.insert(dst.to_string(), content);
                Ok(())
            } else {
                Err(format!("No se encontró temporal '{src}' para renombrar"))
            }
        }

        fn unlink(&self, path: &str) -> Result<(), String> {
            self.unlink_log.lock().unwrap().push(path.to_string());
            self.files.lock().unwrap().remove(path);
            Ok(())
        }

        fn make_parent_dirs(&self, _path: &str) -> Result<(), String> {
            Ok(())
        }
    }
}
