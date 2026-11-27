# Cómo hacer un plugin de Muddie

English: [plugins.md](plugins.md). Todas las funciones que puede usar un plugin
están en [scripting.es.md](scripting.es.md); sus tipos, en
[muddie.d.luau](muddie.d.luau).

Un plugin le enseña a Muddie cómo es un MUD: sus indicadores, sus canales, su
mapa, lo que merece decirse en voz alta. Muddie no sabe nada de ningún MUD en
concreto; todo eso son plugins. El jugador activa un plugin, o lo acepta
cuando Muddie se lo ofrece al conectar, y el plugin lo prepara todo él solo.

## Contenido

1. [Qué es un plugin](#qué-es-un-plugin)
2. [Un plugin en cinco minutos](#un-plugin-en-cinco-minutos)
3. [Probarlo mientras lo escribes](#probarlo-mientras-lo-escribes)
4. [Los archivos](#los-archivos)
5. [Cómo lo instala y lo actualiza Muddie](#cómo-lo-instala-y-lo-actualiza-muddie)
6. [Escribirlo bien](#escribirlo-bien)
7. [Recetas](#recetas)
8. [Publicarlo](#publicarlo)
9. [Escribir un plugin con una IA](#escribir-un-plugin-con-una-ia)

## Qué es un plugin

Una carpeta, cuyo nombre es el id del plugin, con estos archivos y nada más:

| Archivo | Qué es |
|---|---|
| `plugin.toml` | Qué es: su nombre, qué hace, para qué MUD es |
| `*.luau` | Sus scripts, al menos uno |
| `rules.toml` | Las reglas que trae, como las del editor: indicadores, disparadores, alias, teclas, temporizadores. Opcional |
| `template.toml` | Un perfil listo para jugar al MUD, para **Perfil nuevo**. Opcional |

Hay tres clases, y todas se escriben igual:

- **Incluidos** en Muddie: el plugin **GMCP y MSDP estándar**, para lo que
  muchos MUD mandan de la misma manera.
- **Oficiales**: los del repositorio
  [muddie-plugins](https://github.com/midnattsol/muddie-plugins), que Muddie
  descarga y mantiene al día él solo.
- **De git**: cualquier repositorio con un plugin en su raíz, que el jugador
  instala con **Instalar un plugin…** en la paleta y su dirección.

## Un plugin en cinco minutos

Un plugin para un MUD llamado Example, en `mud.example.org`, que manda la vida
y la energía por GMCP en `Char.Vitals`, y los mensajes de sus canales en
`Comm.Message`. La carpeta es `example-mud/`.

`plugin.toml`:

```toml
name = "Example MUD"
about = { en = "Its gauges and channels.", es = "Sus indicadores y canales." }
hosts = ["mud.example.org"]
names = ["example"]
muddie = "0.5.0"
```

`rules.toml`, con sus indicadores: siguen solos lo que manda el MUD, y el
jugador puede cambiarlos en el editor como si fueran suyos.

```toml
[[gauge]]
name = "Life"
value = "Char.Vitals.hp"
max = "Char.Vitals.maxhp"
kind = "bar"
colour = "#5fb85f"
steps = [{ under = 60, colour = "#e0c040" }, { under = 30, colour = "#e05050" }]
aloud = "lowest"
panel = "Points"

[[gauge]]
name = "Energy"
value = "Char.Vitals.ep"
max = "Char.Vitals.maxep"
kind = "bar"
colour = "#5f8fd8"
aloud = "wait"
panel = "Points"
```

`example.luau`, lo que hace:

```lua
-- Example MUD: its channels, each in its tab. Its gauges are in rules.toml.
gmcp.support("Char 1", "Comm 1")

on("gmcp", "Comm.Message", function(message)
    if type(message) == "table" and message.channel and message.text then
        channels.say(message.channel, message.text)
    end
end)
```

`template.toml`, para que **Perfil nuevo** ofrezca el MUD listo para jugar:

```toml
name = "Example MUD"
host = "mud.example.org"
port = 4000
plugins = ["example-mud"]
```

Eso es todo. Al conectar a `mud.example.org`, Muddie ofrece el plugin; una vez
activado, los indicadores se ven y se oyen cuando bajan, y cada canal tiene su
pestaña, que se lee en voz alta tras su nombre.

## Probarlo mientras lo escribes

1. Pon la carpeta dentro de la carpeta `plugins` de la carpeta de
   configuración: `~/.config/muddie/plugins/example-mud/` en Linux,
   `%APPDATA%\muddie\plugins\example-mud\` en Windows. Muddie lo toma como uno
   de git.
2. Actívalo en la pestaña **Plugins** del perfil, en la ventana **Perfiles**.
   Sus scripts se instalan en el perfil, en **Scripts**, en una carpeta con su
   nombre; sus indicadores, en **Indicadores**.
3. Cuando cambies los archivos del plugin, **Instalar de nuevo**, en su menú de
   la pestaña **Plugins**, lo deja como está ahora. Para probar un cambio más
   rápido, cambia la copia instalada en el editor y ejecútala; después pásalo
   al plugin.

Para ver qué hace:

- `session.feed("text")`, en **Ejecutar Luau…** de la paleta, toma el texto
  como si lo hubiera mandado el MUD: sirve para probar un disparador sin el
  MUD.
- `muddie --debug | tee muddie.log` muestra todo lo que pasa: el texto
  (`TEXT`, `PROMPT`, `PARTIAL`), lo que se manda, cada mensaje GMCP (`GMCP<`),
  lo que los scripts escriben con `log` y sus errores (`SCRIPT`). Es lo primero
  que hay que mirar para saber qué manda un MUD y para encontrar lo que falla.
- Un error en un script se muestra en la esquina y queda guardado en el
  editor, con su archivo y su línea.
- [muddie.d.luau](muddie.d.luau) tiene todas las funciones con sus tipos: si se
  lo das a un editor con luau-lsp, o a `luau-analyze`, un nombre mal escrito se
  ve en cuanto lo escribes.

## Los archivos

### `plugin.toml`

```toml
name = { en = "Example MUD", es = "Ejemplo" }   # a text, or one by language
about = "What it does, in a sentence."          # the same
hosts = ["mud.example.org"]   # part of the address of the MUDs it is for
names = ["example"]           # or the names they give themselves (MSSP)
default_on = false            # on in every profile until switched off
muddie = "0.5.0"              # the oldest Muddie it works with
```

- `name` y `about` se muestran en la pestaña **Plugins** y en el ofrecimiento.
  Un texto, o una tabla por código de idioma (`en`, `es`...), como los que
  acepta [`tr`](scripting.es.md#trtexts); el nombre en inglés es la carpeta en
  la que se instala, en cada perfil.
- `hosts` y `names` dicen cuándo ofrecerlo: cuando la dirección contiene uno de
  `hosts`, o cuando el MUD dice por MSSP que se llama como uno de `names`.
- `default_on`: activado en todos los perfiles salvo que se desactive. Para el
  plugin de un solo MUD, déjalo desactivado: se ofrece al conectar a ese MUD.
- `muddie`: un Muddie más antiguo que esta versión se queda con la versión del
  plugin que ya tiene.
- Nada más: cualquier otro campo es un error, y el plugin no se acepta.

### `rules.toml`

Las reglas que trae, escritas como el `rules.toml` de un perfil. Cada una va en
una carpeta con el nombre del plugin; `folder = "Combat"` la pone en una
carpeta dentro de esa. Todas las clases admiten `name`, `folder`, `tags` y
`enabled`.

Un indicador, `[[gauge]]`:

| Campo | Qué es |
|---|---|
| `name` | Su nombre, tal como se muestra y se dice |
| `value` | Dónde está su valor: una ruta de GMCP (`Char.Vitals.hp`), una variable de MSDP o una variable de `vars` |
| `max` | Dónde está su máximo, o un número |
| `kind` | `"bar"`, `"cells"`, `"number"`, `"light"`, `"text"` o `"effects"` |
| `colour`, `off_colour` | Un nombre o `#rrggbb` |
| `steps` | `[{ under = 30, colour = "#e05050" }]`: el color por debajo de cada porcentaje |
| `aloud` | `"usual"`, `"lowest"`, `"wait"`, `"cut"` o `"never"`, como en [`gauges.set`](scripting.es.md#gaugessetname-gauge) |
| `always` | `true` para que se oiga también con la lectura del juego apagada |
| `panel` | El panel de indicadores en el que se muestra, por su nombre: se crea si no existe |

Un disparador, `[[trigger]]`: `pattern` (un texto que contiene la línea, o una
expresión regular con `regex = true`), y lo que hace: `send` (órdenes, una por
línea o separadas por `;`, con `%1`... para lo que ha coincidido y `$name` para
las variables), `speak`, `sound`, `highlight` (un color), `panel` (muestra la
línea allí), `gag = true` (la oculta), `silence = true` (la muestra y no la
lee), o `code` con `luau = true` (Luau, que recibe `m` y `line` igual que la
función de un disparador).

Un alias, `[[alias]]`: `pattern` (la primera palabra de lo que se escribe, o
una expresión regular con `regex = true`), y `send` o `code`. Una tecla,
`[[macro]]`: `key` (`"f5"`, `"ctrl+k"`, `"numpad8"`), y `send` o `code`. Un
temporizador, `[[timer]]`: `seconds`, `repeat = true` para que se repita una y
otra vez, y `send` o `code`.

Lo que se hace mejor en un script, hazlo en un script: las reglas son para lo
que el jugador querrá cambiar en el editor, como los colores y los pasos de un
indicador.

### `template.toml`

```toml
name = "Example MUD"        # the game, which the profile is named after
host = "mud.example.org"
port = 4000
tls = false
encoding = "utf-8"          # optional: as the MUD says, if not given
language = "es"             # optional: the language the game is played in
plugins = ["example-mud"]   # the plugins switched on, by their ids
```

**Perfil nuevo** lo ofrece entre los MUD a los que jugar.

### Los scripts

- **Ponles nombre en inglés y sin espacios**: `example.luau`, `aloud.luau`,
  `prompt.luau`. Se ejecutan en el orden de sus nombres.
- Se instalan en el perfil del jugador, en una carpeta con el nombre del
  plugin, para leerlos y cambiarlos en el editor. Al desactivar el plugin se
  desactivan.
- **Módulos**: `require("prompt")` carga `prompt.luau` de la propia carpeta del
  plugin, una sola vez, para todos sus scripts. Cada `.luau` de esa carpeta se
  ejecuta también como script, así que un módulo solo devuelve lo que tiene.
- **Pon lo que se oye en un script aparte**, `aloud.luau`, que el jugador puede
  desactivar por separado sin perder lo que se ve (ver
  [Escribirlo bien](#escribirlo-bien)).

## Cómo lo instala y lo actualiza Muddie

- **Al activarlo** en un perfil, el plugin se instala en él: sus scripts, en
  una carpeta con su nombre entre los scripts del perfil; sus reglas, entre las
  del perfil, en una carpeta con su nombre; sus indicadores, en los paneles que
  nombran. Son del jugador, para cambiarlos.
- **Al desactivarlo**, esas carpetas se desactivan; al volver a activarlo, lo
  que el jugador había desactivado dentro sigue desactivado.
- **Una versión nueva**, oficial o de git, se instala sola al abrir el perfil,
  nunca en mitad de una partida. Lo que tenía antes y ya no tiene desaparece,
  scripts y reglas; un script que el jugador cambió se guarda al lado, como
  `name.luau.before-update`; lo que el jugador añadió se queda.
- **Instalar de nuevo**, en su menú, vuelve a poner lo que instaló tal como
  viene.
- En una sesión sin perfil, se ejecuta tal como viene, hasta que acaba la
  sesión.

## Escribirlo bien

- **Lo hace todo él solo.** Al activarlo, el plugin pide al MUD lo que
  necesita (`gmcp.support`), pone sus indicadores y paneles, y le dice al
  jugador, una vez, lo que tenga que hacer en el juego. Nada que preparar a
  mano.
- **Datos, no pantalla.** Lo que dice el MUD va a datos (indicadores, canales,
  el mapa, `vars`); un panel es solo una forma de verlo. Quien escucha recibe lo
  mismo: los indicadores se consultan con Alt+0 y se dicen cuando cambian.
- **Lo que se oye sale de los datos, no de lo que se ve.** Escríbelo en
  `aloud.luau`, escuchando lo que manda el MUD (`on("gmcp")`, disparadores),
  nunca un indicador o un panel que el jugador puede borrar.
- **Poco ruido.** Di lo que cambia, no todo otra vez: el estado que llega y el
  que se va, no todos. Lo importante, con `aloud = "cut"`; lo demás, después de
  lo que se esté leyendo. Usa `line:mute()` o `line:read(shorter)` para las
  líneas que dicen demasiado. Deja `always = true` para lo urgente.
- **Todos los textos en inglés y en español**, por idioma:
  `notify{en = ..., es = ...}`, `tr{...}`. El jugador los ve en el idioma de la
  interfaz.
- **Avisa una sola vez.** Lo que el jugador tiene que saber (un módulo que
  activar en el juego, un mapa más nuevo) se dice una vez, y se guarda en
  `vars` si debe quedar dicho.
- **Guarda en `vars` lo que elige el jugador**, con nombres propios del plugin:
  `vars.example_learning`.
- **Nada del MUD dentro de Muddie.** Si el plugin necesita algo que Muddie no
  tiene, eso es una función de la API que hay que pedir, útil para cualquier
  MUD.

## Recetas

### Indicadores desde GMCP

En `rules.toml`, como en [el ejemplo](#un-plugin-en-cinco-minutos). Un paquete
que solo manda lo que ha cambiado (solo la vida, cuando solo ha cambiado la
vida) se combina con lo que llegó antes, para que los indicadores lo tengan
entero:

```lua
gmcp.merge("Char.Status")
```

### Los estados del personaje, como un indicador de efectos

```lua
local states = {}

local function show()
    local items = {}
    for _, state in states do
        table.insert(items, {name = state.name, about = state.about})
    end
    gauges.set("States", {kind = "effects", items = items, aloud = "never",
                          panel = "States", place = "Example MUD/States"})
end

on("gmcp", "Char.States", function(data)
    states = {}
    for _, state in data do
        table.insert(states, {name = state.name, about = state.help})
    end
    show()
end)
```

Y lo que cambia de ellos, en `aloud.luau`, a partir de los mismos datos:

```lua
local known = nil

on("gmcp", "Char.States", function(data)
    local now = {}
    for _, state in data do now[state.name] = true end
    if known then
        for name in now do
            if not known[name] then speak(`{name}.`) end
        end
        for name in known do
            if not now[name] then speak({en = `No longer: {name}.`, es = `Ya no: {name}.`}) end
        end
    end
    known = now
end)

on("disconnect", function() known = nil end)
```

### Canales

```lua
on("gmcp", "Comm.Message", function(message)
    channels.say(message.channel, message.text)
end)
-- What was said before the player came: kept, not read.
on("gmcp", "Comm.History", function(message)
    channels.say(message.channel, message.text, {aloud = "never"})
end)
```

### El mapa

Si el MUD manda `Room.Info`, como la mayoría, el plugin estándar ya hace el
mapa. Si dice en qué sala estás con un GMCP propio:

```lua
map.native = false
on("gmcp", "Room.Info", function(room)
    map.visit({id = room.vnum, name = room.name, area = room.zone, exits = room.exits})
end)
```

Si solo escribe la sala en su texto, el mapa te sigue por el nombre y las
salidas, y puede aprender lo que le falta:

```lua
map.language = "es"
map.names = {region = ":", exits = "[]"}
map.learn = vars.example_learning ~= false

trigger("^(.+) \\[([a-z,]+)\\]$", function(m)
    local exits = {}
    for exit in m[2]:gmatch("[^,]+") do table.insert(exits, exit) end
    map.arrive({name = m[1], exits = exits})
end)
```

Una opción para el jugador, en el menú del mapa y en la paleta, que se
recuerda:

```lua
map.action({en = "Learn the map as you go", es = "Aprender el mapa sobre la marcha"},
    function(on)
        vars.example_learning = on
        map.learn = on
    end,
    {checked = map.learn})
```

### El prompt

Que no se lea, y que se lea lo que viene detrás en la misma línea:

```lua
trigger("^HP:\\d+ SP:\\d+> ?(.*)$", function(m, line)
    if m[1] == "" then line:mute() else line:read(m[1]) end
end)
```

Si cada jugador se hace su propio prompt, apréndelo tal como lo dice el juego,
guárdalo en `vars` y saca de él el patrón. El plugin de Reinos de Leyenda, en
muddie-plugins, lo hace así en su `prompt.luau`.

### Avisar al jugador una vez

```lua
on("connect", function()
    after(90, function()
        if not gmcp.get("Char.Vitals") and not vars.example_gmcp_told then
            vars.example_gmcp_told = true
            notify({en = "Type 'gmcp on' in the game for your gauges.",
                    es = "Escribe 'gmcp on' en el juego para tus indicadores."})
        end
    end)
end)
```

## Publicarlo

- **En un repositorio propio**: pon los archivos del plugin en su raíz. El
  jugador lo instala con **Instalar un plugin…**, en la paleta, y su
  dirección; **Actualizar un plugin…** lo pone al día.
- **Como plugin oficial**: abre una pull request en
  [muddie-plugins](https://github.com/midnattsol/muddie-plugins), con el plugin
  en una carpeta con su id. Allí tiene que llevar también `rules.toml` y
  `template.toml`, y ninguna carpeta dentro. Sus herramientas lo comprueban
  antes de aceptarlo:

  ```sh
  python3 tools/check.py   # the files of every plugin
  python3 tools/api.py     # the use of the API, with luau-analyze
  python3 tools/pack.py    # plugins.zip, as Muddie downloads it
  ```

  Una vez aceptado, release-please saca una versión, y todos los Muddie la
  toman solos.

## Escribir un plugin con una IA

Una IA puede escribir un plugin con estos archivos y una sesión del MUD. Dale:

1. Esta guía, [plugins.es.md](plugins.es.md).
2. La referencia, [scripting.es.md](scripting.es.md), y los tipos,
   [muddie.d.luau](muddie.d.luau).
3. **El registro de una sesión real**, `muddie --debug | tee session.log`:
   entrar, caminar, un combate, los canales. Muestra lo que manda el MUD, línea
   a línea y por GMCP, y ninguna descripción lo sustituye. Quita tu contraseña
   si la escribiste donde el juego la muestra.
4. El plugin de Reinos de Leyenda, en muddie-plugins, como ejemplo de uno
   completo.

Y pídele, por ejemplo:

> Haz un plugin de Muddie para el MUD de session.log, siguiendo plugins.es.md
> y scripting.es.md, y usando solo las funciones de muddie.d.luau. Indicadores
> para lo que manda por GMCP, sus canales cada uno en una pestaña, el mapa
> siguiendo al jugador y, en aloud.luau, que solo se diga en voz alta lo que
> cambia. Todos los textos en inglés y en español. Dame cada archivo completo.

Después pruébalo como en
[Probarlo mientras lo escribes](#probarlo-mientras-lo-escribes), y devuélvele
lo que falle: el error tal como lo muestra Muddie, y las líneas del registro de
alrededor. Antes de usarlo, comprueba:

- [ ] `plugin.toml` solo tiene los campos de arriba, y `muddie = "0.5.0"` o posterior.
- [ ] Los scripts tienen nombre en inglés, sin espacios; lo que se oye, en `aloud.luau`.
- [ ] Solo usa funciones de `muddie.d.luau`; `luau-analyze` no encuentra nada mal escrito.
- [ ] Nada se dice dos veces: lo que dice un indicador no lo repite un script.
- [ ] Todos los textos que ve el jugador, en inglés y en español.
- [ ] Al activarlo en un perfil nuevo, funciona sin tocar nada a mano.
