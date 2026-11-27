# Cómo hacer un plugin de Muddie

English: [plugins.md](plugins.md).

Esta guía te lleva desde cero hasta un plugin que otros jugadores pueden
instalar. No necesitas haber hecho ninguno antes. Cuando quieras consultar una
función, ve a [scripting.es.md](scripting.es.md). Cuando quieras los tipos
exactos, ve a [muddie.d.luau](muddie.d.luau).

## Contenido

1. [Para qué sirve un plugin](#para-qué-sirve-un-plugin)
2. [Tu primer plugin](#tu-primer-plugin)
3. [Pruébalo mientras lo escribes](#pruébalo-mientras-lo-escribes)
4. [Los archivos en detalle](#los-archivos-en-detalle)
5. [Cómo lo instala y lo actualiza Muddie](#cómo-lo-instala-y-lo-actualiza-muddie)
6. [Hacerlo bien](#hacerlo-bien)
7. [Recetas](#recetas)
8. [Publicarlo](#publicarlo)
9. [Escribir un plugin con una IA](#escribir-un-plugin-con-una-ia)

## Para qué sirve un plugin

Un plugin le enseña a Muddie cómo funciona un MUD. Trae sus indicadores, sus
canales, su mapa y lo que merece la pena decir en voz alta. Muddie no sabe
nada de ningún MUD en concreto, así que todo eso vive en plugins.

Haz un plugin cuando quieras compartir ese trabajo con todos los jugadores del
MUD. Unos cuantos scripts en tu perfil solo te sirven a ti. Un plugin lo
prepara todo él solo: el jugador lo activa, o dice que sí cuando Muddie se lo
ofrece al conectar, y no hace falta nada más.

### Qué lleva dentro

Un plugin es una carpeta. El nombre de la carpeta es el id del plugin. Dentro
lleva estos archivos y nada más:

| Archivo | Qué contiene |
|---|---|
| `plugin.toml` | El nombre del plugin, qué hace y para qué MUD es |
| `*.luau` | Sus scripts. Necesitas al menos uno |
| `rules.toml` | Opcional. Indicadores, disparadores, alias, teclas y temporizadores, las mismas reglas que hace el editor |
| `template.toml` | Opcional. Un perfil listo para jugar al MUD, que se ofrece en **Perfil nuevo** |

### Las tres clases

Las tres clases se escriben igual. Solo cambia de dónde vienen:

- **Incluidos**: Muddie trae el plugin **GMCP y MSDP estándar**. Se encarga de
  lo que muchos MUD mandan de la misma manera.
- **Oficiales**: están en el repositorio
  [muddie-plugins](https://github.com/midnattsol/muddie-plugins). Muddie los
  descarga y los mantiene al día él solo.
- **De git**: cualquier repositorio con un plugin en su raíz. El jugador lo
  instala con **Instalar un plugin…** en la paleta, dando la dirección del
  repositorio.

## Tu primer plugin

En esta sección se hace un plugin completo con cuatro archivos. Cópialo,
cambia los nombres y tendrás un punto de partida para tu MUD.

El MUD del ejemplo se llama Example y está en `mud.example.org`. Manda la vida
y la energía por GMCP en `Char.Vitals`. Manda los mensajes de los canales en
`Comm.Message`. La carpeta del plugin es `example-mud/`.

**Paso 1.** Escribe `plugin.toml`. Dice qué es el plugin y cuándo ofrecerlo:

```toml
name = "Example MUD"
about = { en = "Its gauges and channels.", es = "Sus indicadores y canales." }
hosts = ["mud.example.org"]
names = ["example"]
muddie = "0.5.0"
```

**Paso 2.** Escribe `rules.toml` con los indicadores. Cada indicador sigue lo
que manda el MUD sin nada de código. El jugador puede cambiarlos en el editor
como si los hubiera hecho él.

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

**Paso 3.** Escribe `example.luau`. Le pide GMCP al MUD y pone cada canal en
su pestaña:

```lua
-- Example MUD: its channels, each in its tab. Its gauges are in rules.toml.
gmcp.support("Char 1", "Comm 1")

on("gmcp", "Comm.Message", function(message)
    if type(message) == "table" and message.channel and message.text then
        channels.say(message.channel, message.text)
    end
end)
```

**Paso 4.** Escribe `template.toml`. Con él, **Perfil nuevo** ofrece el MUD
listo para jugar:

```toml
name = "Example MUD"
host = "mud.example.org"
port = 4000
plugins = ["example-mud"]
```

Ya está. Cuando un jugador conecta a `mud.example.org`, Muddie le ofrece el
plugin. Una vez activado, el jugador ve los indicadores y los oye cuando
bajan. Cada canal tiene su pestaña, y Muddie lee cada mensaje en voz alta
después del nombre del canal.

## Pruébalo mientras lo escribes

Puedes usar tu plugin en Muddie mucho antes de publicarlo. Sigue estos pasos:

1. Pon la carpeta del plugin dentro de la carpeta `plugins` de la carpeta de
   configuración de Muddie. En Linux es `~/.config/muddie/plugins/example-mud/`.
   En Windows es `%APPDATA%\muddie\plugins\example-mud\`. Muddie lo trata
   como un plugin de git.
2. Abre la ventana **Perfiles**, ve a la pestaña **Plugins** del perfil y
   activa el plugin. Sus scripts aparecen en **Scripts**, en una carpeta con
   su nombre. Sus indicadores aparecen en **Indicadores**.
3. Cada vez que cambies los archivos del plugin, abre su menú en la pestaña
   **Plugins** y elige **Instalar de nuevo**. El perfil recibe los archivos
   tal como están ahora.

Para probar más rápido, cambia la copia instalada en el editor y ejecútala
allí. Cuando funcione, copia el cambio a la carpeta del plugin.

### Ver qué pasa

Estas herramientas te enseñan lo que manda el MUD y lo que hace tu plugin:

- **`session.feed("text")`** hace como si el MUD hubiera mandado ese texto.
  Escríbelo en **Ejecutar Luau…** de la paleta. Úsalo para probar un
  disparador sin conectar.
- **`muddie --debug | tee muddie.log`** muestra todo lo que pasa. Ves el texto
  (`TEXT`, `PROMPT`, `PARTIAL`), lo que se manda, cada mensaje GMCP (`GMCP<`),
  lo que los scripts escriben con `log` y los errores de los scripts
  (`SCRIPT`). Mira aquí primero para saber qué manda un MUD y para encontrar
  lo que falla.
- **Los errores de los scripts** salen en la esquina. El editor también los
  guarda, con el archivo y la línea.
- **[muddie.d.luau](muddie.d.luau)** tiene todas las funciones con sus tipos.
  Dáselo a un editor con luau-lsp, o a `luau-analyze`. Así, un nombre mal
  escrito se marca en cuanto lo escribes.

## Los archivos en detalle

Esta sección explica cada campo de cada archivo. Vuelve a ella cuando el
primer plugin se te quede corto.

### `plugin.toml`

Este archivo le dice a Muddie qué es el plugin y cuándo ofrecerlo.

```toml
name = { en = "Example MUD", es = "Ejemplo" }   # a text, or one by language
about = "What it does, in a sentence."          # the same
hosts = ["mud.example.org"]   # part of the address of the MUDs it is for
names = ["example"]           # or the names they give themselves (MSSP)
default_on = false            # on in every profile until switched off
muddie = "0.5.0"              # the oldest Muddie it works with
```

- **`name` y `about`** salen en la pestaña **Plugins** y en el ofrecimiento.
  Cada uno es un texto, o una tabla por código de idioma (`en`, `es`, etc.),
  igual que lo que acepta [`tr`](scripting.es.md#trtexts). El nombre en
  inglés es también el nombre de la carpeta en la que se instala el plugin, en
  cada perfil.
- **`hosts` y `names`** deciden cuándo ofrece Muddie el plugin. Lo ofrece
  cuando la dirección contiene uno de `hosts`. También lo ofrece cuando el MUD
  dice por MSSP que se llama como uno de `names`.
- **`default_on`** activa el plugin en todos los perfiles hasta que el jugador
  lo desactive. Para un plugin de un solo MUD, déjalo desactivado. Muddie lo
  ofrece igualmente cuando el jugador conecta a ese MUD.
- **`muddie`** es el Muddie más antiguo con el que funciona el plugin. Un
  Muddie más antiguo se queda con la versión del plugin que ya tiene.
- **No se admite ningún otro campo.** Cualquier otro campo es un error, y
  Muddie rechaza el plugin.

### `rules.toml`

Este archivo trae las reglas del plugin. Usa reglas para lo que el jugador
querrá cambiar en el editor, como los colores y los pasos de un indicador.
Todo lo demás, hazlo en un script.

Se escribe igual que el `rules.toml` de un perfil. Cada regla va en una
carpeta con el nombre del plugin. `folder = "Combat"` pone una regla en una
carpeta llamada Combat dentro de esa. Todas las clases de regla admiten
`name`, `folder`, `tags` y `enabled`.

#### Indicadores

Un indicador se escribe como `[[gauge]]`. Muestra un valor que manda el MUD.

| Campo | Qué es |
|---|---|
| `name` | El nombre del indicador, tal como se muestra y se dice |
| `value` | De dónde sale el valor: una ruta de GMCP (`Char.Vitals.hp`), una variable de MSDP o una variable de `vars` |
| `max` | De dónde sale el máximo, o un número |
| `kind` | `"bar"`, `"cells"`, `"number"`, `"light"`, `"text"` o `"effects"` |
| `colour`, `off_colour` | Un nombre de color o `#rrggbb` |
| `steps` | El color por debajo de cada porcentaje, por ejemplo `[{ under = 30, colour = "#e05050" }]` |
| `aloud` | `"usual"`, `"lowest"`, `"wait"`, `"cut"` o `"never"`, como en [`gauges.set`](scripting.es.md#gaugessetname-gauge) |
| `always` | Con `true` se oye aunque la lectura del juego esté apagada |
| `panel` | El nombre del panel de indicadores en el que sale. Muddie crea el panel si no existe |

#### Disparadores

Un disparador se escribe como `[[trigger]]`. Reacciona a una línea del MUD.

- **`pattern`** es un texto que contiene la línea. Con `regex = true`, es una
  expresión regular.
- Después di qué hace el disparador, con uno o varios de estos campos:
  - **`send`**: órdenes que mandar, una por línea o separadas por `;`. Usa
    `%1`, etc. para lo que ha coincidido, y `$name` para una variable.
  - **`speak`**: un texto que decir en voz alta.
  - **`sound`**: un sonido que tocar.
  - **`highlight`**: un color para la línea.
  - **`panel`**: un panel en el que mostrar la línea.
  - **`gag = true`**: oculta la línea.
  - **`silence = true`**: muestra la línea pero no la lee.
  - **`code`** con **`luau = true`**: código Luau. Recibe `m` y `line`, igual
    que la función de un disparador en un script.

#### Alias, teclas y temporizadores

- **Un alias**, `[[alias]]`, reacciona a lo que escribe el jugador. Su
  `pattern` es la primera palabra escrita, o una expresión regular con
  `regex = true`. Hace `send` o `code`.
- **Una tecla**, `[[macro]]`, reacciona al pulsar una tecla. Su `key` es un
  nombre como `"f5"`, `"ctrl+k"` o `"numpad8"`. Hace `send` o `code`.
- **Un temporizador**, `[[timer]]`, se ejecuta pasado un tiempo. Su `seconds`
  dice cuánto. Con `repeat = true` se repite una y otra vez. Hace `send` o
  `code`.

### `template.toml`

Este archivo es un perfil ya hecho para el MUD. **Perfil nuevo** lo ofrece en
la lista de MUD a los que jugar.

```toml
name = "Example MUD"        # the game, which the profile is named after
host = "mud.example.org"
port = 4000
tls = false
encoding = "utf-8"          # optional: as the MUD says, if not given
language = "es"             # optional: the language the game is played in
plugins = ["example-mud"]   # the plugins switched on, by their ids
```

### Los scripts

En los scripts es donde el plugin hace su trabajo. Ten en cuenta estas
normas:

- **Ponles nombre en inglés y sin espacios.** Por ejemplo `example.luau`,
  `aloud.luau` y `prompt.luau`. Muddie los ejecuta en el orden de sus nombres.
- **Se instalan en el perfil del jugador**, en una carpeta con el nombre del
  plugin. El jugador puede leerlos y cambiarlos en el editor. Cuando el plugin
  se desactiva, sus scripts también se desactivan.
- **Comparte código con módulos.** `require("prompt")` carga `prompt.luau` de
  la propia carpeta del plugin. Lo carga una sola vez, para todos los scripts
  del plugin. Muddie también ejecuta como script cada `.luau` de la carpeta,
  así que un módulo solo debe devolver lo que tiene.
- **Pon todo lo que se oye en su propio script**, `aloud.luau`. Así el jugador
  puede desactivar lo que se oye y quedarse con lo que se ve. Mira
  [Hacerlo bien](#hacerlo-bien).

## Cómo lo instala y lo actualiza Muddie

No tienes que hacer nada para que esto pase. Léelo para saber con qué se
queda el jugador.

- **Cuando el jugador lo activa** en un perfil, Muddie instala el plugin en
  ese perfil. Los scripts van a una carpeta con el nombre del plugin, entre
  los scripts del perfil. Las reglas van a una carpeta con el nombre del
  plugin, entre las reglas del perfil. Los indicadores van a los paneles que
  nombran. Desde ese momento todo es del jugador, y puede cambiarlo.
- **Cuando el jugador lo desactiva**, esas carpetas se desactivan. Cuando lo
  vuelve a activar, lo que había desactivado dentro sigue desactivado.
- **Cuando hay una versión nueva**, oficial o de git, Muddie la instala al
  abrir el perfil. Nunca actualiza en mitad de una partida.
  - Los scripts y las reglas que la versión nueva ya no tiene desaparecen.
  - Un script que el jugador cambió se guarda junto al nuevo, como
    `name.luau.before-update`.
  - Lo que el jugador añadió se queda.
- **Instalar de nuevo**, en el menú del plugin, vuelve a poner todo lo que
  instaló, tal como viene en el plugin.
- **En una sesión sin perfil**, el plugin se ejecuta tal como viene hasta que
  acaba la sesión.

## Hacerlo bien

Estas costumbres hacen que un plugin sea agradable, para quien mira y para
quien escucha.

- **Lo prepara todo él solo.** Al activarse, el plugin le pide al MUD lo que
  necesita (`gmcp.support`). Pone sus indicadores y paneles. Le dice al
  jugador, una vez, lo que tenga que hacer en el juego. El jugador nunca
  prepara nada a mano.
- **Manda lo que dice el MUD a datos, no a la pantalla.** Datos son los
  indicadores, los canales, el mapa y `vars`. Un panel es solo una forma de
  ver esos datos. Quien escucha recibe los mismos datos: consulta los
  indicadores con Alt+0 y los oye cuando cambian.
- **Saca lo que se oye de los datos, no de lo que se ve.** Escríbelo en
  `aloud.luau`, y escucha lo que manda el MUD, con `on("gmcp")` y
  disparadores. Nunca escuches un indicador o un panel, porque el jugador
  puede borrarlo.
- **Haz poco ruido.** Di lo que cambia, no todo otra vez. Con los estados, di
  el que llega y el que se va, no la lista entera.
  - Di lo importante con `aloud = "cut"`, y lo demás después de lo que se
    esté leyendo.
  - Usa `line:mute()` o `line:read(shorter)` para las líneas que dicen
    demasiado.
  - Deja `always = true` para lo urgente.
- **Escribe todos los textos en inglés y en español**, como una tabla por
  idioma: `notify{en = ..., es = ...}`, `tr{...}`. El jugador ve cada texto en
  el idioma de la interfaz.
- **Avisa al jugador una sola vez.** Hay cosas que el jugador tiene que saber,
  como un módulo que activar en el juego o un mapa más nuevo. Díselas una vez.
  Guarda en `vars` que ya lo dijiste, si debe quedar dicho.
- **Guarda en `vars` lo que elige el jugador**, con nombres propios del
  plugin, como `vars.example_learning`.
- **No toques las teclas del jugador.** El plugin de un MUD no cambia las
  teclas de Muddie: el jugador se las aprende una vez, para todos los MUD.
  Las teclas van en un plugin propio, que elige el jugador (mira
  [Compartir un juego de teclas](#compartir-un-juego-de-teclas)).
- **Deja el MUD fuera de Muddie.** Si el plugin necesita algo que Muddie no
  tiene, pide una función nueva de la API. Tiene que servir para cualquier
  MUD.

## Recetas

Cada receta resuelve una necesidad habitual. Copia el código y cambia los
nombres de GMCP y los patrones para que encajen con tu MUD.

### Mostrar la vida y la energía

Escribe los indicadores en `rules.toml`, como en
[tu primer plugin](#tu-primer-plugin).

Algunos MUD solo mandan lo que ha cambiado. Por ejemplo, mandan solo la vida
cuando solo ha cambiado la vida. Pídele a Muddie que combine cada mensaje con
lo que llegó antes, para que los indicadores tengan siempre todos los
valores:

```lua
gmcp.merge("Char.Status")
```

### Mostrar los estados del personaje

Este script muestra los estados del personaje como un indicador de efectos:

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

Para decir en voz alta qué estados llegan y cuáles se van, añade esto a
`aloud.luau`. Lee los mismos datos de GMCP, no el indicador:

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

### Poner cada canal en su pestaña

`channels.say` pone un mensaje en la pestaña de su canal y lo lee en voz alta.
Los mensajes de antes de que llegara el jugador se guardan pero no se leen:

```lua
on("gmcp", "Comm.Message", function(message)
    channels.say(message.channel, message.text)
end)
-- What was said before the player came: kept, not read.
on("gmcp", "Comm.History", function(message)
    channels.say(message.channel, message.text, {aloud = "never"})
end)
```

### Seguir al jugador en el mapa

La mayoría de los MUD mandan `Room.Info`. Si el tuyo lo hace, el plugin
estándar ya dibuja el mapa y no necesitas nada.

Si tu MUD dice en qué sala está el jugador con un GMCP propio, apaga el mapa
estándar y dale al mapa cada sala:

```lua
map.native = false
on("gmcp", "Room.Info", function(room)
    map.visit({id = room.vnum, name = room.name, area = room.zone, exits = room.exits})
end)
```

Si tu MUD solo escribe la sala en su texto, el mapa sigue al jugador por el
nombre y las salidas de la sala. También puede aprender lo que le falta:

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

Para que el jugador elija si el mapa aprende, añade una opción. Sale en el
menú del mapa y en la paleta, y Muddie la recuerda:

```lua
map.action({en = "Learn the map as you go", es = "Aprender el mapa sobre la marcha"},
    function(on)
        vars.example_learning = on
        map.learn = on
    end,
    {checked = map.learn})
```

### Callar el prompt

Este disparador hace que el prompt no se lea. Si detrás del prompt viene algo
en la misma línea, lee solo eso:

```lua
trigger("^HP:\\d+ SP:\\d+> ?(.*)$", function(m, line)
    if m[1] == "" then line:mute() else line:read(m[1]) end
end)
```

En algunos MUD cada jugador se diseña su propio prompt. En ese caso, aprende
el prompt tal como lo muestra el juego, guárdalo en `vars` y saca de él el
patrón. El plugin de Reinos de Leyenda, en muddie-plugins, lo hace así en su
`prompt.luau`.

### Avisar al jugador una vez

Este script espera 90 segundos después de conectar. Si no ha llegado la vida
por GMCP, le dice al jugador cómo activarla y guarda en `vars` que ya se lo
dijo:

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

### Compartir un juego de teclas

Un plugin puede no tener más que teclas: un juego que alguien se ha hecho a
su gusto, para que otros lo instalen y lo ajusten. No lleva `hosts`, así que
Muddie no lo ofrece para ningún MUD; cada jugador lo activa en los perfiles
que quiera.

`plugin.toml`:

```toml
name = "Keypad keys"
about = { en = "Explore and read with the keypad.", es = "Explorar y leer con el teclado numérico." }
muddie = "0.5.0"
```

`keys.luau`:

```lua
-- keys.luau: Muddie's keys, as this set has them. Change what you like;
-- what is not here keeps Muddie's own keys.
local mine = {
    explore = "numpad0",
    read_previous = "numpad8",
    read_next = "numpad2",
    read_last = "numpad5",
    map_north = {"up", "numpad8"},
    map_south = {"down", "numpad2"},
}
for name, keys in mine do
    shortcuts[name] = keys
end
```

Todas las teclas de Muddie, por su nombre, están en
[la referencia](scripting.es.md#todos-los-atajos). Quien lo instala cambia la
lista en el editor, en **Scripts**, y lo apaga para recuperar las teclas de
Muddie.

## Publicarlo

Hay dos formas de compartir tu plugin. Elige una.

### En un repositorio propio

1. Pon los archivos del plugin en la raíz del repositorio.
2. Da a los jugadores su dirección. Lo instalan con **Instalar un plugin…** en
   la paleta.
3. Los jugadores lo ponen al día con **Actualizar un plugin…**.

### Como plugin oficial

Los plugins oficiales llegan a todos los jugadores de Muddie, y Muddie los
actualiza él solo.

1. Pon el plugin en una carpeta con el nombre de su id, en una copia de
   [muddie-plugins](https://github.com/midnattsol/muddie-plugins). Allí el
   plugin tiene que llevar también `rules.toml` y `template.toml`, y ninguna
   carpeta dentro.
2. Pasa las comprobaciones del repositorio:

```sh
python3 tools/check.py   # the files of every plugin
python3 tools/api.py     # the use of the API, with luau-analyze
python3 tools/pack.py    # plugins.zip, as Muddie downloads it
```

3. Abre una pull request.

Cuando se acepte, release-please saca una versión, y todos los Muddie la toman
solos.

## Escribir un plugin con una IA

Una IA puede escribir un buen plugin si le das el material adecuado. Sigue
estos pasos:

1. Dale esta guía, [plugins.es.md](plugins.es.md).
2. Dale la referencia, [scripting.es.md](scripting.es.md), y los tipos,
   [muddie.d.luau](muddie.d.luau).
3. Dale **el registro de una sesión real**, hecho con
   `muddie --debug | tee session.log`. En esa sesión, entra, camina, pelea una
   vez y usa los canales. El registro muestra lo que manda el MUD, línea a
   línea y por GMCP. Ninguna descripción lo sustituye. Quita tu contraseña si
   la escribiste donde el juego la muestra.
4. Dale el plugin de Reinos de Leyenda, de muddie-plugins, como ejemplo de un
   plugin completo.
5. Pídele el plugin. Por ejemplo:

   > Haz un plugin de Muddie para el MUD de session.log, siguiendo
   > plugins.es.md y scripting.es.md, y usando solo las funciones de
   > muddie.d.luau. Indicadores para lo que manda por GMCP, sus canales cada
   > uno en una pestaña, el mapa siguiendo al jugador y, en aloud.luau, que
   > solo se diga en voz alta lo que cambia. Todos los textos en inglés y en
   > español. Dame cada archivo completo.

6. Prueba el resultado como en
   [Pruébalo mientras lo escribes](#pruébalo-mientras-lo-escribes).
7. Cuando algo falle, dale a la IA el error tal como lo muestra Muddie, y las
   líneas del registro de alrededor.

Antes de usar el plugin, comprueba que:

- [ ] `plugin.toml` solo tiene los campos de arriba, y `muddie = "0.5.0"` o posterior.
- [ ] Los scripts tienen nombre en inglés y sin espacios, y lo que se oye está en `aloud.luau`.
- [ ] Solo usa funciones de `muddie.d.luau`, y `luau-analyze` no encuentra nada mal escrito.
- [ ] Nada se dice dos veces: un script no repite lo que ya dice un indicador.
- [ ] Todos los textos que ve el jugador están en inglés y en español.
- [ ] Al activarlo en un perfil nuevo, funciona sin tocar nada a mano.
