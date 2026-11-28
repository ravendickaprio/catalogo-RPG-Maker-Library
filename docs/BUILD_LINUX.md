# Compilar y ejecutar en Linux (Nobara / Fedora)

> **Estado:** la versión Linux **no se ha compilado ni probado todavía** (la máquina de desarrollo es Windows y no hay WSL). Esta guía se basa en los requisitos oficiales de Tauri 2 y en el código; todo lo marcado **(?)** debe verificarse en el primer intento real. El flujo `.github/workflows/build.yml` compila en Ubuntu y Fedora para detectar problemas antes.

## 1. Requisitos

### Paquetes del sistema (Fedora 40+ / Nobara)
Según los requisitos de Tauri 2 para Fedora:
```bash
sudo dnf check-update
sudo dnf install webkit2gtk4.1-devel \
  openssl-devel \
  curl \
  wget \
  file \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  libxdo-devel
sudo dnf group install "c-development"      # gcc, make, etc. (en Fedora 41+ es "development-tools")
```
Notas:
- `webkit2gtk4.1-devel` trae también GTK 3, libsoup3 y JavaScriptCore; **Tauri 2 necesita WebKitGTK 4.1** (no 4.0).
- `libappindicator-gtk3-devel` es lo que indica Tauri para Fedora; algunas distribuciones lo sustituyen por `libayatana-appindicator-gtk3-devel` **(?)**. Si uno no existe en su versión, pruebe el otro. Esta app no crea icono de bandeja, pero Tauri enlaza la librería.
- Para el selector de archivos (`rfd` con backend GTK) hace falta `gtk3-devel` (llega como dependencia de `webkit2gtk4.1-devel`; instálelo explícitamente si falla el enlace) **(?)**.
- Para generar `.rpm` hace falta `rpm-build` (`sudo dnf install rpm-build`) **(?)**. Para el `.deb` desde Fedora, `dpkg` puede ser necesario **(?)**; la CI genera el `.deb` en Ubuntu.
- Para AppImage: `sudo dnf install squashfs-tools fuse fuse-libs` (el AppImage necesita FUSE para ejecutarse) **(?)**. En Nobara `xdg-utils` y `file` suelen venir instalados.
- Opcional, para el selector de archivos nativo en sesiones Wayland: `xdg-desktop-portal` y `xdg-desktop-portal-gtk` (o `-kde`).

### Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"
rustup default stable
```
El crate declara `rust-version = "1.80"` (mínimo). Se desarrolla con Rust estable reciente (1.100 en la máquina de desarrollo).

### Node.js
Node **20 LTS o superior** (22 recomendado) y npm. En Fedora: `sudo dnf install nodejs npm`, o use `nvm`. El frontend usa Vite 7, que requiere Node 20.19+ **(?)**: si `dnf` ofrece una versión menor, use `nvm install 22`.

## 2. Compilar
```bash
git clone <repo> && cd catalogo-RPG-Maker-Library
git checkout feature/desktop-modernization-tauri
cd desktop
npm ci
npx tauri build            # genera todos los formatos configurados para Linux
# o uno solo:
npx tauri build --bundles rpm        # también: deb, appimage
```
Para depurar: `npx tauri dev` (arranca Vite en el puerto 1420 y la ventana).

La primera compilación de Rust tarda varios minutos (se compila SQLite y libwebp incluidos; no hay que instalarlos). El perfil release usa LTO, así que es lento y consume memoria (unos pocos GB).

## 3. Artefactos
Rutas relativas a `desktop/src-tauri/target/release/bundle/`:
| Formato | Ruta | Instalar / ejecutar |
|---|---|---|
| RPM | `rpm/*.rpm` | `sudo dnf install ./archivo.rpm` |
| DEB | `deb/*.deb` | `sudo apt install ./archivo.deb` |
| AppImage | `appimage/*.AppImage` | `chmod +x` y ejecutar |

Los nombres exactos los decide Tauri (producto «RPG Maker Library», versión 0.9.0). También queda el binario suelto `target/release/rpg-library`.

## 4. Ejecutar contra su base de datos
```bash
RPGLIB_DB_PATH="$HOME/juegos/juegos.db" ./rpg-library
```
Sin la variable, la app muestra «Elegir base de datos» (selector o ruta escrita). Haga un backup de `juegos.db` antes de la primera prueba. Para consultar sin riesgo: `RPGLIB_READ_ONLY=1` o el modo solo lectura de Ajustes.

Datos de la app: `~/.config/local.rpgmakerlibrary.desktop` (ajustes) y `~/.local/share/local.rpgmakerlibrary.desktop` (historial/deshacer) **(?)**; los backups van en `backups/` junto a la base.

## 5. Notas específicas de Linux (según el código)
- **Protocolo de portadas:** en Windows las portadas se piden a `http://cover.localhost/<id>` y en Linux/macOS a `cover://localhost/<id>` (`desktop/src/lib/cover.ts`, decidido por el user agent). La CSP de `tauri.conf.json` permite ambos (`img-src ... cover: http://cover.localhost`). **Si las portadas no se ven en Linux, revise primero este punto** (consola del WebView: clic derecho → Inspeccionar solo existe en `tauri dev`) **(?)**.
- **Selector de archivos:** `tauri-plugin-dialog` usa `rfd` con GTK3 en Linux. Con Wayland y sin portal puede fallar o verse sin tema; alternativa: escribir la ruta a mano (la pantalla de primer inicio y el importador lo permiten).
- **Rutas de Windows en la base:** el campo `ruta` de una base creada en Windows contiene `C:\…`. El análisis de rutas de Mantenimiento las marca como «ruta de Windows» en Linux (y las de `/…` como «ruta de Linux» en Windows), en lugar de contarlas como inexistentes. No se modifica ninguna ruta automáticamente.
- **Rutas de red (UNC) o relativas:** no se tocan, en ningún sistema.
- **Abrir carpeta del juego:** la app no abre carpetas automáticamente ni ejecuta comandos de shell.
- **Mayúsculas/minúsculas:** la búsqueda parcial pliega Unicode con `rpg_fold`; el orden por título usa `COLLATE NOCASE` de SQLite (solo ASCII), igual que antes. Puede haber diferencias de orden con acentos respecto a Python.
- **Concurrencia:** `journal_mode = delete` (sin WAL). Si la base está en un recurso de red (NFS/SMB/Samba), los bloqueos de SQLite pueden no ser fiables: use una copia local **(?)**.
- **Contraseñas por página:** necesitan un servicio Secret Service en ejecución y desbloqueado (GNOME Keyring o KWallet ≥ 5.97; KDE en Nobara lo trae). Si no lo hay, la app lo indica y no guarda nada (ADR-019). No hay dependencias nuevas de compilación (`zbus` es Rust puro) **(?)** sin probar en Linux.
- **Bordes de ventana:** el aspecto del WebKitGTK varía con el tema GTK; no hay pruebas visuales en Linux todavía.

## 6. Verificación recomendada la primera vez
1. `cd desktop/src-tauri && cargo test && cargo clippy --all-targets -- -D warnings`.
2. `cd desktop && npm test && npm run typecheck`.
3. `python3 -I tools/interop_test.py` (compila la sonda y comprueba la coexistencia con Python).
4. Abrir una **copia** de la base; revisar portadas, búsqueda, edición, importar una carpeta de prueba y deshacerlo.
5. Anotar el resultado en `docs/PROGRESS.md` (y qué marcas **(?)** se confirmaron).
