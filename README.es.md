# Muddie

Un cliente de MUD moderno y accesible, escrito en Rust.

- **Hecho para oírse además de verse.** Cada línea puede leerse con la voz
  del sistema, hay teclas para repasar lo anterior, y la interfaz está
  expuesta a los lectores de pantalla.
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

## Compilar

```
cargo build --release -p muddie
```

El binario queda en `target/release/muddie`. En Linux la voz necesita
speech-dispatcher en marcha, y el sonido usa `pw-play`, `paplay`, `ffplay` o
`aplay`, el que haya instalado; no hace falta nada más para compilar.

## Instalar en Linux

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
