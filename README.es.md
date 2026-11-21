# Muddie

Un cliente de MUD moderno y accesible, escrito en Rust.

- **Hecho para oírse además de verse.** Cada línea la lee el lector de
  pantalla que uses, y llega así a una línea braille, o la voz del sistema;
  hay teclas para repasar lo anterior, y la interfaz está expuesta a los
  lectores de pantalla. Funciona igual en Linux, Windows y macOS.
- **Programable en Luau**, dentro de un sandbox: triggers, alias, timers,
  teclas, manejadores de GMCP y MSDP, y un mapa que puedes alimentar desde
  cualquier fuente. Los scripts que te pasen no pueden tocar tus ficheros.
- **Sin programar** para lo habitual: triggers, alias y teclas se crean desde
  una ventana.
- **Un automapper** que dibuja mientras caminas, calcula rutas y las recorre
  paso a paso, confirmando cada uno.
- **Sesiones en pestañas**, perfiles con sus propios scripts y mapa, registro,
  reconexión, búsqueda, enlaces, temas, y español e inglés.
- **Protocolos:** GMCP, MSDP, MSSP, MCCP2, MCCP3, MXP, MSP, TLS.

## Instalar

Cada versión trae, en la
[página de versiones](https://github.com/midnattsol/muddie/releases):

- **Windows**: `muddie-windows-x86_64-setup.exe`, un instalador que pone
  Muddie en el menú Inicio y en Configuración para desinstalarlo.
- **macOS**: `muddie-macos-aarch64.dmg`; arrastra Muddie a Aplicaciones. No
  está firmado por un desarrollador conocido por Apple, así que la primera
  vez ábrelo con Abrir en el menú del clic derecho.
- **Debian y Ubuntu**: `muddie_*.deb`, que se abre con el instalador de
  software o con `sudo apt install ./muddie_*.deb`.
- **Cualquier Linux, Windows o macOS**: un archivo para descomprimir y
  ejecutar tal cual.

## Compilar

```
cargo build --release -p muddie
```

El binario queda en `target/release/muddie`. Para compilar hacen falta CMake
y un compilador de C++; en Linux también los ficheros de desarrollo de D-Bus y
Speech Dispatcher (`sudo apt install cmake libdbus-1-dev libspeechd-dev
pkg-config` en Debian y Ubuntu), sin los que el programa sale mudo. El sonido
usa `pw-play`, `paplay`, `ffplay` o `aplay`, el que haya instalado.

Además de `cargo test`, `dbus-run-session -- tests/orca/run.sh
target/release/muddie` prueba Muddie con Orca en un escritorio propio que
nadie ve ni oye (necesita Orca, Xvfb y xdotool); el CI lo ejecuta cada
noche.

## Instalar en Linux desde el código

```
./install.sh
```

Compila Muddie y lo instala para tu usuario en `~/.local`: el programa, su
entrada en el menú de aplicaciones y su icono. `./uninstall.sh` lo quita y
conserva tus perfiles y mapas. Para instalarlo para todos:
`PREFIX=/usr/local sudo -E ./install.sh --no-build` después de compilar.

El icono se genera a partir de `assets/icon.png` con `assets/make-icons.sh`.

## Usar

```
muddie mud.ejemplo.org 4000
```

El [manual](docs/manual.md), la [referencia de scripts](docs/scripting.md) y
los [scripts de ejemplo](examples/scripts) están en inglés.

## Estado del proyecto

Muddie es nuevo. Se desarrolla y se prueba en Linux; las versiones de Windows
y macOS compilan pero apenas se han usado, y se agradecen especialmente los
comentarios de usuarios de lector de pantalla.

## Licencia

Bajo la [licencia Apache 2.0](LICENSE-APACHE) o la [licencia MIT](LICENSE-MIT),
a tu elección.
