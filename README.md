# Batocera Manager 0.0.1

Aplicación de escritorio desarrollada en **Rust** utilizando el toolkit gráfico **libcosmic** (entorno de escritorio COSMIC de System76) diseñada para gestionar consolas portátiles **Anbernic** con **Batocera Linux** o **Knulli**.

---

## Diferencia fundamental: ¿Qué es "Sincronizar" vs. "Copiar"?

| Operación | ¿Qué hace? | ¿Cuándo usarla? |
| :--- | :--- | :--- |
| **🚀 Copiar Juegos Seleccionados** (Pestaña *ROMs y Juegos*) | Te muestra la lista de juegos de cada consola con casillas de verificación. **Tú eliges 1, 2 o los juegos que quieras** y el manager los transfiere directamente a la Anbernic (`/userdata/roms/<sistema>/`). | Cuando tienes juegos nuevos específicos y quieres pasarlos a la consola sin esperar ni tocar el resto. |
| **⚡ Sincronizar** (Pestaña *Sincronización por Sistemas*) | Proceso **automático** que compara tu PC contra la Anbernic: sube los juegos que falten y **actualiza tus partidas guardadas (`.srm` / `.state`)** descargando la partida más reciente al PC (con backup previo) si jugaste en la consola. **Nunca borra juegos**. | Cuando vuelves de jugar en la consola y quieres respaldar tu avance o igualar la colección entera de un sistema. |

---

## Características de la Aplicación

### 1. Explorador y Copiado Selectivo de Juegos (*ROMs y Juegos*)
- **Selector de Consola**: Elige qué sistema explorar (PlayStation, SNES, GBA, Mega Drive, NES, N64, Arcade, PSP, etc.).
- **Lista detallada con casillas (Checkboxes)**:
  - Título del archivo o juego.
  - Tamaño formateado en KB, MB o GB.
  - Indicador de estado:
    - 🟢 *Ya en la Anbernic* (no necesitas volver a copiarlo).
    - 📥 *Listo para copiar (Solo en tu PC)*.
- **Herramientas de selección**:
  - **Botón "Seleccionar los que faltan"**: Marca con un solo clic todos los juegos que aún no tienes en la consola.
  - **Buscador de juegos**: Filtra títulos en tiempo real.
  - **Botón "🚀 Copiar a la Anbernic (X seleccionados)"**: Transfiere los juegos marcados por Wi-Fi de forma desatendida.

---

### 2. Sincronización por Sistemas y Consolas (*Sincronización por Sistemas*)
- **Carpetas locales independientes**: Cada consola puede tener su propia carpeta asignada en el PC (ej. `/home/pedro/Batocera/roms/psx` o en discos externos).
- **Togglers de activación**: Decide qué consolas mantener sincronizadas y cuáles ignorar.
- **Sincronización individual**: Botón dedicado por consola (ej. *"Sincronizar psx"*).
- **Protección de partidas guardadas**: Respaldo automático con marca de tiempo en `~/.local/share/batocera_manager/backups/`.
- **Base de Datos SQLite (`sync.db`)**: Recuerda rutas, consolas activas y estados.

---

### 3. Conversor CHD integrado (*Conversor CHD*)
- Conversión individual de archivos `.cue` a `.chd` mediante `chdman createcd`.
- Conversión masiva en lote (Batch) para carpetas completas.
- Sobreescritura forzada opcional (`-f`) y eliminación segura de `.cue`/`.bin` tras la compresión.

---

### 4. Configuración Persistente (SQLite)
- **Persistencia automática**: Guarda en la base de datos SQLite (`sync.db`) la dirección IP de la consola, puerto SSH, usuario, contraseña y la ruta base local. Al reiniciar la aplicación, los valores se cargan inmediatamente.
- **Estrategia de formatos de ROM**:
  - **Sistemas de disco (PSX, Saturn, Dreamcast)**: Formato `.chd` directamente, sin doble compresión, maximizando compatibilidad y rendimiento de carga en Batocera.
  - **Sistemas de cartuchos (NES, SNES, Mega Drive, GBA)**: Soporte directo para archivos comprimidos `.7z`, `.zip` y extensiones nativas.

---

### 5. Entorno COSMIC Nativo
- Construido con la arquitectura Model-View-Update (MVU) de `libcosmic`.
- Menú lateral de navegación (`nav_bar::Model`) con iconos simbólicos.
- Diálogos nativos de selección de archivos y carpetas (`rfd`).
- Ejecución asíncrona en segundo plano con Tokio para mantener la fluidez de la interfaz.

---

## Ejecución

Para iniciar la aplicación:

```bash
cargo run
```
