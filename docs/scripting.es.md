# Referencia de scripts de Muddie

English: [scripting.md](scripting.md). ¿Nunca has hecho un script? Empieza
por [first-scripts.es.md](first-scripts.es.md). Para hacer un plugin, lee
[plugins.es.md](plugins.es.md). Todas las funciones con sus tipos, para un
editor que compruebe Luau, están en [muddie.d.luau](muddie.d.luau).

Esto es la referencia: cada función, qué hace, qué recibe y qué devuelve. Los
scripts se escriben en [Luau](https://luau.org), un dialecto de Lua 5.1
rápido y con tipos.

Hay una sola API para todo el mundo. Lo que un script muestra se ve, y lo que
dice se oye. El mismo script sirve a quien mira la pantalla y a quien escucha
con un lector de pantalla. Muddie te da los datos y las herramientas; tus
scripts deciden qué se muestra y qué se dice.

Puedes compartir scripts sin miedo. No pueden leer ni escribir archivos,
arrancar programas ni cargar código de fuera de las carpetas de scripts. Si
un script tarda más de medio segundo seguido, Muddie lo para con un error y
sigue funcionando.

## Índice

1. [Empieza aquí](#empieza-aquí)
   - [Dónde van los scripts](#dónde-van-los-scripts)
   - [Volver a ejecutar un script](#volver-a-ejecutar-un-script)
   - [Un primer script](#un-primer-script)
2. [¿Cuál uso?](#cuál-uso)
   - [Para mostrar o decir algo](#para-mostrar-o-decir-algo)
   - [Para reaccionar a algo](#para-reaccionar-a-algo)
   - [Para guardar datos](#para-guardar-datos)
   - [Para seguir la sala del jugador](#para-seguir-la-sala-del-jugador)
3. [Reaccionar a lo que dice el MUD](#reaccionar-a-lo-que-dice-el-mud)
   - [`trigger(pattern, fn, options)`](#triggerpattern-fn-options)
4. [Cambiar cómo se ve o se oye una línea](#cambiar-cómo-se-ve-o-se-oye-una-línea)
   - [Campos de una línea](#campos-de-una-línea)
   - [`line:hide()`](#linehide)
   - [`line:mute()`](#linemute)
   - [`line:read(text, options)`](#linereadtext-options)
   - [`line:replace(text)`](#linereplacetext)
   - [`line:paint(fg, bg)`](#linepaintfg-bg)
   - [`line:link(command, options)`](#linelinkcommand-options)
   - [`line:to_panel(name)`](#lineto_panelname)
5. [Mostrar avisos y hablar](#mostrar-avisos-y-hablar)
   - [`echo(text, options)`](#echotext-options)
   - [`speak(text, options)`](#speaktext-options)
   - [`notify(text, options)`](#notifytext-options)
   - [`tr(texts)`](#trtexts)
   - [`log(text)`](#logtext)
   - [`hush()`](#hush)
6. [Ofrecer una lista, y el portapapeles](#ofrecer-una-lista-y-el-portapapeles)
   - [`choose(items, fn, options)`](#chooseitems-fn-options)
   - [`action(name, fn, options)`](#actionname-fn-options)
   - [`clipboard.set(text)`](#clipboardsettext)
   - [`clipboard.get()`](#clipboardget)
7. [Elegir cómo se oye algo](#elegir-cómo-se-oye-algo)
8. [Mandar órdenes y crear atajos](#mandar-órdenes-y-crear-atajos)
   - [`send(text)`](#sendtext)
   - [`alias(pattern, fn, options)`](#aliaspattern-fn-options)
   - [`on("send", fn)`](#onsend-fn)
9. [Asignar teclas](#asignar-teclas)
   - [`macro(key, action, options)`](#macrokey-action-options)
10. [Cambiar las teclas de Muddie (atajos)](#cambiar-las-teclas-de-muddie-atajos)
   - [`shortcuts.<name>`](#shortcutsname)
   - [`shortcuts.all()`](#shortcutsall)
   - [Todos los atajos](#todos-los-atajos)
11. [Hacer algo más tarde o cada cierto tiempo](#hacer-algo-más-tarde-o-cada-cierto-tiempo)
   - [`after(seconds, fn, options)`](#afterseconds-fn-options)
   - [`every(seconds, fn, options)`](#everyseconds-fn-options)
12. [Esperar en mitad de un script](#esperar-en-mitad-de-un-script)
   - [`wait(seconds)`](#waitseconds)
   - [`wait("line", pattern, options)`](#waitline-pattern-options)
   - [`wait(what, name, options)`](#waitwhat-name-options)
   - [`lines(options)`](#linesoptions)
   - [Dónde no se puede esperar](#dónde-no-se-puede-esperar)
13. [Recordar cosas (vars)](#recordar-cosas-vars)
   - [`vars`](#vars)
   - [`expand(text)`](#expandtext)
   - [`on("var", name, fn)`](#onvar-name-fn)
14. [Mostrar el estado del personaje (indicadores)](#mostrar-el-estado-del-personaje-indicadores)
   - [`gauges.set(name, gauge)`](#gaugessetname-gauge)
   - [`gauges.remove(name, options)`](#gaugesremovename-options)
   - [`gauges.get(name)`](#gaugesgetname)
   - [`gauges.all()`](#gaugesall)
   - [`on("gauge", name, fn, options)`](#ongauge-name-fn-options)
15. [Paneles de texto y canales](#paneles-de-texto-y-canales)
   - [`panels.set(name, text)`](#panelssetname-text)
   - [`panels.clear(name)`](#panelsclearname)
   - [`panels.add(name, text, options)`](#panelsaddname-text-options)
   - [`panels.size(name)`](#panelssizename)
   - [`channels.say(name, text, options)`](#channelssayname-text-options)
   - [`channels.open(name, options)`](#channelsopenname-options)
   - [`channels.close(name)`](#channelsclosename)
   - [`channels.all()`](#channelsall)
16. [Reproducir sonidos](#reproducir-sonidos)
   - [`sounds.play(file, options)`](#soundsplayfile-options)
   - [`sounds.volume(group, volume)`](#soundsvolumegroup-volume)
   - [`sounds.fetch(names)`](#soundsfetchnames)
   - [`sounds.stop()`](#soundsstop)
17. [Hablar con el MUD por detrás (GMCP, MSDP)](#hablar-con-el-mud-por-detrás-gmcp-msdp)
   - [`gmcp.get(package)`](#gmcpgetpackage)
   - [`gmcp.send(package, data)`](#gmcpsendpackage-data)
   - [`gmcp.support(module, ...)`](#gmcpsupportmodule-)
   - [`gmcp.merge(package, ...)`](#gmcpmergepackage-)
   - [`msdp.get(variable)`, `msdp.send(command, ...)`](#msdpgetvariable-msdpsendcommand-)
   - [`msdp.report(variable, ...)`](#msdpreportvariable-)
   - [`mssp.get()`](#msspget)
18. [Reaccionar a otros sucesos (on)](#reaccionar-a-otros-sucesos-on)
   - [`on(what, name, fn, options)`](#onwhat-name-fn-options)
   - [`emit(name, data)`](#emitname-data)
19. [Encender y apagar cosas](#encender-y-apagar-cosas)
   - [Opciones, sitios y etiquetas](#opciones-sitios-y-etiquetas)
   - [Lo que devuelven `trigger`, `alias`, `macro`, `after`, `every` y `on`](#lo-que-devuelven-trigger-alias-macro-after-every-y-on)
   - [`triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts`](#triggers-aliases-macros-timers-gauges-scripts)
   - [`tags`](#tags)
   - [Lo que se va con un script](#lo-que-se-va-con-un-script)
20. [La sesión y la línea de órdenes](#la-sesión-y-la-línea-de-órdenes)
   - [Campos de `session`](#campos-de-session)
   - [`session.completions(name, words)`](#sessioncompletionsname-words)
   - [`session.feed(text)`](#sessionfeedtext)
   - [`output.last(count)`](#outputlastcount)
21. [Hacer el mapa y recorrerlo](#hacer-el-mapa-y-recorrerlo)
   - [Salas y sitios](#salas-y-sitios)
   - [Ajustes del mapa](#ajustes-del-mapa)
   - [Seguir al jugador](#seguir-al-jugador)
     - [`map.visit(room)`](#mapvisitroom)
     - [`map.set_room(room)`](#mapset_roomroom)
     - [`map.here(place)`](#maphereplace)
     - [`map.arrive{name, exits, came}`](#maparrivename-exits-came)
     - [`map.add{name, exits, came}`](#mapaddname-exits-came)
     - [`map.learn`](#maplearn)
     - [`map.candidates()`](#mapcandidates)
     - [`map.failed()`](#mapfailed)
   - [Caminar](#caminar)
     - [`map.way(to, from)`](#mapwayto-from)
     - [`map.go(to)`](#mapgoto)
     - [`map.stop()`](#mapstop)
   - [Consultar](#consultar)
     - [`map.room(place)`](#maproomplace)
     - [`map.find(text, options)`](#mapfindtext-options)
     - [`map.exit(from, direction)`](#mapexitfrom-direction)
     - [`map.direction(word)`](#mapdirectionword)
     - [`map.count()`, `map.areas()`, `map.id(text)`](#mapcount-mapareas-mapidtext)
     - [`map.marks()`](#mapmarks)
   - [Cambiar el mapa](#cambiar-el-mapa)
     - [`map.mark(name, place)`](#mapmarkname-place)
     - [`map.edit_room(place, changes)`](#mapedit_roomplace-changes)
     - [`map.set_exit(from, direction, to, options)`](#mapset_exitfrom-direction-to-options)
     - [`map.remove_exit(from, direction)`, `map.remove_room(place)`](#mapremove_exitfrom-direction-mapremove_roomplace)
     - [`map.clear()`](#mapclear)
     - [`map.tidy(area)`](#maptidyarea)
     - [`map.undo()`](#mapundo)
     - [`map.show_area(area)`](#mapshow_areaarea)
   - [El mapa que publica un MUD](#el-mapa-que-publica-un-mud)
     - [`map.import(address)`](#mapimportaddress)
     - [`map.newer(address, fn)`](#mapneweraddress-fn)
     - [`map.action(name, fn, options)`](#mapactionname-fn-options)
22. [Compartir código entre scripts](#compartir-código-entre-scripts)
   - [`require(name)`](#requirename)
23. [Cuando algo falla](#cuando-algo-falla)
24. [Lo que los scripts no pueden hacer](#lo-que-los-scripts-no-pueden-hacer)

## Empieza aquí

### Dónde van los scripts

Los scripts son archivos de texto que terminan en `.luau`, en la carpeta de
configuración: `~/.config/muddie` en Linux, `%APPDATA%\muddie` en Windows.

| Archivo | Cuándo se ejecuta |
|---|---|
| `init.luau` | En todas las sesiones |
| `profiles/<profile>/scripts/*.luau` | En ese perfil, por orden de nombre; los scripts de un plugin están en la carpeta del plugin |
| los archivos que se dan con `--script` | En todas las sesiones, al final |

También puedes escribirlos dentro de Muddie: en el editor, **Scripts** tiene
todos los scripts, para escribirlos, ejecutarlos y encenderlos o apagarlos.

### Volver a ejecutar un script

Un script se ejecuta una vez, de arriba abajo, cuando Muddie lo carga. Lo que
deja preparado (disparadores, teclas, indicadores...) sigue funcionando
mientras juegas. Cuando cambies un script, vuelve a ejecutarlo: guárdalo en
el editor, pulsa su botón de ejecutar o elige **Reiniciar scripts** en la
paleta (Ctrl+K) para volver a ejecutarlos todos. Para probar una línea de
Luau sin archivo, usa **Ejecutar Luau…** en la paleta.

### Un primer script

Guarda esto como `init.luau` y elige **Reiniciar scripts**.

```lua
-- Say who tells you something, cutting in, and paint the name.
trigger("^(?<who>\\w+) tells you: (.*)$", function(m, line)
    line:paint("bright yellow")
    speak(`{m.who} says {m[2]}`, {aloud = "cut"})
end)

-- Keys of the numeric keypad walk.
macro("numpad8", "north")
macro("numpad2", "south")

-- Health as a bar, from what the MUD sends over GMCP.
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("HP", {value = vitals.hp, max = vitals.maxhp})
end)
```

[first-scripts.es.md](first-scripts.es.md) sigue desde aquí, paso a paso.

## ¿Cuál uso?

Varias funciones hacen trabajos parecidos. Estas tablas te ayudan a elegir.

### Para mostrar o decir algo

| Quieres... | Usa |
|---|---|
| Mostrar una línea en la salida y leerla en voz alta | [`echo`](#echotext-options) |
| Decir algo en voz alta, sin mostrarlo | [`speak`](#speaktext-options) |
| Avisar al jugador de parte del propio Muddie, que se oiga incluso en otra sesión | [`notify`](#notifytext-options) |
| Decir otra cosa en lugar de una línea que ha llegado | [`line:read`](#linereadtext-options) |
| Dejar una nota que solo verás tú, que escribes el script | [`log`](#logtext) |

### Para reaccionar a algo

| Cuando... | Usa |
|---|---|
| Una línea de la salida coincide con un patrón | [`trigger`](#triggerpattern-fn-options) |
| Escribes una orden que coincide con un patrón | [`alias`](#aliaspattern-fn-options) |
| Pulsas una tecla | [`macro`](#macrokey-action-options) |
| Pasa algo que no es texto: datos GMCP, un indicador, una variable, conectarse | [`on`](#onwhat-name-fn-options) |
| La siguiente línea, o las siguientes, dentro de una misma función | [`wait("line")`](#waitline-pattern-options), [`lines`](#linesoptions) |
| Cualquiera de las tres primeras, sin escribir código | Una regla en el editor |

### Para guardar datos

| Quieres que dure... | Usa |
|---|---|
| Mientras el script está cargado (se pierde al volver a ejecutarlo) | Una variable `local` del script |
| Para siempre, guardado con el perfil | [`vars`](#vars) |

Las variables `local` de un script duran mientras está cargado:

- **Se conservan** cuando se corta la conexión y vuelve. A los scripts solo
  se les avisa, con `on("disconnect")` y `on("connect")`. También se
  conservan cuando cambias y guardas otro script o una regla.
- **Se pierden** cuando el propio script se vuelve a ejecutar (lo guardas o
  pulsas su botón de ejecutar), cuando se vuelven a ejecutar todos
  (**Reiniciar scripts**, otro perfil, encender o apagar un plugin) y cuando
  cierras Muddie.

Un script que se vuelve a ejecutar empieza desde su primera línea sin nada.
Ahí es donde tienes que volver a pedirle al MUD lo que ya había mandado.

### Para seguir la sala del jugador

| El servidor... | Usa |
|---|---|
| Dice la sala, con su id (`Room.Info` de GMCP, `ROOM` de MSDP) | Lo hace el plugin estándar; o [`map.visit`](#mapvisitroom) |
| Dice el id de una sala que el mapa ya tiene | [`map.here`](#maphereplace) |
| Solo dice el nombre y las salidas de la sala | [`map.arrive`](#maparrivename-exits-came) |
| Solo dice el nombre y las salidas, y el mapa aún no tiene sala ahí | [`map.add`](#mapaddname-exits-came) |

## Reaccionar a lo que dice el MUD

Un disparador vigila la salida del juego y ejecuta tu función cuando una
línea coincide con un patrón. Sirve para responder al juego, avisarte de algo
o cambiar cómo se ve o se oye una línea.

### `trigger(pattern, fn, options)`

Llama a `fn(m, line)` por cada línea de la salida que coincide con
`pattern`.

**Parámetros:**

- `pattern`: una expresión regular, con la sintaxis de `regex` de Rust, así
  que valen `\\d`, `(?<name>...)` y `(?i)`.
- `fn(m, line)`: `m[0]` es la coincidencia entera, y `m[1]`, `m[2]`... son
  sus grupos. Un grupo con nombre también es `m.name`. `line` es
  [la propia línea](#cambiar-cómo-se-ve-o-se-oye-una-línea).

**Opciones:**

- `place`, `tags`: lo que lo enciende y lo apaga (ver
  [sitios y etiquetas](#opciones-sitios-y-etiquetas)).
- `once`: quita el disparador después de saltar una vez.
- `prompt`: con `true`, solo coincide con prompts; con `false`, con todo
  menos prompts. Si no la das, con ambos. Un prompt es una línea que el MUD
  marca como prompt, o texto sin final de línea que ha dejado de cambiar.

**Devuelve:** lo que ha creado (ver [lo que devuelven](#lo-que-devuelven-trigger-alias-macro-after-every-y-on)).

**Ejemplo:**

```lua
trigger("^You are hungry", function() send("eat bread") end,
        {place = "Survival/Eat", tags = {"survival"}})

trigger("^\\[spam\\]", function(m, line) line:hide() end)

-- A prompt that is not read, and what comes after it on its line is.
trigger("^HP \\d+> (.*)$", function(m, line) line:read(m[1]) end)
```

Los disparadores se ejecutan en el orden en que se crearon. Una línea pasa
por todos los que coinciden con ella, no solo por el primero. No hay
prioridades.

**Ver también:** [`wait("line")`](#waitline-pattern-options) y
[`lines`](#linesoptions), para recoger las siguientes líneas dentro de una
misma función; [`alias`](#aliaspattern-fn-options), para lo que escribes.

## Cambiar cómo se ve o se oye una línea

Cada disparador recibe la línea con la que coincidió. Mientras se ejecuta tu
función, puedes ocultar la línea, cambiarle el color, sustituir palabras,
hacer un enlace o decir otra cosa en su lugar. Si la guardas para después,
solo podrás leer lo que decía.

### Campos de una línea

| Campo | Qué es |
|---|---|
| `line.text` | Lo que dice la línea |
| `line.prompt` | Si es un prompt: marcado así por el MUD, o sin final de línea |
| `line.spans` | La línea en trozos de texto, cada uno con `text`, `fg`, `bg`, `bold`, `italic`, `underline` e `inverse` |

**Ojo:** una línea no es su texto. Para usar sus palabras, usa `line.text`:
`echo(line.text)`, `line.text:match("%d+")`, `line.text == "Done."`. Pasar la
línea donde se espera un texto da un error que lo dice. Pero
`line == "Done."` no es un error en Luau: simplemente siempre es falso. Pon
`--!strict` al principio del script y
[las definiciones](#cuando-algo-falla) lo detectarán.

### `line:hide()`

Oculta la línea: no se muestra ni se lee.

**Devuelve:** nada.

**Ver también:** [`line:mute()`](#linemute), para mostrarla sin leerla.

### `line:mute()`

Muestra la línea pero no la lee en voz alta al llegar. Se sigue guardando,
así que puedes releerla con las teclas de repaso.

**Devuelve:** nada.

**Ver también:** [`line:hide()`](#linehide), para ocultarla también.

### `line:read(text, options)`

Lee `text` en lugar de la línea; la línea se sigue mostrando tal cual.

**Opciones:** `aloud` y `always` (ver
[Elegir cómo se oye algo](#elegir-cómo-se-oye-algo)). Sin ellas, `text` se
oye como se oiría la línea.

**Devuelve:** nada.

**Ver también:** [`line:replace`](#linereplacetext), para cambiar también
lo que se ve.

### `line:replace(text)`

Pone `text` en lugar de lo que coincidió. La línea se ve así y, salvo que
digas otra cosa con `line:read`, se lee así.

**Devuelve:** si ha podido. No puede sobre algo que otro disparador ya ha
sustituido.

### `line:paint(fg, bg)`

Cambia el color de lo que coincidió.

**Parámetros:** `fg` es el color del texto y `bg` el del fondo. Un color es
un nombre (`"red"`, `"bright blue"`), `"#rrggbb"` o un número de la paleta.
`nil` deja ese color como está.

**Devuelve:** nada.

### `line:link(command, options)`

Convierte lo que coincidió en un enlace que manda `command`. Puedes llegar a
él con el teclado, como a los enlaces que marca el MUD.

**Opciones:** `hint`, un texto que dice para qué sirve el enlace.

**Devuelve:** si ha podido, como `line:replace`.

**Ejemplo:**

```lua
-- Someone arrives: paint the line and make the name a link.
trigger("^(\\w+) arrives\\.$", function(m, line)
    line:paint("bright green")
    line:link("look " .. m[1], {hint = "look at them"})
end)

-- Shown, but not read aloud.
trigger("^You hear a distant bell\\.$", function(m, line) line:mute() end)
```

### `line:to_panel(name)`

Copia la línea, con sus colores, al final del panel de texto `name`.

**Devuelve:** nada.

**Ver también:** [`panels.add`](#panelsaddname-text-options), para añadir tu
propio texto a un panel.

## Mostrar avisos y hablar

Estas funciones le hacen llegar palabras al jugador: en la salida, en la
esquina de la ventana o solo por los oídos. Para elegir una, mira
[¿Cuál uso?](#para-mostrar-o-decir-algo)

### `echo(text, options)`

Muestra una línea en la salida y la lee en voz alta.

**Parámetros:** `text` también puede ser una tabla con un texto por idioma,
como en [`tr`](#trtexts).

**Opciones:** `aloud` y `always` (ver
[Elegir cómo se oye algo](#elegir-cómo-se-oye-algo)).

**Devuelve:** nada.

Para mostrar una línea sin leerla, da `{aloud = "never"}`.

**Ver también:** [`speak`](#speaktext-options), [`notify`](#notifytext-options).

### `speak(text, options)`

Dice algo en voz alta y no muestra nada.

**Parámetros:** `text` también puede ser una tabla con un texto por idioma.

**Opciones:** `aloud` y `always`.

**Devuelve:** nada.

**Ejemplo:**

```lua
speak("You are dying!", {aloud = "cut", always = true})
```

**Ver también:** [`echo`](#echotext-options), [`hush`](#hush).

### `notify(text, options)`

Muestra un aviso al jugador. Aparece unos segundos en la esquina inferior
derecha y luego queda en la lista de **Mensajes** (el icono de la campana).
Nunca sale en la salida del juego.

El aviso lo da Muddie, no el juego. Por eso se lee siempre en voz alta,
aunque la lectura del juego esté apagada o su sesión no sea la que está
delante.

**Parámetros:** `text` también puede ser una tabla con un texto por idioma.

**Opciones:** solo `aloud`. Para que el aviso no se lea:
`{aloud = "never"}`.

**Devuelve:** nada.

**Ver también:** [`echo`](#echotext-options), [`speak`](#speaktext-options).

### `tr(texts)`

Elige el texto del idioma de la interfaz de entre una tabla con un texto por
código de idioma. Si falta ese idioma, usa el mismo idioma sin país (`es`
para `es_AR`); si no, el inglés; si no, el primero que haya. Úsalo para todo
lo que un script compartido le enseña al jugador.

**Devuelve:** el texto elegido.

**Ejemplo:**

```lua
notify(tr{en = "Map updated.", es = "Mapa actualizado."})
```

`echo`, `notify` y `speak` también admiten directamente los textos por
idioma: `speak{en = "You are dying", es = "Te mueres"}`.

### `log(text)`

Escribe `text` en la salida estándar si Muddie se arrancó con `--debug`, y si
no, no hace nada. Úsalo para mensajes que solo necesita quien escribe el
script.

**Devuelve:** nada.

**Ver también:** [`muddie --debug`](#cuando-algo-falla).

### `hush()`

Calla lo que se está leyendo en ese momento, en la sesión que está delante.

**Devuelve:** nada.

## Ofrecer una lista, y el portapapeles

A veces el jugador tiene que elegir una cosa entre varias: a quién responder,
qué evento volver a leer, qué objeto mirar. Una lista lo hace igual para
todos: se abre en la paleta, las flechas la recorren, cada elemento se lee en
voz alta, Intro elige y Escape la cierra. Escribir acota la lista.

### `choose(items, fn, options)`

Ofrece `items` en una lista y llama a `fn` con lo que el jugador eligió.

**Parámetros:**

- `items`: una lista. Cada elemento es un texto, o `{text = ..., about = ...}`,
  donde `about` dice algo más de él; cada uno puede ser una tabla de textos
  por idioma, como en [`tr`](#trtexts).
- `fn`: se le llama con el lugar del elemento elegido en la lista (desde 1) y
  su texto; con `nil` si la lista se cerró sin elegir.

**Opciones:** `title`, lo que pregunta la lista, como texto o textos por
idioma. Sin él, la lista dice «Elige uno».

**Devuelve:** nada. A `fn` se le llama después, cuando el jugador elige.

Una lista nueva cierra la que siguiera abierta: el script que la ofreció
recibe `nil`.

**Ejemplo:**

```lua
-- Choose whom to reply to, from those who told you something.
local senders = {}
trigger("^(\\w+) tells you: ", function(m)
    if not table.find(senders, m[1]) then table.insert(senders, m[1]) end
end)
alias("^replyto$", function()
    choose(senders, function(index, name)
        if name then vars.reply_to = name end
    end, {title = {en = "Reply to", es = "Responder a"}})
end)
```

**Ver también:** [`clipboard.set(text)`](#clipboardsettext).

### `action(name, fn, options)`

Ofrece hacer algo, que se elige por `name` en la paleta (Ctrl+K), mientras el
script está cargado: lo que no es del mapa, como cambiar de modo o activar
unas teclas. Las acciones de un plugin salen bajo el nombre del plugin.
`action(name)` la quita.

**Parámetros:** `name` puede darse por idioma, como en [`tr`](#trtexts).

**Opciones:** `checked`, verdadero o falso, la convierte en un interruptor:
al elegirla se llama a `fn(on)` con cómo queda.

**Devuelve:** nada.

**Ejemplo:**

```lua
-- A switch in the palette, remembered.
action({en = "Read the channels", es = "Leer los canales"}, function(on)
    vars.read_channels = on
end, {checked = vars.read_channels ~= false})
```

**Ver también:** [`map.action(name, fn, options)`](#mapactionname-fn-options),
para lo que se hace con el mapa, que sale también en el menú del mapa.

### `clipboard.set(text)`

Pone `text` en el portapapeles, para que el jugador lo pegue donde quiera.

**Devuelve:** nada.

**Ejemplo:**

```lua
-- F7 copies your life, to paste it where you like.
macro("f7", function()
    local vitals = gmcp.get("Char.Vitals")
    if vitals then clipboard.set(`{vitals.hp}({vitals.maxhp})`) end
end)
-- "repg" says what Muddie copied last on the group channel.
alias("^repg$", function()
    local text = clipboard.get()
    if text then send("group " .. text) end
end)
```

**Ver también:** [`clipboard.get()`](#clipboardget).

### `clipboard.get()`

Da el último texto que Muddie puso en el portapapeles: un script, o el
jugador con la tecla de revisión que copia una línea.

**Devuelve:** el texto, o `nil` si Muddie aún no ha copiado nada.

Ojo: no lee lo que copiaron otros programas. Un script que te pasen no puede
ver, por ejemplo, una contraseña que copiaste de otro sitio.

**Ver también:** [`clipboard.set(text)`](#clipboardsettext).

## Elegir cómo se oye algo

`speak`, `echo`, `line:read`, `panels.add`, `channels.say` y `gauges.set`
admiten las mismas dos opciones para decir cómo se oyen. Casi nunca te hará
falta ninguna.

| Opción | Cómo se oye |
|---|---|
| `aloud = "usual"` | Lo normal. Detrás de lo que se está leyendo. `line:read` se oye como se oiría la línea; un indicador, según su tipo (ver [`gauges.set`](#gaugessetname-gauge)) |
| `aloud = "wait"` | Detrás de lo que se está leyendo |
| `aloud = "cut"` | En el momento, cortando lo que se está leyendo |
| `aloud = "never"` | Nunca |
| `aloud = "lowest"` | Solo indicadores: espera su turno, pero corta cuando el indicador baja de su paso más bajo |
| `always = true` | También con la lectura del juego apagada |

Todo lo que un script dice sobre el juego sigue las mismas reglas que las
líneas del propio juego:

- Solo se oye en la sesión que está delante.
- No se oye con la lectura del juego apagada, salvo que des
  `always = true`.
- No se oye si los ajustes, el panel o el canal dicen que no se lea.
  `always` no se salta eso.
- Las palabras que se acaban de leer, como línea o en un canal, no se
  vuelven a leer, salvo que el script pida `"cut"`.

`notify` es distinto: habla Muddie, así que se oye en cualquier sesión.

## Mandar órdenes y crear atajos

Usa `send` para mandar una orden como si la escribiera el jugador, y `alias`
para crear tus propias órdenes cortas. `on("send")` ve cada orden justo antes
de que salga, así que puede cambiarla o pararla.

### `send(text)`

Manda una orden al MUD tal cual está escrita. No se le aplican los alias.

**Devuelve:** nada.

**Ver también:** [`on("send")`](#onsend-fn), que la ve antes de que salga.

### `alias(pattern, fn, options)`

Cuando lo que escribes coincide con `pattern`, llama a `fn(m, input)` en vez
de mandarlo.

**Parámetros:** `m` tiene los grupos, como en un
[disparador](#triggerpattern-fn-options); `input` es lo que escribiste.

**Opciones:** `place`, `tags` y `once`.

**Devuelve:** lo que ha creado (ver [lo que devuelven](#lo-que-devuelven-trigger-alias-macro-after-every-y-on)).

**Ejemplo:**

```lua
alias("^k (.+)$", function(m) send("kill " .. m[1]) end)
```

**Ver también:** [`macro`](#macrokey-action-options), para una tecla en vez
de una orden.

### `on("send", fn)`

Llama a `fn(text)` con cada orden justo antes de que vaya al MUD, venga de
donde venga: escrita, o mandada por un script, una regla, un camino o un
enlace.

**Parámetros:** lo que devuelve `fn` decide qué se manda:

- un texto: se manda ese en su lugar;
- `false`: no se manda nada;
- `nil`: la orden va tal cual.

**Devuelve:** lo que ha creado, como [`on`](#onwhat-name-fn-options).

**Ejemplo:**

```lua
on("send", function(text)
    if text == "quit" then
        notify("Type it in full: quit!")
        return false
    end
    if text == "quit!" then return "quit" end
end)
```

No se le avisa del nombre ni de la contraseña con que entra el cliente, ni de
las órdenes que manda la propia función. Dentro no se puede
[esperar](#dónde-no-se-puede-esperar).

## Asignar teclas

Una tecla asignada ejecuta órdenes o una función al pulsarla. Es lo más
rápido para andar con el teclado numérico o lanzar un hechizo con F5.

### `macro(key, action, options)`

Asigna a `key` unas órdenes o una función: `macro("f5", "look")`,
`macro("ctrl+shift+k", fn)`.

**Parámetros:**

- `key`: el nombre de una tecla (ver abajo).
- `action`: una función, u órdenes, que se mandan como las manda una regla:
  separadas por `;`, con cada `$name` sustituido por su variable y sin pasar
  por los alias.

**Opciones:** `place` y `tags`.

**Devuelve:** lo que ha creado (ver [lo que devuelven](#lo-que-devuelven-trigger-alias-macro-after-every-y-on)).

**Ejemplo:**

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad5", "look")
```

Si asignas otra vez una tecla con otro `place`, existen las dos, y se
ejecuta la primera asignada que esté encendida. Si la asignas otra vez con el
mismo sitio, lo nuevo sustituye a lo de antes.

**En el mapa.** Mientras exploras el mapa, una tecla cuya macro manda solo
una dirección mueve la sala elegida en esa dirección en vez de caminar: en
las palabras del MUD o en inglés (`"este"`, `"e"`, `"east"`), como la lee
[`map.direction`](#mapdirectionword). Así las teclas con las que caminas en
el juego también caminan por el mapa. Una macro que llama a una función, o
que manda otra cosa, se queda como está.

**Nombres de teclas.** Se escriben en minúsculas, con los modificadores en el
orden `ctrl`, `alt`, `shift`: `f1`...`f35`, `ctrl+k`, `alt+shift+up`. Del
teclado principal solo puedes asignar las teclas de función y las que se
pulsan con Ctrl o Alt. Así una tecla asignada nunca te roba lo que escribes.

**El teclado numérico** tiene nombres propios: `numpad0` a `numpad9`,
`numpadadd`, `numpadsubtract`, `numpadmultiply`, `numpaddivide`,
`numpaddecimal` y `numpadenter`. Con Bloq Num apagado, las cifras son otras
teclas, con el nombre de lo que llevan impreso: `numpadup`, `numpaddown`,
`numpadleft`, `numpadright`, `numpadhome`, `numpadend`, `numpadpageup`,
`numpadpagedown`, `numpadclear` (el 5), `numpadinsert` y `numpaddelete`. En
cuanto asignas una tecla del teclado numérico, deja de escribir su cifra.

## Cambiar las teclas de Muddie (atajos)

Cada tecla que Muddie usa para sí mismo es un atajo que puedes cambiar: la
paleta, las teclas de revisión que leen la salida, las teclas que mueven por
el mapa mientras lo exploras, las teclas del editor. Las teclas que tiene
cada uno al principio están en la [tabla de abajo](#todos-los-atajos). Puedes
cambiarlas en **Ajustes → Teclas**, o desde un script. Un script viene bien
para tener todas tus teclas en un archivo, compartirlo y que cada persona lo
ajuste a su gusto.

Las teclas que elige un script van antes que las de Ajustes mientras el
script está cargado. Cuando el script se apaga o se quita, las teclas vuelven
a ser las de antes. Ajustes dice qué script eligió cada una.

### `shortcuts.<name>`

Lee o cambia las teclas de un atajo, por su nombre.

- **Léelo** para saber las teclas que tiene ahora, como una lista de textos:
  `shortcuts.explore` da `{"alt+e"}`.
- **Ponle** una tecla o una lista de teclas: `shortcuts.explore = "f9"`,
  `shortcuts.map_north = {"up", "k"}`.
- **Ponle `false`** para dejar el atajo sin teclas.
- **Ponle `nil`** para devolverle las teclas que tenía antes del script.

Las teclas se escriben como en [`macro`](#macrokey-action-options), y en un
Mac también con `cmd`. Para `hush` también vale una tecla modificadora sola:
`"ctrl"`. Da igual escribir en mayúsculas o minúsculas.

**Dónde puede ir cada tecla:**

- Un atajo que funciona en todas partes necesita teclas que no escriban:
  con Ctrl, Alt o Cmd, una tecla de función o una tecla del teclado numérico.
- En la línea de órdenes también valen teclas que no escriben, como las
  flechas, pero no Intro.
- En el mapa, mientras lo exploras, vale cualquier tecla: el mapa tiene el
  teclado para él solo.
- Tab, Escape y Mayús+F10 siempre sirven para moverse por la ventana. No se
  pueden usar para otra cosa.

**Cuando dos atajos quieren las mismas teclas:**

- Si un script le da a un atajo teclas que ya tiene otro donde funcionan los
  dos, el otro las pierde.
- Una [macro](#macrokey-action-options) con las mismas teclas va antes que el
  atajo de Muddie.
- Mientras exploras el mapa, sus teclas van primero.

**Devuelve:** al leerlo, una lista de textos.

Ojo: un nombre que no existe, una tecla que no existe o una tecla que no
puede ir donde funciona el atajo dan un error que dice cuál.

**Ejemplo:**

```lua
-- My keys: explore the map with F9, and read the lines with the keypad.
shortcuts.explore = "f9"
shortcuts.read_previous = "numpad8"
shortcuts.read_next = "numpad2"
-- On the map, the letters too, besides the arrows.
shortcuts.map_north = {"up", "k"}
shortcuts.map_south = {"down", "j"}
-- Ctrl no longer silences speech.
shortcuts.hush = false
```

**Ver también:** [`shortcuts.all()`](#shortcutsall), [`macro`](#macrokey-action-options).

### `shortcuts.all()`

Da la lista de todos los atajos, con las teclas que tienen ahora. Pásasela a
una IA, o léela en voz alta, para saber qué puedes cambiar.

**Devuelve:** una lista de tablas, cada una con:

- `name`: su nombre, para usarlo como `shortcuts.<name>`.
- `place`: dónde funciona: `"everywhere"` (en todas partes), `"reading"`
  (leyendo), `"line"` (la línea de órdenes), `"map"` (explorando el mapa),
  `"editor"` o `"layout"` (la disposición de paneles).
- `about`: qué hace, en el idioma de la interfaz.
- `keys`: sus teclas ahora, como una lista de textos.

**Ejemplo:**

```lua
for _, shortcut in shortcuts.all() do
    echo(`{shortcut.name}: {table.concat(shortcut.keys, ", ")} ({shortcut.about})`)
end
```

**Ver también:** [`shortcuts.<name>`](#shortcutsname).

### Todos los atajos

Estas son las teclas de cada atajo antes de que nadie las cambie. En un Mac,
`ctrl` aquí es `cmd`, y las teclas que van con `alt` y una letra o una cifra
van con `ctrl`; `shortcuts.all()` las da como son en cada sistema.

| Nombre | Dónde | Teclas de inicio | Qué hace |
|---|---|---|---|
| `palette` | `everywhere` | `ctrl+k` | Paleta |
| `settings` | `everywhere` | `ctrl+comma` | Ajustes |
| `new_session` | `everywhere` | `ctrl+t` | Sesión nueva |
| `close_session` | `everywhere` | `ctrl+w` | Cerrar esta sesión |
| `reconnect` | `everywhere` | `alt+r` | Reconectar |
| `switch_view` | `everywhere` | `ctrl+e` | Entre el juego y el editor |
| `data_view` | `everywhere` | `ctrl+d` | Entre el juego y los datos entrantes |
| `find` | `everywhere` | `ctrl+f` | Buscar en la salida |
| `messages` | `everywhere` | `alt+n` | Mensajes |
| `gauges` | `everywhere` | `alt+0` | Leer los indicadores |
| `where` | `everywhere` | `alt+m` | Dónde estoy |
| `explore` | `everywhere` | `alt+e` | Explorar el mapa |
| `map` | `everywhere` | `ctrl+m` | Mostrar u ocultar el mapa |
| `keys_help` | `everywhere` | `f1` | Decir las teclas que conviene saber |
| `next_zone` | `everywhere` | `f6` | Ir a la salida y a cada panel |
| `previous_zone` | `everywhere` | `shift+f6` | Volver por los paneles |
| `hush` | `everywhere` | `ctrl` | Silenciar la voz |
| `read_previous` | `reading` | `alt+up` | Leer la línea anterior |
| `read_next` | `reading` | `alt+down` | Leer la línea siguiente |
| `read_first` | `reading` | `alt+home` | Leer la primera línea |
| `read_last` | `reading` | `alt+end` | Leer la última línea |
| `read_recent_1` | `reading` | `alt+1` | Leer la línea más reciente |
| `read_recent_2` | `reading` | `alt+2` | Leer la 2.ª línea más reciente |
| `read_recent_3` | `reading` | `alt+3` | Leer la 3.ª línea más reciente |
| `read_recent_4` | `reading` | `alt+4` | Leer la 4.ª línea más reciente |
| `read_recent_5` | `reading` | `alt+5` | Leer la 5.ª línea más reciente |
| `read_recent_6` | `reading` | `alt+6` | Leer la 6.ª línea más reciente |
| `read_recent_7` | `reading` | `alt+7` | Leer la 7.ª línea más reciente |
| `read_recent_8` | `reading` | `alt+8` | Leer la 8.ª línea más reciente |
| `read_recent_9` | `reading` | `alt+9` | Leer la 9.ª línea más reciente |
| `copy_read` | `reading` | `alt+c` | Copiar la última línea leída |
| `follow_link` | `reading` | `alt+o` | Seguir el enlace de la última línea leída |
| `panel_previous` | `reading` | `alt+shift+left` | Elegir el panel de texto anterior para leer |
| `panel_next` | `reading` | `alt+shift+right` | Elegir el panel de texto siguiente para leer |
| `panel_read_previous` | `reading` | `alt+shift+up` | Leer la línea anterior del panel |
| `panel_read_next` | `reading` | `alt+shift+down` | Leer la línea siguiente del panel |
| `panel_read_first` | `reading` | `alt+shift+home` | Leer la primera línea del panel |
| `panel_read_last` | `reading` | `alt+shift+end` | Leer la última línea del panel |
| `panel_recent_1` | `reading` | `alt+shift+1` | Leer la línea más reciente del panel |
| `panel_recent_2` | `reading` | `alt+shift+2` | Leer la 2.ª línea más reciente del panel |
| `panel_recent_3` | `reading` | `alt+shift+3` | Leer la 3.ª línea más reciente del panel |
| `panel_recent_4` | `reading` | `alt+shift+4` | Leer la 4.ª línea más reciente del panel |
| `panel_recent_5` | `reading` | `alt+shift+5` | Leer la 5.ª línea más reciente del panel |
| `panel_recent_6` | `reading` | `alt+shift+6` | Leer la 6.ª línea más reciente del panel |
| `panel_recent_7` | `reading` | `alt+shift+7` | Leer la 7.ª línea más reciente del panel |
| `panel_recent_8` | `reading` | `alt+shift+8` | Leer la 8.ª línea más reciente del panel |
| `panel_recent_9` | `reading` | `alt+shift+9` | Leer la 9.ª línea más reciente del panel |
| `panel_copy_read` | `reading` | `alt+shift+c` | Copiar la última línea leída del panel |
| `history_previous` | `line` | `up` | Orden anterior |
| `history_next` | `line` | `down` | Orden siguiente |
| `history_search` | `line` | `ctrl+r` | Buscar en los comandos escritos antes |
| `map_north` | `map` | `up`, `8`, `numpad8`, `numpadup` | Seguir la salida norte |
| `map_south` | `map` | `down`, `2`, `numpad2`, `numpaddown` | Seguir la salida sur |
| `map_west` | `map` | `left`, `4`, `numpad4`, `numpadleft` | Seguir la salida oeste |
| `map_east` | `map` | `right`, `6`, `numpad6`, `numpadright` | Seguir la salida este |
| `map_northwest` | `map` | `7`, `numpad7`, `numpadhome` | Seguir la salida noroeste |
| `map_northeast` | `map` | `9`, `numpad9`, `numpadpageup` | Seguir la salida noreste |
| `map_southwest` | `map` | `1`, `numpad1`, `numpadend` | Seguir la salida suroeste |
| `map_southeast` | `map` | `3`, `numpad3`, `numpadpagedown` | Seguir la salida sureste |
| `map_up` | `map` | `pageup`, `numpadsubtract` | Seguir la salida hacia arriba |
| `map_down` | `map` | `pagedown`, `numpadadd` | Seguir la salida hacia abajo |
| `map_in` | `map` | `numpaddivide` | Seguir la salida hacia dentro |
| `map_out` | `map` | `numpadmultiply` | Seguir la salida hacia fuera |
| `map_again` | `map` | `space`, `numpad5`, `numpadclear` | Repetir la sala |
| `map_home` | `map` | `home` | Volver a tu sala |
| `map_back` | `map` | `backspace` | Volver a la sala anterior |
| `map_go` | `map` | `enter`, `numpadenter` | Ir a la sala elegida |
| `map_way` | `map` | `shift+enter` | Decir el camino a la sala elegida |
| `map_help` | `map` | `f1` | Decir todas las teclas del mapa |
| `map_info` | `map` | `ctrl+i` | Todo sobre la sala |
| `map_near` | `map` | `ctrl+l` | Qué hay cerca |
| `map_find` | `map` | `ctrl+f` | Buscar en el mapa |
| `map_mark` | `map` | `ctrl+b` | Marcar la sala |
| `map_note` | `map` | `ctrl+n` | Escribir una nota en la sala |
| `move_up` | `editor` | `alt+up` | Subir la regla |
| `move_down` | `editor` | `alt+down` | Bajar la regla |
| `move_out` | `editor` | `alt+left` | Sacar la regla de su carpeta |
| `rename` | `editor` | `f2` | Renombrar |
| `delete` | `editor` | `delete` | Eliminar |
| `select_all` | `editor` | `ctrl+a` | Seleccionar todos los elementos visibles |
| `undo` | `editor` | `ctrl+z` | Deshacer |
| `redo` | `editor` | `ctrl+y`, `ctrl+shift+z` | Rehacer |
| `panel_up` | `layout` | `alt+up` | Subir el panel |
| `panel_down` | `layout` | `alt+down` | Bajar el panel |
| `panel_side_back` | `layout` | `alt+left` | Mover el panel al lado anterior |
| `panel_side_on` | `layout` | `alt+right` | Mover el panel al lado siguiente |


## Hacer algo más tarde o cada cierto tiempo

Los temporizadores ejecutan una función pasado un rato, o una y otra vez.
Sirven, por ejemplo, para guardar cada minuto.

### `after(seconds, fn, options)`

Llama a `fn()` una vez, pasados esos segundos.

**Opciones:** `place` y `tags`.

**Devuelve:** lo que ha creado; `made:left()` da los segundos que faltan.

**Ver también:** [`wait(seconds)`](#waitseconds), para pararse dentro de una
función.

### `every(seconds, fn, options)`

Llama a `fn()` cada esos segundos, hasta que lo quitas. Mientras está
apagado, se salta sus turnos.

**Opciones:** `place` y `tags`.

**Devuelve:** lo que ha creado; `made:left()` da los segundos hasta la
siguiente vez.

**Ejemplo:**

```lua
local saving = every(60, function() send("save") end)
after(5, function() saving:remove() end)
```

## Esperar en mitad de un script

Cuando algo lleva varios pasos, escríbelos uno detrás de otro. Una función
puede pararse hasta que llegue una línea, baje un indicador o pasen unos
segundos, y después seguir. Es mucho más sencillo que disparadores y
temporizadores que se lanzan unos a otros.

### `wait(seconds)`

Para la función durante esos segundos. El resto del cliente y los demás
scripts siguen.

**Devuelve:** nada.

### `wait("line", pattern, options)`

Para hasta que una línea posterior coincide con `pattern`.

**Opciones:**

- `timeout`: cuántos segundos esperar como mucho.
- `prompt`: como en un [disparador](#triggerpattern-fn-options).

**Devuelve:** lo mismo que recibe un disparador: los grupos y la línea.
`nil` si antes pasa `timeout`.

**Ejemplo:**

```lua
alias("^cure$", function()
    send("drink potion")
    local m = wait("line", "^You feel better", {timeout = 3})
    if not m then return echo("It did nothing.") end
    wait(2)
    send("drink potion")
end)

trigger("^You are carrying:$", function()
    local things = {}
    for line in lines({till = "^$", timeout = 2}) do table.insert(things, line.text) end
    vars.things = things
end)
```

**Ver también:** [`lines`](#linesoptions), para varias líneas;
[`trigger`](#triggerpattern-fn-options).

### `wait(what, name, options)`

Para hasta que llega lo que escucharía `on(what, name, ...)`:
`wait("gauge", "HP", {under = 30, timeout = 10})`. Si no lleva nombre, no
des `name`.

**Opciones:** `timeout`, y las de [`on`](#onwhat-name-fn-options).

**Devuelve:** lo que `on` le pasaría a su función, o `true` si no le pasa
nada. `nil` si antes pasa `timeout`.

### `lines(options)`

Te da las siguientes líneas una a una, en un bucle `for`.

**Opciones:** para en lo primero que ocurra de esto, y tienes que dar al
menos una:

- `till`: una línea coincide con este patrón (esa línea no se incluye);
- `count`: han llegado tantas líneas;
- `timeout`: no llega ninguna en tantos segundos.

**Devuelve:** una función para el `for`, que da una
[línea](#campos-de-una-línea) cada vez.

**Ver también:** [`wait("line")`](#waitline-pattern-options).

### Dónde no se puede esperar

Una espera termina cuando su script se para o se vuelve a ejecutar.

No puedes esperar dentro de `on("send", ...)`, dentro de un módulo mientras
`require` lo carga, ni dentro de una función que se le pasa a otra, como la
que le das a `table.sort`.

Para esperar sin frenar el resto de tu código, envuélvelo:
`coroutine.wrap(function() ... end)()` ejecuta lo que tiene dentro, esperas
incluidas, en paralelo al resto.

## Recordar cosas (vars)

Las variables de `vars` se guardan con el perfil, así que sobreviven a
cerrar Muddie. Úsalas para un objetivo, una lista de amigos o cualquier cosa
que tus scripts no deban olvidar. Para lo que solo dura mientras un script
está cargado, mira [Para guardar datos](#para-guardar-datos).

### `vars`

Las variables del perfil, como una tabla. Son las mismas que ves en
**Variables** en el editor. Dale un valor a un campo para guardarlo, léelo
para tenerlo tal como está ahora y ponlo a `nil` para olvidarlo.

**Ejemplo:**

```lua
vars.target = "orc"          -- keeps it
send("kill " .. vars.target) -- reads it, as it is now
vars.target = nil            -- forgets it
for name, value in vars do echo(`{name} = {value}`) end
```

**Ojo:** una tabla guardada en una variable se guarda tal como estaba al
guardarla. Si después cambias la tabla, vuelve a guardarla:

```lua
local bag = vars.bag or {}
bag.gold = 5
vars.bag = bag
```

**Ver también:** [`on("var")`](#onvar-name-fn), [`expand`](#expandtext).

### `expand(text)`

Pone las variables en `text`, como en las órdenes de las reglas: `$name`
pasa a ser el valor de esa variable, `$name.key` un campo de una tabla y
`$name.1` un elemento de una lista.

**Devuelve:** el texto con las variables puestas.

### `on("var", name, fn)`

Llama a `fn(value, name)` cuando cambia la variable, la cambie un script, un
disparador o el editor.

**Devuelve:** lo que ha creado, como [`on`](#onwhat-name-fn-options).

Un indicador hecho en el editor también puede seguir una variable, sin
script.

## Mostrar el estado del personaje (indicadores)

Un indicador muestra una parte del estado de tu personaje: la vida como una
barra, una luz para «envenenado», una lista de efectos. Los indicadores van
en paneles de indicadores, y sus cambios se leen en voz alta.

Un indicador es ante todo un dato. Lo haya hecho quien lo haya hecho (el
editor, un plugin, un script o el MUD), cualquier script puede leerlo y
escucharlo, lo muestre un panel o no. Con lector de pantalla, los paneles de
indicadores se saltan; pulsa Alt+0 para oír lo que muestran, y los cambios se
dicen en cuanto ocurren.

### `gauges.set(name, gauge)`

Muestra un indicador, o lo actualiza si ya existe.

**Parámetros:** `gauge` es una tabla con estos campos:

| Campo | Qué es |
|---|---|
| `kind` | `"bar"` (lo normal), `"cells"`, `"number"`, `"light"`, `"text"` o `"effects"` |
| `value`, `max` | Su nivel. Una barra y las celdas necesitan los dos; un número, `value` |
| `on` | De una luz: si está encendida |
| `text` | De un texto: lo que dice |
| `items` | De los efectos: lo que tiene el personaje, como una lista (o una tabla por nombre) de nombres o de `{name = "Poisoned", about = "1 damage a second", colour = "red"}` |
| `colour`, `off_colour` | Un nombre o `#rrggbb`; `off_colour` es el color de una luz apagada |
| `steps` | El color que toma por debajo de cada porcentaje: `{{under = 60, colour = "yellow"}, {under = 30, colour = "red"}}` |
| `aloud`, `always` | Cómo se oyen sus cambios. `"usual"`: un nivel corta cuando baja de un paso, y los demás cambios esperan su turno. También `"lowest"`, `"wait"`, `"cut"` y `"never"` |
| `panel` | El panel de indicadores donde se muestra, por su nombre. Si no existe, se crea. Sin él, el primer panel de indicadores que haya |
| `place`, `tags` | Lo que lo enciende y lo apaga, como a un disparador |

**Devuelve:** nada.

**Ejemplo:**

```lua
gauges.set("HP", {value = 40, max = 100,
                  steps = {{under = 30, colour = "red"}}, aloud = "lowest"})
gauges.set("Poisoned", {kind = "light", on = true, colour = "green"})
gauges.set("States", {kind = "effects", items = {"Blessed", "Stunned"}})
```

Cuándo se dice un cambio de nivel:

- cuando baja de uno de sus pasos; y no otra vez hasta que vuelve a subir
  claramente por encima de ese paso;
- y de nuevo por cada décima parte más que baje estando por debajo de un
  paso.

Una luz, un texto o unos efectos se dicen siempre que cambia lo que dicen.

**Ver también:** [`on("gauge")`](#ongauge-name-fn-options),
[`gauges.get`](#gaugesgetname).

### `gauges.remove(name, options)`

Quita el indicador.

**Opciones:** `place` y `tags`, los mismos que se le dieron al ponerlo.

**Devuelve:** nada.

### `gauges.get(name)`

Da un indicador tal como está ahora, por su nombre o por su sitio.

**Devuelve:** una tabla con `name`, `place` y `kind`, y además, según su
tipo: `value`, `max` y `percent`; `on`; `text`; o `items`, una lista de
`{name, about, colour}`. `nil` si no existe; un indicador apagado cuenta como
si no existiera.

### `gauges.all()`

Da todos los indicadores.

**Devuelve:** una lista, cada indicador como lo da `gauges.get`.

### `on("gauge", name, fn, options)`

Llama a `fn(gauge, change)` cuando cambia el indicador. Si no das `name`, te
avisa de todos los indicadores. Que se cree un indicador no cuenta como
cambio.

**Parámetros:** `gauge` es cómo está ahora, como lo da `gauges.get`.
`change.was` es cómo estaba antes. De los efectos, `change.came` es la lista
de los que han llegado y `change.gone` la de los que se han ido.

**Opciones:**

- `under = 30`: solo cuando baja del 30 %. Salta una vez, y no vuelve a
  saltar hasta que el indicador haya subido un 5 % por encima.
- `over = 90`: cuando llega al 90 %.
- `place`, `tags`.

**Devuelve:** lo que ha creado, como [`on`](#onwhat-name-fn-options).

**Ejemplo:**

```lua
on("gauge", "HP", function() sounds.play("alarm.ogg") end, {under = 30})

on("gauge", "States", function(gauge, change)
    for _, state in change.came do speak(state.name) end
end)
```

## Paneles de texto y canales

Un panel de texto es un recuadro junto a la salida donde un script puede
poner texto: una lista de botín, un mapa que dibuja el servidor, un registro
de un tipo de mensaje. Los canales son los canales de charla del MUD, cada
uno en su pestaña, para que la conversación no se pierda entre la pelea.

### `panels.set(name, text)`

Sustituye todo lo que hay en un panel de texto por `text`. Úsalo para algo
que el servidor manda listo para mostrar, como un mapa que dibuja él.

**Parámetros:** `name` es el panel. Se crea si no hay ninguno con ese nombre.
`"map"` es siempre el panel del mapa, se llame como se llame, y `"channels"`
es el panel de canales.

**Devuelve:** nada.

**Ver también:** [`panels.add`](#panelsaddname-text-options), para añadir sin
sustituir.

### `panels.clear(name)`

Vacía el panel.

**Devuelve:** nada.

### `panels.add(name, text, options)`

Añade `text` al final de un panel, conservando lo que tenía, y lo lee en voz
alta tras el nombre del panel.

**Opciones:** `aloud` y `always`.

**Devuelve:** nada.

**Ejemplo:**

```lua
trigger("^You find (.+)\\.$", function(m) panels.add("Loot", m[1]) end)

trigger("^(\\w+) chats: (.*)$", function(m, line)
    channels.say("chat", `{m[1]}: {m[2]}`)
    line:hide()
end)
```

**Ver también:** [`channels.say`](#channelssayname-text-options),
[`line:to_panel`](#lineto_panelname).

### `panels.size(name)`

Dice cuántos caracteres caben ahora mismo en un panel. Sin nombre, el panel
del mapa.

**Devuelve:** `{columns, rows}`, o `nil` si el panel aún no ha salido en
pantalla.

**Ver también:** `on("panel")` en [`on`](#onwhat-name-fn-options), que te
avisa cuando cambia el tamaño.

### `channels.say(name, text, options)`

Añade `text` como dicho en el canal `name` del MUD, que tiene su propia
pestaña en un panel, y lo lee en voz alta tras el nombre del canal.

**Opciones:**

- `panel`: el panel donde va el canal, por su nombre. Si ya hay uno con ese
  nombre, se usa tal cual; si no, se crea. Sin él, el panel de canales.
- `aloud` y `always`. Usa `aloud = "never"` para lo que se dijo antes de que
  llegara el jugador.

**Devuelve:** nada.

Si dos scripts dicen el mismo mensaje a la vez, cuenta como uno.

**Ver también:** [`panels.add`](#panelsaddname-text-options).

### `channels.open(name, options)`

Crea la pestaña del canal antes de que nadie hable en él.

**Opciones:** `panel`, como en `channels.say`.

**Devuelve:** nada.

### `channels.close(name)`

Quita la pestaña del canal, como cuando el personaje deja el canal.

**Devuelve:** nada.

### `channels.all()`

Da los canales que hay.

**Devuelve:** una lista con sus nombres.

## Reproducir sonidos

Los sonidos dan pistas rápidas sin palabras: un pitido cuando te golpean,
una alarma cuando tienes poca vida, el viento de un bosque. Pon tus archivos
en la carpeta `sounds` del perfil. Muddie lee WAV, OGG, MP3, FLAC y M4A, y
los reproduce igual en Linux, Windows y Mac.

### `sounds.play(file, options)`

Reproduce un sonido, por su ruta o por su nombre en la carpeta `sounds` del
perfil.

**Opciones:**

- `volume`, de 0 a 1, y `loop`, para que se repita una y otra vez.
- `start` y `finish`: dónde empieza y dónde acaba dentro del sonido, en
  segundos.
- `fade_in` y `fade_out`: cuántos segundos tarda en subir, y en apagarse
  antes de `finish`.
- `pan`: dónde se oye, de -1, todo a la izquierda, a 1, todo a la derecha; 0,
  en el centro, que es como suena si no lo das. Pon el sonido donde está lo
  que suena: una puerta que se abre al oeste, a la izquierda.
- `group`: el grupo de sonidos en el que va, como `"ambient"` o `"alerts"`,
  cuyo volumen pone [`sounds.volume`](#soundsvolumegroup-volume).

**Devuelve:** el sonido:

- `sound:stop()` lo para.
- `sound:playing()` dice si sigue sonando.
- `sound:volume(volume)` lo sube o lo baja mientras suena, de 0 a 1.

**Ejemplo:**

```lua
trigger("^You are hit", function() sounds.play("hit.ogg", {volume = 0.5}) end)
```

```lua
-- The wind of a forest, quietly, on the left; louder as the fight ends.
local wind = sounds.play("wind.ogg", {loop = true, volume = 0.3, pan = -0.6, group = "ambient"})
trigger("^You won the fight", function() wind:volume(0.6) end)
```

**Ver también:** [`sounds.volume(group, volume)`](#soundsvolumegroup-volume).

### `sounds.volume(group, volume)`

Pone el volumen de un grupo de sonidos, de 0 a 1: los que suenan ya y los que
suenen después. Sin `volume`, dice cuánto está. Los sonidos que pone el
propio MUD van en el grupo `"mud"`.

**Devuelve:** el volumen del grupo ahora; 1 si nadie lo cambió.

Cuando se va el script que lo cambió, el grupo vuelve a su volumen de antes.
Para guardar lo que elige el jugador, guárdalo en [`vars`](#vars) y vuelve a
ponerlo al arrancar el script.

**Ejemplo:**

```lua
-- "ambient 20" sets the background sounds to 20 %.
alias("^ambient (%d+)$", function(m)
    vars.ambient = tonumber(m[1]) / 100
    sounds.volume("ambient", vars.ambient)
end)
```

### `sounds.fetch(names)`

Descarga en segundo plano los sonidos de un plugin que los tiene en la web
(su `plugin.toml` dice dónde, con `sounds`), para que ninguno llegue tarde la
primera vez que suena. Solo baja los que no están guardados o han cambiado;
si son más de tres, lo avisa al empezar y cuando están todos.

**Parámetros:** `names`, una lista con los nombres de los sonidos dentro de
esa carpeta.

**Devuelve:** nada.

**Ejemplo:**

```lua
sounds.fetch({"Combate/Alerta vida 30.wav", "Comunicaciones/Tel.wav"})
```

Un script del plugin los reproduce como cualquier otro, con
[`sounds.play`](#soundsplayfile-options): primero el archivo propio del
jugador con ese nombre, si lo tiene; si no, el guardado; si no, se descarga
y suena en cuanto llega. No se descarga nada mientras **Descargar sonidos**
esté apagado en el perfil.

### `sounds.stop()`

Para todos los sonidos.

**Devuelve:** nada.

## Hablar con el MUD por detrás (GMCP, MSDP)

Muchos MUD mandan datos que nunca ves en el texto, como tu vida o la sala en
la que estás. Usan unos protocolos llamados GMCP y MSDP. Con estos datos
puedes llenar indicadores y el mapa sin leer el texto. Para reaccionar cuando
llegan datos, usa `on("gmcp", ...)` u `on("msdp", ...)` (ver
[`on`](#onwhat-name-fn-options)).

### `gmcp.get(package)`

Da los últimos datos recibidos de exactamente ese paquete.

**Devuelve:** los datos, o `nil` si no ha llegado ninguno.

### `gmcp.send(package, data)`

Manda un mensaje GMCP al servidor.

**Devuelve:** nada.

### `gmcp.support(module, ...)`

Le pide al servidor módulos GMCP, como `"Name version"`, además de los que
pide el propio cliente (`Core` y `Client.Media`). Algunos servidores solo
mandan lo que se les pide. La petición dura mientras el script está cargado:
al apagarlo o quitarlo, se le dice al servidor que ya no los quiere.

**Devuelve:** nada.

**Ejemplo:**

```lua
gmcp.support("Char 1", "Room 1")
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("HP", {value = vitals.hp, max = vitals.maxhp})
end)

msdp.report("HEALTH", "HEALTH_MAX")
on("msdp", "HEALTH", function(value) echo("health " .. value) end)
```

### `gmcp.merge(package, ...)`

Para los paquetes de los que el servidor solo manda lo que cambia, como un
`Char.Status` con solo la vida. Lo que llega se mezcla con lo que guardan
`gmcp.get` y los indicadores, también las tablas de dentro. Quien escucha
sigue recibiendo solo lo que llegó.

**Devuelve:** nada.

### `msdp.get(variable)`, `msdp.send(command, ...)`

Lo mismo que `gmcp.get` y `gmcp.send`, para MSDP.

**Devuelve:** `msdp.get` devuelve el valor, como texto; `msdp.send`, nada.

### `msdp.report(variable, ...)`

Le pide al servidor que mande estas variables MSDP cada vez que cambien,
mientras el script está cargado.

**Devuelve:** nada.

### `mssp.get()`

Da el estado MSSP del servidor.

**Devuelve:** una tabla de textos.

## Reaccionar a otros sucesos (on)

`on` ejecuta una función cuando pasa algo que no es una línea de texto:
llegan datos GMCP, cambia una variable, te conectas, otro script dice algo.

### `on(what, name, fn, options)`

Llama a `fn` cuando pasa `what`. Para los sucesos que no llevan `name`,
escribe `on(what, fn)`.

**Parámetros:**

| `what` | `name` | `fn` recibe |
|---|---|---|
| `"gmcp"` | un paquete; cubre también los que están debajo (`"Char"` cubre `Char.Vitals`) | los datos, y el paquete |
| `"msdp"` | una variable | el valor (un texto), y la variable |
| `"var"` | una variable de [`vars`](#vars) | el valor, y el nombre |
| `"gauge"` | un indicador, o ninguno para todos | el indicador, y cómo ha cambiado (ver [`on("gauge")`](#ongauge-name-fn-options)) |
| `"channel"` | un canal, o ninguno para todos | `{channel, text}` |
| `"emit"` | lo que dice otro script | lo que dijo, y el nombre |
| `"send"` | ninguno | cada orden que va camino del MUD (ver [`on("send")`](#onsend-fn)) |
| `"room"` | ninguno | la sala en la que está ahora el jugador, como la da [`map.room`](#maproomplace), y el id de la anterior (`nil` la primera vez) |
| `"connect"`, `"disconnect"` | ninguno | nada |
| `"focus"` | ninguno | si la sesión pasa a estar delante, en una ventana que está delante |
| `"reader"` | ninguno | si un lector de pantalla pasa a leer Muddie |
| `"mxp"` | ninguno | `{name, value}`, cuando el MUD nombra un valor con MXP |
| `"panel"` | ninguno | `{name, map, columns, rows}`, cuando un panel que muestra un dibujo se queda con un tamaño nuevo; `map` es true para el panel del mapa |

**Un paso a otra sala** se avisa en cuanto el mapa sabe que el jugador está allí, lo sepa como lo sepa: el plugin estándar, un script o una caminata.

```lua
-- The note of a room is said as you come into it.
on("room", function(room)
    if room.note ~= "" then speak(room.note) end
end)
```

**Opciones:** `place` y `tags`; para un indicador, también `under` y `over`.

**Devuelve:** lo que ha creado, igual que un disparador (ver
[lo que devuelven](#lo-que-devuelven-trigger-alias-macro-after-every-y-on)).

**Ejemplo:**

```lua
on("connect", function() send("look") end)
```

Si `on` no conoce `what`, el error dice lo que puede escuchar. Los cambios de
una variable, un indicador o un canal, y los mensajes de otro script, se
avisan cuando termina el script que los hizo. Así siempre recibes el cambio
entero, no la mitad.

**Ver también:** [`wait(what, name)`](#waitwhat-name-options), para esperar
uno de estos sucesos dentro de una función.

### `emit(name, data)`

Manda un mensaje a los demás scripts, que lo escuchan con
`on("emit", name, fn)`. Así cuenta un plugin lo que a otros les puede
interesar.

**Devuelve:** nada.

**Ejemplo:**

```lua
-- In one script:
emit("fight", {with = "orc"})
-- In another:
on("emit", "fight", function(data) speak(`Fighting {data.with}`) end)
```

## Encender y apagar cosas

Todo lo que crea un script se puede encender y apagar: en una pelea quieres
los disparadores y las teclas de combate, y fuera de ella quizá no. Dales un
sitio o etiquetas y enciende o apaga grupos enteros de una vez.

### Opciones, sitios y etiquetas

Las opciones van siempre al final, en una tabla:
`trigger(pattern, fn, {once = true})`. Una opción que una función no admite
da un error con las que sí admite. Muddie nunca ignora una opción sin avisar.

Los disparadores, alias, teclas, temporizadores, escuchas e indicadores
admiten dos opciones:

- `place`, como `"Folder/Name"`: dónde está. Apagar una carpeta apaga todo lo
  que hay dentro.
- `tags`, una lista de etiquetas o una sola. Apagar una etiqueta apaga todo
  lo que la lleva.

Algo solo funciona mientras su sitio, todas las carpetas por encima y todas
sus etiquetas están encendidos.

Las reglas del editor tienen como sitio su carpeta y su nombre:
`Combat/Fight back`. Dos reglas con el mismo nombre llevan un número:
`Combat/Fight back #2`. El sitio de un script es su ruta entre los scripts
del perfil, sin extensión: `Mapping/rooms`.

### Lo que devuelven `trigger`, `alias`, `macro`, `after`, `every` y `on`

Cada una devuelve lo que ha creado, y puedes guardarlo en una variable:

| Método | Qué hace |
|---|---|
| `made:enable()`, `made:disable()`, `made:toggle()` | Lo encienden o lo apagan; devuelven si queda encendido |
| `made:enabled()` | Si está encendido |
| `made:remove()` | Lo quita; devuelve si estaba |
| `made:left()` | De un temporizador, los segundos que le quedan |

### `triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts`

Encienden o apagan las cosas de un tipo por su sitio, o una carpeta entera
de una vez, para la sesión. Vale tanto si están hechas en un script como en
el editor.

**Funciones:** cada uno tiene `enable(place)`, `disable(place)`,
`toggle(place)`, `enabled(place)` y `list()`.

**Devuelve:** `toggle` y `enabled` devuelven si está encendido; `list`, los
sitios que hay; las demás, nada.

**Ejemplo:**

```lua
triggers.disable("Combat")
macros.enable("Combat/Kick")
local on = scripts.toggle("Mapping")
```

### `tags`

Las mismas funciones, por etiqueta: `tags.disable("combat")` apaga todo lo
que lleva esa etiqueta, sea del tipo que sea.

### Lo que se va con un script

Cuando un script se apaga, se quita o se vuelve a ejecutar, todo lo que creó
se va con él:

- sus disparadores, alias, teclas, temporizadores, escuchas e indicadores;
- el texto que puso en paneles y los canales que abrió;
- sus sonidos, que se paran;
- lo que ajustó del mapa y de la sesión, que vuelve a como estaba:
  `map.learn`, `map.directions`, `map.names`, `map.native`, `map.persist`,
  `map.language`, `session.separator`, `session.completions`, `gmcp.merge`,
  `gmcp.support` y `msdp.report`.

## La sesión y la línea de órdenes

`session` le cuenta a tu script cómo está la conexión: a dónde está
conectada, si hay un lector de pantalla, qué hay escrito en la línea de
órdenes. Unas pocas funciones ayudan a escribir y a probar.

### Campos de `session`

| Campo | Qué es |
|---|---|
| `session.connected` | Si la sesión está conectada |
| `session.host` | El servidor al que se conecta |
| `session.profile` | El nombre de su perfil, o `nil` |
| `session.reader` | Si un lector de pantalla está leyendo Muddie |
| `session.focused` | Si la sesión está delante, en una ventana que está delante |
| `session.input` | Lo escrito en la línea de órdenes. Dale un valor para poner texto ahí. Es `nil` mientras se escribe una contraseña, y entonces tampoco se puede cambiar |
| `session.reading` | Si el juego se lee en voz alta. Ponlo a `false` para callarlo, como hace un modo silencioso; `true` o `nil` vuelve a leerlo. El jugador también puede cambiarlo; cuando se va el script que lo puso, el juego se vuelve a leer |
| `session.separator` | Lo que separa varias órdenes escritas en una línea: `";"` si no lo cambias. `false` quita la separación; `nil` vuelve a `";"`. Escríbelo dos veces para mandarlo tal cual |

### `session.completions(name, words)`

Añade palabras para que Tab las ofrezca, además de las que se ven en la
salida. Cada `name` tiene su propia lista; `session.completions(name)` quita
esa lista.

**Devuelve:** nada.

**Ejemplo:**

```lua
session.completions("friends", {"Ana", "Luis"})
if session.reader then echo("A screen reader is reading Muddie.") end
```

### `session.feed(text)`

Trata `text` como si lo hubiera mandado el MUD. Sirve para probar qué hace tu
script con una línea.

**Devuelve:** nada.

### `output.last(count)`

Da lo que dicen las últimas `count` líneas de la salida. Sin `count`, solo
la última.

**Devuelve:** una lista de textos, la más nueva al final.

## Hacer el mapa y recorrerlo

Muddie va haciendo un mapa de las salas que visitas y puede llevarte a
cualquiera de ellas. El plugin **GMCP y MSDP estándar** hace el mapa con
`Room.Info` de GMCP y `ROOM` de MSDP. Para un MUD que describe las salas de
otra forma, o solo en el texto, un script alimenta el mapa él mismo y puede
apagar el cargador estándar. Para elegir la función que encaja con tu MUD,
mira [Para seguir la sala del jugador](#para-seguir-la-sala-del-jugador).

### Salas y sitios

Una sala se da como una tabla con `id`. También puede tener `name`, `area`,
`exits` (cada dirección con el id de una sala), `x`, `y`, `z` y `colour` (un
nombre, `"#rrggbb"` o un número de la paleta). Una sala dada sin color
conserva el que tenía.

Un id es un número, o el texto con el que el servidor nombra la sala. Puedes
pasar tal cual lo que un MUD manda en `Room.Info` o `ROOM`: `num` o `vnum`
valen como `id`, `zone` como `area` y `coords` como posición, en mayúsculas
o minúsculas.

Algunas funciones piden un sitio (`map.room`, `map.here`, `map.mark`,
`map.edit_room`, `map.remove_room`, `map.way`, `map.go`). Un sitio es el id
de una sala, o una marca o un nombre de sala tal como los escribiría el
jugador. Si coinciden varias salas, se usa la más cercana.

### Ajustes del mapa

Son campos: se les da un valor, y se leen. Darle `nil` a uno lo deja como
estaba.

| Campo | Qué hace |
|---|---|
| `map.native` | `false` apaga el cargador estándar mientras el script está cargado |
| `map.persist` | `false` guarda el mapa solo en memoria, para servidores que mandan el mapa ellos |
| `map.learn` | `true` hace que el mapa aprenda mientras el jugador se mueve (ver [`map.learn`](#maplearn)) |
| `map.language` | El idioma en que se juega el MUD, por código o nombre (`"es"`). El mapa usa las palabras de ese idioma para las direcciones |
| `map.directions` | Palabras propias del MUD para las direcciones, además de las de su idioma: `{n = {"nte"}, s = "sur"}`. La primera de cada una es la que mandan los caminos |
| `map.names` | Cómo escribe el MUD los nombres de sus salas: `{region = ":", exits = "[]"}`. Primero va la región, hasta el texto de `region`; después las salidas, entre los caracteres de `exits` |

### Seguir al jugador

#### `map.visit(room)`

Dice que el jugador está ahora en `room`, y añade o actualiza la sala.

**Devuelve:** el id de la sala.

**Ejemplo:**

```lua
-- A MUD with no GMCP: the room from its title and exits lines.
map.native = false
local title
trigger("^\\[ (.+) \\]$", function(m) title = m[1] end)
trigger("^Exits: (.+)$", function(m)
    local exits = {}
    for direction, to in m[1]:gmatch("(%a+)=(%a+)") do exits[direction] = to end
    map.visit({id = title, name = title, area = "town", exits = exits})
end)
```

**Ver también:** [`map.here`](#maphereplace), [`map.arrive`](#maparrivename-exits-came).

#### `map.set_room(room)`

Añade o actualiza una sala sin mover al jugador a ella.

**Devuelve:** el id de la sala.

#### `map.here(place)`

Dice que el jugador está en esa sala, que ya tiene que estar en el mapa.

**Devuelve:** si el mapa tiene la sala.

**Ver también:** [`map.visit`](#mapvisitroom).

#### `map.arrive{name, exits, came}`

Para servidores que no dicen en qué sala estás. Dice que el jugador está en
una sala con este nombre y estas salidas, a la que ha llegado por la salida
`came`.

**Devuelve:** el id de la sala, o `nil` mientras el mapa no está seguro de
cuál es.

**Ver también:** [`map.add`](#mapaddname-exits-came), [`map.learn`](#maplearn),
[`map.candidates`](#mapcandidates).

#### `map.add{name, exits, came}`

Para servidores que no dicen en qué sala estás. Úsalo cuando el jugador ha
ido por `came` a un sitio donde el mapa no tiene sala. Busca la sala entre
las que tiene el mapa, o crea una nueva. El jugador puede deshacer las salas
añadidas así.

**Devuelve:** el id de la sala y `"found"` o `"made"`. Si no puede, `nil` y
el motivo:

- `"lost"`: el mapa no sabe seguro dónde estaba el jugador;
- `"known"`: esa salida ya lleva a una sala;
- `"alike"`: podría ser cualquiera de varias salas;
- `"unnamed"`.

#### `map.learn`

Un ajuste: ponlo a `true` y el mapa aprende de las salas que recibe
`map.arrive`. Nunca aprende de verlas una sola vez, ni desde una sala en la
que no está seguro de que estuviera el jugador. El mapa:

- añade las salas que no tiene a donde va el jugador por la brújula;
- corrige el nombre de las salas que tenía con otro, cuando el siguiente
  movimiento lo confirma;
- añade las salidas que una sala muestra y al mapa le faltan;
- hace que una salida lleve a la sala a la que lleva de verdad.

Cada cambio se avisa, y se puede deshacer como cualquier cambio que el mapa
hace por su cuenta.

#### `map.candidates()`

Da las salas en las que puede estar el jugador mientras el mapa no sabe en
cuál.

**Devuelve:** una lista de ids de sala; vacía cuando el mapa lo sabe, o
cuando no tiene ni idea.

#### `map.failed()`

Le dice al mapa que el servidor ha dicho que un movimiento ha fallado. Los
movimientos ya mandados cuentan como no hechos, y un camino se para.

**Devuelve:** nada.

### Caminar

#### `map.way(to, from)`

Calcula el camino de `from` a `to`. Sin `from`, sale de donde está el
jugador.

**Devuelve:** una lista de órdenes, o `nil` si no hay camino.

#### `map.go(to)`

Lleva al jugador hasta allí, esperando a que cada movimiento se confirme
antes de dar el siguiente.

**Devuelve:** nada.

**Ejemplo:**

```lua
alias("^home$", function() map.go("home") end)
```

#### `map.stop()`

Para un camino, como hace **Dejar de ir**.

**Devuelve:** nada.

### Consultar

#### `map.room(place)`

Da una sala; sin `place`, la del jugador.

**Devuelve:** una tabla con `id`, `name`, `area`, `x`, `y`, `z`, `exits`
(dirección a id de sala), `cost` (lo que le cuesta a un camino entrar en
ella), `avoid`, `colour` (`"#rrggbb"` o `nil`), `note` (`""` si no tiene),
`renamed` (su nombre se puso en lugar del del mapa), `added` (la añadió el
mapa por su cuenta), `shape` y `symbol`. `nil` si no existe.

#### `map.find(text, options)`

Busca salas como lo hace **Buscar salas…**: por marca, nota o nombre.

**Opciones:** `exact = true`: solo las salas que se llaman exactamente
`text`, como las nombra el MUD.

**Devuelve:** una lista de ids de sala, la más cercana primero.

#### `map.exit(from, direction)`

Da una salida de una sala.

**Devuelve:** una tabla con `to`, `cost`, `blocked` (fuera de los caminos),
`command` y `custom` (una que añadió el jugador); `nil` si no existe.

#### `map.direction(word)`

Dice qué dirección significa una palabra. La palabra puede estar en el
idioma del MUD o en inglés.

**Devuelve:** la dirección como la guarda el mapa (`"n"`, `"ne"`... `"u"`,
`"d"`, `"in"`, `"out"`), o `nil` si no es una dirección.

#### `map.count()`, `map.areas()`, `map.id(text)`

**Devuelve:** `map.count()`, cuántas salas hay; `map.areas()`, una lista con
los nombres de las áreas; `map.id(text)`, el número que se usa para un id de
texto.

#### `map.marks()`

Da las marcas.

**Devuelve:** una tabla de marcas por nombre, cada una con el id de su sala.

### Cambiar el mapa

#### `map.mark(name, place)`

Marca una sala con `name`, como hace **Marcar…**. `map.mark(name)` quita la
marca.

**Devuelve:** si existía la sala (o la marca).

#### `map.edit_room(place, changes)`

Cambia solo los campos que den `changes`, y deja el resto.

**Parámetros:** `changes` puede tener:

- `name`: un nombre en lugar del del mapa. Se mantiene aunque el servidor
  vuelva a describir la sala o se vuelva a importar el mapa.
- `note`, `cost`, `avoid`, `x`, `y`, `z`, `colour`.
- `shape`: `"square"`, `"circle"`, `"diamond"` o `"hexagon"`.
- `symbol`: uno o dos caracteres escritos en la sala.

`false` devuelve el color o la forma del mapa, quita el coste o quita el
símbolo.

**Devuelve:** si existía la sala.

#### `map.set_exit(from, direction, to, options)`

Añade una salida propia del jugador, o cambia a dónde lleva una salida. Con
`to` a `nil`, solo cambia lo que den `options`.

**Opciones:** `cost`, `blocked` o `command` (como `"open door;n"`).

**Devuelve:** si existían la sala y la salida.

#### `map.remove_exit(from, direction)`, `map.remove_room(place)`

Quitan una salida o una sala.

**Devuelve:** si existía.

#### `map.clear()`

Olvida todas las salas.

**Devuelve:** nada.

#### `map.tidy(area)`

Vuelve a dibujar un área desde cero, si así queda mejor; sin `area`, la del
jugador.

**Devuelve:** cuántas salas se han movido. Si no ha movido ninguna, ha
dejado el área como estaba, y devuelve como segundo valor
`{now = {wrong_exits, stacked}, ["then"] = {wrong_exits, stacked}}`.

#### `map.undo()`

Deshace el último cambio que el mapa hizo por su cuenta.

**Devuelve:** si había alguno.

#### `map.show_area(area)`

Muestra un área en el mapa, solo para mirarla, sin ir allí.

**Devuelve:** nada.

### El mapa que publica un MUD

#### `map.import(address)`

Importa un mapa hecho con Mudlet desde `address` (un archivo `.dat`, o un
zip con uno, en la web o en disco), en lugar del mapa actual. Lo que puso el
jugador se conserva: salas renombradas, notas, marcas, salas que evitar,
salidas propias y las salas que añadió el mapa.

**Devuelve:** nada.

#### `map.newer(address, fn)`

Comprueba, sin descargarlo, si el mapa de `address` ha cambiado desde que se
importó, y llama a `fn()` si es así. Úsalo para avisar al jugador, en vez de
cambiarle el mapa sin preguntar.

**Devuelve:** nada.

#### `map.action(name, fn, options)`

Añade una entrada llamada `name` al menú del mapa y a la paleta, que ejecuta
`fn()`. Dura mientras el script está cargado; las entradas de un plugin
salen bajo el nombre del plugin. `map.action(name)` quita la entrada.

**Parámetros:** `name` se puede dar por idioma, como en [`tr`](#trtexts).

**Opciones:** `checked = false` hace de la entrada un interruptor: al
elegirla se llama a `fn(on)` con su nuevo estado.

**Devuelve:** nada.

## Compartir código entre scripts

Cuando varios scripts necesitan las mismas funciones de ayuda, ponlas en un
módulo y cárgalo con `require`. Así comparten código los scripts de un
plugin.

### `require(name)`

Carga `name.luau`, o `name/init.luau`. Busca primero en la carpeta del
script que lo pide, y después en las de los demás scripts cargados. Un
módulo se carga una sola vez.

**Devuelve:** lo que devuelve el módulo.

**Ejemplo:**

```lua
-- util.luau
local util = {}
function util.percent(value, max) return math.floor(value * 100 / max) end
return util
```

```lua
-- another script, beside it
local util = require("util")
echo(`{util.percent(30, 120)} %`)
```

**Ojo:** en la carpeta de un plugin, todos los archivos `.luau` se ejecutan
también como scripts, módulos incluidos. Así que un módulo solo debe devolver
lo que tiene, sin crear disparadores, teclas ni nada más.

## Cuando algo falla

- Un error se muestra como `script.luau:12: what is wrong`, y el editor lo
  guarda. Llamar a algo que no existe dice su nombre:
  `there is no function gauges.sett`.
- Una opción que una función no admite, algo que `on` no conoce, un nombre de
  tecla o un color que no existen: cada uno da un error con lo que sí vale.
- [`muddie.d.luau`](muddie.d.luau) tiene todas las funciones con sus tipos.
  Cárgalo en un editor con luau-lsp (como archivo de definiciones), o pásaselo
  a `luau-analyze`. Lo que esté mal escrito se ve mientras escribes. Con
  `--!strict` al principio del script, también se ve una línea comparada con
  un texto.
- `muddie --debug` escribe en la salida estándar todo lo que pasa:
  - el texto: `TEXT` para una línea, `PROMPT` para una que el MUD marca como
    prompt, `PARTIAL` para texto que aún no ha terminado;
  - lo que se manda: `SENT`;
  - el GMCP y los demás protocolos;
  - lo que los scripts mandan a `log`, y sus errores: `SCRIPT`.

  `muddie --debug | tee muddie.log` lo guarda en un archivo. Las contraseñas
  no se muestran nunca.

## Lo que los scripts no pueden hacer

Estos límites son a propósito. Los scripts no pueden:

- Crear sus propias ventanas, etiquetas o barras. Usa un panel de texto o un
  indicador: se oyen además de verse.
- Leer ni escribir archivos, arrancar programas ni descargar lo que quieran.
  Un script de otra persona no debe poder hacer daño. Usa `vars` para guardar
  datos.
- Añadir sus propias órdenes o ajustes a la paleta. Usa alias, teclas y
  `vars`.
- Ordenar los disparadores por prioridad. Se ejecutan en el orden en que se
  crearon, y `wait` sigue lo que llega en varias líneas.
- Guardar datos propios en cada sala del mapa.
