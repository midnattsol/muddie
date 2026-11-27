# Referencia de scripts de Muddie

English: [scripting.md](scripting.md). Para hacer un plugin, empieza por
[plugins.es.md](plugins.es.md). Todas las funciones con sus tipos, para un
editor que compruebe Luau: [muddie.d.luau](muddie.d.luau).

Los scripts se escriben en [Luau](https://luau.org), un dialecto de Lua 5.1
rápido y con tipos. Hay una sola API para todos: lo que un script muestra se ve,
lo que dice se oye, y el mismo script sirve a quien mira la pantalla y a quien
escucha con un lector de pantalla. Muddie da los datos y las herramientas; qué
se muestra y qué se dice lo deciden los scripts.

Los scripts corren aislados: no pueden leer ni escribir archivos, arrancar
programas ni cargar código de fuera de las carpetas de scripts, así que un
script que te pase otra persona no puede hacer daño. Uno que tarde más de medio
segundo seguido se para con un error, y el cliente sigue.

## Índice

1. [Primeros pasos](#primeros-pasos)
2. [Mostrar y decir](#mostrar-y-decir)
3. [Lo que se oye](#lo-que-se-oye)
4. [Disparadores](#disparadores)
5. [La línea](#la-línea)
6. [Alias, teclas y temporizadores](#alias-teclas-y-temporizadores)
7. [Lo que crea un script, y encenderlo o apagarlo](#lo-que-crea-un-script-y-encenderlo-o-apagarlo)
8. [Esperar](#esperar)
9. [Escuchar: on](#escuchar-on)
10. [Variables](#variables)
11. [Indicadores](#indicadores)
12. [Paneles y canales](#paneles-y-canales)
13. [Sonidos](#sonidos)
14. [La sesión y la línea de órdenes](#la-sesión-y-la-línea-de-órdenes)
15. [Datos del MUD](#datos-del-mud)
16. [El mapa](#el-mapa)
17. [Módulos](#módulos)
18. [Errores, y cómo comprobar un script](#errores-y-cómo-comprobar-un-script)
19. [Lo que no hay](#lo-que-no-hay)

## Primeros pasos

### Dónde están los scripts

Todas las rutas son dentro de la carpeta de configuración: `~/.config/muddie`
en Linux, `%APPDATA%\muddie` en Windows.

| Archivo | Cuándo se ejecuta |
|---|---|
| `init.luau` | En todas las sesiones |
| `profiles/<profile>/scripts/*.luau` | En ese perfil, por orden de nombre; los de un plugin, en la carpeta del plugin |
| los archivos que se dan con `--script` | En todas las sesiones, al final |

En la paleta (Ctrl+K), **Reiniciar scripts** los vuelve a ejecutar tras
editarlos, y **Ejecutar Luau…** ejecuta una línea de Luau en el momento. En el
editor, **Scripts** tiene cada script, para escribirlo, ejecutarlo y encenderlo
o apagarlo.

### Un primer script

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

### Qué conserva un script, y cuándo lo pierde

Lo que un script guarda en sus propias variables dura mientras está cargado:

- **Se conserva** cuando se pierde la conexión y se vuelve a hacer (a los
  scripts solo se les avisa, con `on("disconnect")` y `on("connect")`), y cuando
  se cambia y se guarda otro script o una regla.
- **Se pierde** cuando el propio script se vuelve a ejecutar, al guardarlo o con
  su botón de ejecutar; cuando se vuelve a ejecutar todo (**Reiniciar scripts**,
  otro perfil, encender o apagar un plugin); y al cerrar Muddie. Un script que
  se vuelve a ejecutar empieza desde su primera línea sin nada: ahí es donde hay
  que pedirle al MUD lo que ya había mandado.

Lo que tenga que sobrevivir a todo eso va en [`vars`](#variables).

Lo que un script crea y ajusta se va con él cuando se apaga, se quita o se vuelve
a ejecutar: sus disparadores, alias, teclas, temporizadores, escuchas e
indicadores; el texto que puso en paneles y los canales que abrió; sus sonidos,
que se paran; y lo que ajustó del mapa y de la sesión (`map.learn`,
`map.directions`, `map.names`, `map.native`, `map.persist`, `map.language`,
`session.separator`, `session.completions`, `gmcp.merge`, `gmcp.support`,
`msdp.report`), que vuelve a como estaba.

### Opciones, sitios y etiquetas

Las opciones van siempre al final, en una tabla:
`trigger(pattern, fn, {once = true})`. Una opción que una función no admite es
un error que dice cuáles admite; nunca se ignora sin avisar.

Todo lo que se puede encender y apagar admite `place` y `tags`:

- `place`, como `"Folder/Name"`: su sitio, por el que se enciende y se apaga.
  Apagar una carpeta apaga todo lo que hay dentro.
- `tags`, una lista de etiquetas o una sola: cada una lo enciende y lo apaga
  junto con todo lo demás que la lleva.

Algo funciona mientras su sitio, todas las carpetas por encima y todas sus
etiquetas están encendidos. Una regla del editor tiene como sitio su carpeta y
su nombre (`Combat/Fight back`; dos iguales se distinguen por un número,
`Combat/Fight back #2`). El sitio de un script es dónde está entre los scripts
del perfil, sin extensión: `Mapping/rooms`.

## Mostrar y decir

### `send(text)`

Manda una orden al MUD tal cual: no se le aplican los alias.

### `echo(text, options)`

Muestra una línea en la salida y la lee en voz alta. Admite `aloud` y `always`
(ver [Lo que se oye](#lo-que-se-oye)).

### `speak(text, options)`

Dice algo en voz alta y no muestra nada. Admite `aloud` y `always`.

```lua
speak("You are dying!", {aloud = "cut", always = true})
```

### `notify(text, options)`

Le dice algo al jugador como el cliente dice lo suyo: unos segundos sobre la
esquina inferior derecha, y después bajo la campana; nunca en la salida. Habla
Muddie, no el juego, así que se oye en cualquier sesión y con la lectura del
juego apagada, salvo con `{aloud = "never"}`.

### `log(text)`

Muestra algo en la salida estándar si Muddie se arrancó con `--debug`; si no,
nada. Para lo que solo quiere ver quien escribe el script.

### `tr(texts)`

El texto en el idioma de la interfaz, de entre los que se dan por código de
idioma: si no está, el de su idioma sin país (`es` para `es_AR`); si no, el
inglés; si no, el primero. Para lo que un script compartido le enseña al
jugador.

```lua
notify(tr{en = "Map updated.", es = "Mapa actualizado."})
```

`echo`, `notify` y `speak` admiten también ellos mismos los textos por idioma:
`speak{en = "You are dying", es = "Te mueres"}`.

### `hush()`

Calla lo que se está leyendo, en la sesión que está delante.

## Lo que se oye

`speak`, `echo`, `line:read`, `panels.add`, `channels.say` y `gauges.set`
dicen de la misma forma cómo se oyen, en sus opciones:

| Opción | Cómo se oye |
|---|---|
| `aloud = "usual"` | Lo que pasa si no se dice nada: detrás de lo que se está leyendo; `line:read`, como se leería la línea; un indicador, según su tipo (ver [`gauges.set`](#gaugessetname-gauge)) |
| `aloud = "wait"` | Detrás de lo que se está leyendo |
| `aloud = "cut"` | Cortando lo que se está leyendo |
| `aloud = "never"` | Nunca |
| `aloud = "lowest"` | Solo de un indicador: espera, pero corta al bajar de su paso más bajo |
| `always = true` | También con la lectura del juego apagada |

Todo lo del juego pasa por una sola puerta, con las mismas reglas que sus
líneas:

- Solo se oye en la sesión que está delante.
- No se oye con la lectura del juego apagada, salvo con `always`.
- No se oye si los ajustes, el panel o el canal dicen que no se lea; `always` no
  se salta eso.
- Las mismas palabras recién leídas, como línea o de un canal, no se vuelven a
  leer, salvo que un script pida que corten.

`notify` no es del juego: se oye en cualquier sesión.

## Disparadores

### `trigger(pattern, fn, options)`

Llama a `fn(m, line)` por cada línea de la salida que coincida con `pattern`,
una expresión regular (la de `regex` de Rust, así que valen `\\d`,
`(?<name>...)` y `(?i)`). `m[0]` es la coincidencia entera, `m[1]`... sus
grupos, y un grupo con nombre es también `m.name`. `line` es
[la línea](#la-línea), para cambiar cómo se ve y cómo se oye.

Opciones: `place`, `tags`, `once` (se quita tras saltar una vez) y `prompt`:
`true`, solo para prompts; `false`, para todo menos prompts; sin darla, para
ambos. Un prompt es lo que el MUD marca como tal, o el texto que se queda sin
terminar, una vez que deja de cambiar.

```lua
trigger("^You are hungry", function() send("eat bread") end,
        {place = "Survival/Eat", tags = {"survival"}})

trigger("^\\[spam\\]", function(m, line) line:hide() end)

-- A prompt that is not read, and what comes after it on its line is.
trigger("^HP \\d+> (.*)$", function(m, line) line:read(m[1]) end)
```

Los disparadores se ejecutan en el orden en que se crearon; una línea pasa por
todos los que coinciden.

## La línea

Un disparador recibe la línea con la que coincidió, para hacer con ella lo que
quiera mientras la mira. Guardada para después, solo se le puede preguntar qué
decía.

### Campos

| Campo | Qué es |
|---|---|
| `line.text` | Lo que dice |
| `line.prompt` | Si es un prompt: marcado así por el MUD, o sin terminar |
| `line.spans` | La línea como trozos de texto con `text`, `fg`, `bg`, `bold`, `italic`, `underline` e `inverse` |

### `line:hide()`

No se muestra ni se lee.

### `line:mute()`

Se muestra, y se guarda para releerla con las teclas de repaso, pero no se lee
en voz alta al llegar.

### `line:read(text, options)`

Se lee `text` en su lugar, como se leería la línea, o como digan `aloud` y
`always`.

### `line:replace(text)`

Pone `text` en lugar de lo que coincidió: se ve así y, salvo que se diga otra
cosa, se lee así. Devuelve si ha podido: no sobre lo que ya cambió otro
disparador.

### `line:paint(fg, bg)`

Cambia el color de lo que coincidió. Un color es un nombre (`"red"`,
`"bright blue"`), `"#rrggbb"` o un número de la paleta; `nil` deja ese como
está.

### `line:link(command, options)`

Hace de lo que coincidió un enlace que manda `command`, al que se llega con el
teclado como a los que marca el MUD. `{hint = "..."}` dice para qué es.
Devuelve si ha podido, como `line:replace`.

### `line:to_panel(name)`

Añade la línea a un panel de texto, con sus colores.

### Una línea no es su texto

Su texto es siempre `line.text`: `echo(line.text)`, `line.text:match("%d+")`,
`line.text == "Done."`. Dar la línea donde se espera un texto es un error que lo
dice; pero `line == "Done."` no es un error en Luau, solo es siempre falso. Con
`--!strict` al principio del script, [las definiciones](#errores-y-cómo-comprobar-un-script)
lo detectan.

## Alias, teclas y temporizadores

### `alias(pattern, fn, options)`

Llama a `fn(m, input)` en vez de mandar lo que se escribe, cuando coincide con
`pattern`. Opciones: `place`, `tags` y `once`.

```lua
alias("^k (.+)$", function(m) send("kill " .. m[1]) end)
```

### `macro(key, action, options)`

Ejecuta órdenes o una función al pulsar una tecla: `macro("f5", "look")`,
`macro("ctrl+shift+k", fn)`. Las órdenes se mandan como las de una regla:
separadas por `;`, con `$name` sustituido y sin pasar por los alias. Opciones:
`place` y `tags`.

Una tecla asignada otra vez con otro `place` hace eso mientras está encendida:
la primera asignada que esté encendida es la que se ejecuta. Asignada otra vez
con el mismo sitio, pasa a hacer lo nuevo.

Las teclas se nombran en minúsculas, con sus modificadores en el orden `ctrl`,
`alt`, `shift`: `f1`...`f35`, `ctrl+k`, `alt+shift+up`. Del bloque principal
solo se pueden asignar las teclas de función y las que se pulsan con Ctrl o Alt,
así que nunca se roba lo que se escribe.

El teclado numérico tiene nombres propios: `numpad0` a `numpad9`, `numpadadd`,
`numpadsubtract`, `numpadmultiply`, `numpaddivide`, `numpaddecimal` y
`numpadenter`. Con Bloq Num apagado, las cifras son otras teclas, con el nombre
de lo que llevan escrito: `numpadup`, `numpaddown`, `numpadleft`,
`numpadright`, `numpadhome`, `numpadend`, `numpadpageup`, `numpadpagedown`,
`numpadclear` (el 5), `numpadinsert` y `numpaddelete`. Una tecla del teclado
numérico asignada deja de escribir su cifra.

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad5", "look")
```

### `after(seconds, fn, options)`

Llama a `fn()` una vez, pasados esos segundos. Opciones: `place` y `tags`.

### `every(seconds, fn, options)`

Llama a `fn()` una y otra vez, cada esos segundos. Apagado, deja pasar sus
turnos.

```lua
local saving = every(60, function() send("save") end)
after(5, function() saving:remove() end)
```

## Lo que crea un script, y encenderlo o apagarlo

### Lo que devuelven `trigger`, `alias`, `macro`, `after`, `every` y `on`

Lo que han creado:

| Método | Qué hace |
|---|---|
| `made:enable()`, `made:disable()`, `made:toggle()` | Lo encienden o lo apagan; devuelven si queda encendido |
| `made:enabled()` | Si está encendido |
| `made:remove()` | Lo quita; devuelve si estaba |
| `made:left()` | De un temporizador, los segundos que le quedan |

### `triggers`, `aliases`, `macros`, `timers`, `gauges`, `scripts`

Encienden y apagan las cosas de un tipo por su sitio, o una carpeta con todo lo
que tiene, estén hechas en un script o en el editor, para la sesión:

```lua
triggers.disable("Combat")
macros.enable("Combat/Kick")
local on = scripts.toggle("Mapping")
```

Cada uno tiene `enable(place)`, `disable(place)`, `toggle(place)` (devuelve si
queda encendido), `enabled(place)` y `list()`, los sitios que hay.

### `tags`

Lo mismo con las etiquetas: `tags.disable("combat")` apaga todo lo que lleva esa
etiqueta, de cualquier tipo.

## Esperar

Lo que lleva varios pasos se escribe uno detrás de otro, en vez de con
disparadores y temporizadores que se lanzan unos a otros.

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

### `wait(seconds)`

Se para ahí un rato; el resto del cliente y de los scripts sigue.

### `wait("line", pattern, options)`

Se para hasta que coincide una línea posterior a esta, y devuelve lo que recibe
un disparador: los grupos y la línea. Opciones: `timeout`, en segundos, tras el
cual devuelve `nil`; y `prompt`, como el de un disparador.

### `wait(what, name, options)`

Se para hasta que llega lo que escucha `on(what, name, ...)`, y devuelve lo que
recibiría (`true`, si no recibe nada). Opciones: `timeout`, y las de `on`:
`wait("gauge", "HP", {under = 30, timeout = 10})`.

### `lines(options)`

Las líneas que llegan a continuación, una a una, en un `for`: hasta que una
coincide con `till`, que no se incluye; hasta que ha habido `count`; o hasta que
no llega ninguna en `timeout` segundos. Necesita una de las tres.

### Dónde no se puede esperar

Lo que espera se va cuando su script se para o se vuelve a ejecutar. No se puede
esperar en `on("send", ...)`, en un módulo mientras se carga con `require`, ni
en una función que se le pasa a otra, como a `table.sort`.
`coroutine.wrap(function() ... end)()` ejecuta lo que tiene dentro, esperas
incluidas, en paralelo al resto.

## Escuchar: on

### `on(what, name, fn, options)`

Llama a `fn` cuando llega algo. Devuelve lo que ha creado, como un disparador.
Opciones: `place` y `tags`; para un indicador, también `under` y `over`. Algo
que no sabe escuchar es un error que dice qué hay.

| `what` | `name` | `fn` recibe |
|---|---|---|
| `"gmcp"` | un paquete; también los que están debajo (`"Char"` cubre `Char.Vitals`) | los datos, y el paquete |
| `"msdp"` | una variable | el valor (un texto), y la variable |
| `"var"` | una variable de [`vars`](#variables) | el valor, y el nombre |
| `"gauge"` | un indicador, o ninguno para todos | el indicador, y cómo ha cambiado (ver [`on("gauge")`](#ongauge-name-fn-options)) |
| `"channel"` | un canal, o ninguno para todos | `{channel, text}` |
| `"emit"` | lo que dice otro script | lo que dijo, y el nombre |
| `"send"` | ninguno | cada orden que va camino del MUD (ver [`on("send")`](#onsend-fn)) |
| `"connect"`, `"disconnect"` | ninguno | nada |
| `"focus"` | ninguno | si la sesión pasa a estar delante, en una ventana que lo está |
| `"reader"` | ninguno | si un lector de pantalla pasa a leer Muddie |
| `"mxp"` | ninguno | `{name, value}`, cuando el MUD nombra un valor con MXP |
| `"panel"` | ninguno | `{name, map, columns, rows}`, cuando un panel que muestra un dibujo deja de cambiar de tamaño; `map` es true para el panel del mapa |

Lo que cambia de una variable, un indicador, un canal u otro script se avisa
cuando termina el script que lo cambió, así que se avisa entero.

### `emit(name, data)`

Les dice algo a los demás scripts, que lo escuchan con `on("emit", name, fn)`.
Así cuenta un plugin lo que a otros les puede interesar.

```lua
-- In one script:
emit("fight", {with = "orc"})
-- In another:
on("emit", "fight", function(data) speak(`Fighting {data.with}`) end)
```

### `on("send", fn)`

Llama a `fn(text)` con cada orden que va a mandarse al MUD, venga de donde
venga: escrita, o mandada por un script, una regla, un camino o un enlace. Lo
que devuelve se manda en su lugar; con `false`, no se manda nada; con `nil`, va
tal cual. No se le avisa del nombre ni de la contraseña con que entra el
cliente, ni de lo que manda la propia función.

```lua
on("send", function(text)
    if text == "quit" then
        notify("Type it in full: quit!")
        return false
    end
    if text == "quit!" then return "quit" end
end)
```

## Variables

### `vars`

Las variables, como una tabla, guardadas con el perfil: sobreviven a la sesión.
Son las de **Variables** del editor.

```lua
vars.target = "orc"          -- keeps it
send("kill " .. vars.target) -- reads it, as it is now
vars.target = nil            -- forgets it
for name, value in vars do echo(`{name} = {value}`) end
```

Una tabla guardada en una variable se guarda tal como estaba al darla: cámbiala
y vuelve a darla.

```lua
local bag = vars.bag or {}
bag.gold = 5
vars.bag = bag
```

### `expand(text)`

`text` con cada `$name` sustituido por el valor de esa variable, `$name.key` por
un campo de una tabla y `$name.1` por un elemento de una lista, como en las
órdenes de las reglas.

### `on("var", name, fn)`

Llama a `fn(value, name)` cuando cambia la variable, sea un script, un
disparador o el editor quien la cambie. Un indicador del editor también puede
seguir una variable.

## Indicadores

Un indicador es ante todo un dato: lo haya hecho quien lo haya hecho (el
editor, un plugin, un script o el MUD), un script puede preguntarlo y
escucharlo, lo muestre un panel o no. Con lector de pantalla, los paneles de
indicadores no existen para él; lo que muestran se lee con Alt+0, y se dice
cuando cambia.

### `gauges.set(name, gauge)`

Muestra un indicador o lo actualiza. Lo que recibe:

| Campo | Qué es |
|---|---|
| `kind` | `"bar"` (si no se dice nada), `"cells"`, `"number"`, `"light"`, `"text"` o `"effects"` |
| `value`, `max` | Su nivel: una barra y las celdas necesitan los dos; un número, `value` |
| `on` | De una luz: si está encendida |
| `text` | De un texto: lo que dice |
| `items` | De los efectos: lo que tiene el personaje, una lista (o una tabla por nombre) de nombres o de `{name = "Poisoned", about = "1 damage a second", colour = "red"}` |
| `colour`, `off_colour` | Un nombre o `#rrggbb`; `off_colour` es el de una luz apagada |
| `steps` | Los colores que toma por debajo de cada porcentaje: `{{under = 60, colour = "yellow"}, {under = 30, colour = "red"}}` |
| `aloud`, `always` | Cómo se oyen sus cambios. `"usual"`: un nivel corta lo que se lee al bajar de un paso, y lo demás espera su turno; también `"lowest"`, `"wait"`, `"cut"`, `"never"` |
| `panel` | El panel de indicadores donde se muestra, por su nombre: se crea si no existe; si no se dice ninguno, el primero que haya |
| `place`, `tags` | Lo que lo enciende y lo apaga, como a un disparador |

Un nivel se dice al bajar de uno de sus pasos, una vez hasta que vuelve a
subir claramente por encima, y de nuevo cada décima parte más que baje estando
por debajo de uno; una luz, unas palabras o unos efectos se dicen cuando cambia
lo que dicen.

```lua
gauges.set("HP", {value = 40, max = 100,
                  steps = {{under = 30, colour = "red"}}, aloud = "lowest"})
gauges.set("Poisoned", {kind = "light", on = true, colour = "green"})
gauges.set("States", {kind = "effects", items = {"Blessed", "Stunned"}})
```

### `gauges.remove(name, options)`

Lo quita. Opciones: `place` y `tags`, los que se le dieron al ponerlo.

### `gauges.get(name)`

Cómo está ahora el indicador, por su nombre o por su sitio, o `nil`: una tabla
con `name`, `place` y `kind`, y según su tipo, `value`, `max` y `percent`;
`on`; `text`; o `items`, una lista de `{name, about, colour}`. Uno apagado es
como si no existiera.

### `gauges.all()`

Todos, en una lista, cada uno como lo da `gauges.get`.

### `on("gauge", name, fn, options)`

Llama a `fn(gauge, change)` cuando cambia el indicador, con cómo está ahora; sin
nombre, para todos. `change.was` es cómo estaba; de los efectos, `change.came` y
`change.gone` son listas de los que han llegado y de los que se han ido. Cómo es
un indicador al crearse no cuenta como cambio.

`{under = 30}`: solo al bajar del 30 %, una vez, hasta que vuelve a estar un 5 %
por encima. `{over = 90}`: al llegar al 90 %.

```lua
on("gauge", "HP", function() sounds.play("alarm.ogg") end, {under = 30})

on("gauge", "States", function(gauge, change)
    for _, state in change.came do speak(state.name) end
end)
```

## Paneles y canales

### `panels.set(name, text)`

Pone `text` en un panel de texto, en lugar de lo que mostraba: para lo que el
servidor manda listo para mostrar, como un mapa que dibuja él. El panel se crea
si no hay ninguno con ese nombre; `"map"` es el panel del mapa, se llame como se
llame, y `"channels"` el panel de canales.

### `panels.clear(name)`

Lo vacía.

### `panels.add(name, text, options)`

Añade `text` al final de un panel, que conserva lo que tenía, y lo lee en voz
alta tras el nombre del panel. Admite `aloud` y `always`.

### `panels.size(name)`

Cuántos caracteres caben en un panel, como `{columns, rows}`, en el momento;
`nil` si no ha llegado a verse en pantalla. Sin nombre, el panel del mapa.
`on("panel")` avisa cuando cambia.

### `channels.say(name, text, options)`

Añade `text` como dicho en el canal `name` del MUD, que tiene su propia pestaña
en un panel, y lo lee en voz alta tras el nombre del canal. Opciones:

- `panel`: el panel donde va el canal, por su nombre. Si hay uno con ese nombre
  se usa tal cual, y si no, se crea; sin decirlo, el panel de canales.
- `aloud` y `always`: `aloud = "never"` para lo que se dijo antes de que llegara
  el jugador.

El mismo mensaje dicho dos veces a la vez, por dos scripts, cuenta como uno.

### `channels.open(name, options)`

Crea la pestaña del canal antes de que nadie hable en él. Admite `panel`.

### `channels.close(name)`

Quita la pestaña, como cuando el personaje deja el canal.

### `channels.all()`

Los nombres de los canales que hay.

## Sonidos

### `sounds.play(file, options)`

Reproduce un sonido, por su ruta o por su nombre en la carpeta `sounds` del
perfil. Opciones:

- `volume`, de 0 a 1, y `loop`.
- Donde el reproductor del sistema puede (Linux): `start` y `finish`, dónde
  empezar y dónde terminar, y `fade_in` y `fade_out`, cuánto tarda en subir y en
  apagarse, en segundos. En Windows y Mac suena entero.

Devuelve el sonido: `sound:stop()`, y `sound:playing()`, si está sonando.

### `sounds.stop()`

Para todos los sonidos.

## La sesión y la línea de órdenes

### Campos de `session`

| Campo | Qué es |
|---|---|
| `session.connected` | Si la sesión está conectada |
| `session.host` | A dónde se conecta |
| `session.profile` | El nombre de su perfil, o `nil` |
| `session.reader` | Si un lector de pantalla está leyendo Muddie |
| `session.focused` | Si la sesión está delante, en una ventana que lo está |
| `session.input` | Lo escrito en la línea de órdenes; dale un valor para poner algo ahí. `nil` mientras es una contraseña, que tampoco se puede poner |
| `session.separator` | Lo que separa las órdenes escritas en una línea: `";"` si no se ha puesto nada; `false` quita la separación; `nil` vuelve a `";"`. Doble, se escribe tal cual |

### `session.completions(name, words)`

Palabras para que Tab las ofrezca, además de las que se ven en la salida. Cada
nombre tiene su propia lista; `session.completions(name)` la quita.

### `session.feed(text)`

Toma `text` como si lo hubiera mandado el MUD: para probar qué hace un script
con ello.

### `output.last(count)`

Lo que dicen las últimas `count` líneas de la salida, la más nueva al final; si
no se da `count`, la última.

## Datos del MUD

```lua
gmcp.support("Char 1", "Room 1")
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("HP", {value = vitals.hp, max = vitals.maxhp})
end)

msdp.report("HEALTH", "HEALTH_MAX")
on("msdp", "HEALTH", function(value) echo("health " .. value) end)
```

### `gmcp.get(package)`

Los últimos datos recibidos de exactamente ese paquete.

### `gmcp.send(package, data)`

Manda un mensaje GMCP.

### `gmcp.support(module, ...)`

Le pide al servidor módulos GMCP además de los del cliente (`Core` y
`Client.Media`), como `"Name version"`. Algunos servidores solo mandan lo que se
les pide. Se piden mientras el script está cargado: al apagarlo o quitarlo, se
le dice al servidor que ya no los quiere.

### `gmcp.merge(package, ...)`

Para los paquetes de los que el servidor solo manda lo que cambia, como un
`Char.Status` con solo la vida que ha cambiado: lo que llega se mezcla con lo
que guardan `gmcp.get` y los indicadores, también las tablas de dentro. Quien
escucha sigue recibiendo lo que llegó.

### `msdp.get(variable)`, `msdp.send(command, ...)`

Lo mismo para MSDP. Los valores llegan como texto.

### `msdp.report(variable, ...)`

Le pide al servidor que avise de las variables MSDP cuando cambien, mientras el
script está cargado.

### `mssp.get()`

El estado MSSP del servidor, como una tabla.

## El mapa

El plugin **GMCP y MSDP estándar** hace el mapa con `Room.Info` de GMCP y `ROOM`
de MSDP. Para un MUD que dice las salas de otra forma, o solo en el texto, un
script alimenta el mapa él mismo, y puede apagar el cargador estándar.

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

### Salas y sitios

Una sala se da como una tabla con `id` y, si se quiere, `name`, `area`, `exits`
(dirección a id de sala), `x`, `y`, `z` y `colour` (un nombre, `"#rrggbb"` o un
número de la paleta; una sala dada sin él conserva el color que tenía). Un id es
un número, o el texto con el que el servidor nombra la sala. Lo que un MUD manda
en `Room.Info` o `ROOM` se puede dar tal cual llega: `num` o `vnum` por `id`,
`zone` por `area`, `coords` por dónde está, en mayúsculas o minúsculas.

Un sitio, donde una función lo pide (`map.room`, `map.here`, `map.mark`,
`map.edit_room`, `map.remove_room`, `map.way`, `map.go`), es el id de una sala,
o una marca o un nombre como los escribe el jugador, el más cercano si hay
varios.

### Ajustes del mapa

Son campos: se les da un valor, y se leen. `nil` deja uno como estaba.

| Campo | Qué hace |
|---|---|
| `map.native` | `false` apaga el cargador estándar mientras el script está cargado |
| `map.persist` | `false` guarda el mapa solo en memoria, para servidores que lo mandan ellos |
| `map.learn` | `true` hace que el mapa aprenda mientras el jugador se mueve (ver [`map.learn`](#maplearn)) |
| `map.language` | El idioma en que se juega el MUD, por código o nombre (`"es"`): el mapa toma sus palabras para las direcciones |
| `map.directions` | Palabras propias del MUD para las direcciones, por encima de las de su idioma: `{n = {"nte"}, s = "sur"}`; la primera de cada una es la que mandan los caminos |
| `map.names` | Cómo nombra el MUD sus salas: `{region = ":", exits = "[]"}`, primero la región, hasta `region`, y después las salidas, entre los caracteres de `exits` |

### Seguir al jugador

#### `map.visit(room)`

El jugador está ahora en `room`, que se añade o se actualiza. Devuelve su id.

#### `map.set_room(room)`

Añade o actualiza una sala sin moverse a ella. Devuelve su id.

#### `map.here(place)`

El jugador está en esa sala, una que tiene el mapa. Devuelve si la tiene.

#### `map.arrive{name, exits, came}`

Para servidores que no dicen en qué sala se está: el jugador está en una sala
con este nombre y estas salidas, a la que ha llegado por la salida `came`.
Devuelve el id de la sala, o `nil` mientras no está seguro.

#### `map.add{name, exits, came}`

Para servidores que no dicen en qué sala se está, cuando el jugador ha ido por
`came` a donde el mapa no tiene sala: busca la sala entre las que tiene, o la
crea. Devuelve su id y `"found"` o `"made"`, o `nil` y por qué no: `"lost"`, el
mapa no sabe seguro dónde estaba el jugador; `"known"`, la salida ya lleva a una
sala; `"alike"`, podría ser cualquiera de varias salas; `"unnamed"`. El jugador
puede deshacer las salas añadidas.

#### `map.learn`

Con `true`, el mapa aprende de las salas que recibe `map.arrive`, nunca de verlas
una sola vez, ni desde una sala en la que no sabe seguro que estuviera el
jugador: añade las salas que no tiene a donde va el jugador por la brújula;
corrige los nombres de las salas que tenía con otro, cuando el siguiente
movimiento lo confirma; añade las salidas que una sala muestra y a él le faltan;
y hace que una salida lleve a la sala a la que lleva de verdad. Cada cambio se
avisa, y se deshace como uno propio del mapa.

#### `map.candidates()`

Los ids de las salas en las que puede estar el jugador mientras el mapa no sabe
en cuál; vacía cuando lo sabe, o cuando no tiene ni idea.

#### `map.failed()`

El servidor ha dicho que un movimiento ha fallado: los movimientos mandados no
han ocurrido, y un camino se para.

### Caminar

#### `map.way(to, from)`

Las órdenes para ir de `from` (de donde está el jugador, si no se da) a `to`, o
`nil`.

#### `map.go(to)`

Va hasta allí, un movimiento confirmado cada vez.

#### `map.stop()`

Para un camino, como hace **Dejar de ir**.

### Consultar

#### `map.room(place)`

Una sala como tabla, la del jugador si no se da ninguna: `id`, `name`, `area`,
`x`, `y`, `z`, `exits` (dirección a id de sala), `cost` (lo que cuesta entrar en
ella a un camino), `avoid`, `colour` (`"#rrggbb"` o `nil`), `note` (`""` si no
tiene), `renamed` (su nombre es uno puesto en lugar del del mapa), `added` (la
añadió el mapa por su cuenta), `shape` y `symbol`.

#### `map.find(text, options)`

Los ids de las salas que encuentra `text`, como las encuentra
**Buscar salas…**: por marca, nota o nombre, la más cercana primero. Con
`{exact = true}`, de las que se llaman exactamente `text`, como las nombra el
MUD.

#### `map.exit(from, direction)`

Una salida como tabla: `to`, `cost`, `blocked` (fuera de los caminos), `command`
y `custom` (una propia del jugador).

#### `map.direction(word)`

La dirección que dice una palabra, como la guarda el mapa (`"n"`, `"ne"`...
`"u"`, `"d"`, `"in"`, `"out"`), en las palabras del MUD o en inglés; `nil` si no
dice ninguna.

#### `map.count()`, `map.areas()`, `map.id(text)`

Cuántas salas hay; los nombres de las áreas; el id que se usa para un id de
texto.

#### `map.marks()`

Las marcas, por nombre, cada una con el id de su sala.

### Cambiar el mapa

#### `map.mark(name, place)`

Marca una sala con `name`, como hace **Marcar…**; `map.mark(name)` olvida la
marca. Devuelve si existía la sala, o la marca.

#### `map.edit_room(place, changes)`

Cambia lo que dé `changes` de una sala, dejando el resto: `name` (en lugar del
del mapa: que el servidor vuelva a decir la sala, o que se importe otra vez el
mapa, lo respeta), `note`, `cost`, `avoid`, `x`, `y`, `z`, `colour`, `shape`
(`"square"`, `"circle"`, `"diamond"` o `"hexagon"`) y `symbol` (uno o dos
caracteres escritos en ella). `false` devuelve el color o la forma del mapa, sin
coste o sin símbolo. Devuelve si existía la sala.

#### `map.set_exit(from, direction, to, options)`

Añade una salida propia del jugador, o cambia a dónde lleva una; con `to` a
`nil`, cambia solo lo que den `options`: `cost`, `blocked` o `command`
(`"open door;n"`). Devuelve si existían la sala y la salida.

#### `map.remove_exit(from, direction)`, `map.remove_room(place)`

Quitan una.

#### `map.clear()`

Olvida todas las salas.

#### `map.tidy(area)`

Vuelve a dibujar un área desde cero, la del jugador si no se da ninguna, si así
queda mejor dibujada. Devuelve cuántas salas se han movido; ninguna si se ha
dejado como estaba, y entonces, como segundo valor, `{now = {wrong_exits, stacked}, ["then"] = {wrong_exits, stacked}}`.

#### `map.undo()`

Deshace el último cambio que el mapa hizo por su cuenta. Devuelve si había
alguno.

#### `map.show_area(area)`

Muestra un área en el mapa, para mirarla, sin ir.

### El mapa que publica un MUD

#### `map.import(address)`

Importa el mapa hecho con Mudlet que hay en `address` (un `.dat`, o un zip con
uno; en la web o en disco) en lugar de este. Lo que puso el jugador se conserva:
salas renombradas, notas, marcas, salas que evitar y salidas propias, y las
salas que añadió el mapa.

#### `map.newer(address, fn)`

Averigua, sin traerlo, si el mapa de `address` ha cambiado desde que se trajo, y
llama a `fn()` si es así: para avisar al jugador, en vez de cambiarle el mapa
sin preguntar.

#### `map.action(name, fn, options)`

Ofrece hacer `fn()` con el mapa, eligiéndolo por `name` en el menú del mapa y en
la paleta, mientras el script está cargado; el de un plugin, bajo el nombre del
plugin. `name` se puede dar por idioma, como a `tr`. Con `{checked = false}` es
un interruptor: al elegirlo se llama a `fn(on)` con cómo queda. `map.action(name)`
lo quita.

## Módulos

### `require(name)`

Carga `name.luau`, o `name/init.luau`, de la carpeta del script que lo pide (o,
si no está, de la de cualquier otro cargado), una sola vez, y devuelve lo que
devuelve. Así comparten sus módulos los scripts de un plugin.

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

Un módulo en la carpeta de un plugin también se ejecuta como script, igual que
todos los `.luau` que hay allí: haz que un módulo solo devuelva lo que tiene, y
que no cree cosas.

## Errores, y cómo comprobar un script

- Un error se muestra como `script.luau:12: what is wrong`, y se guarda para el
  editor. Una llamada a algo que no existe dice a qué: `there is no function
  gauges.sett`.
- Una opción que una función no admite, algo que `on` no conoce, una tecla o un
  color que no lo son, son errores que dicen qué hay.
- [`muddie.d.luau`](muddie.d.luau) tiene todas las funciones con sus tipos, para
  un editor con luau-lsp (como archivo de definiciones) o para `luau-analyze`:
  lo que esté mal escrito se ve al escribirlo, y, con `--!strict` al principio
  del script, también una línea comparada con un texto.
- `muddie --debug` muestra en la salida estándar todo lo que pasa: el texto
  (`TEXT`, una línea; `PROMPT`, una que el MUD marca como prompt; `PARTIAL`, lo
  que aún no ha terminado), lo que se manda (`SENT`), el GMCP y los demás
  protocolos, lo que los scripts mandan a `log`, y sus errores (`SCRIPT`).
  `muddie --debug | tee muddie.log` lo guarda. Las contraseñas no se muestran
  nunca.

## Lo que no hay

A propósito, los scripts no pueden:

- Crear ventanas, etiquetas o barras propias: eso lo hacen un panel de texto o
  un indicador, que se oyen además de verse.
- Leer ni escribir archivos, arrancar programas ni descargar lo que quieran: un
  script de otra persona no debe poder hacer daño. `vars` guarda datos.
- Añadir sus propias órdenes o ajustes a la paleta: eso lo hacen los alias, las
  teclas y `vars`.
- Ordenar los disparadores por prioridad: se ejecutan en el orden en que se
  crearon, y `wait` sigue lo que llega en varias líneas.
- Guardar datos propios en cada sala del mapa.
