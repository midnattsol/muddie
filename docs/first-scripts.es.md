# Tus primeros scripts

English: [first-scripts.md](first-scripts.md).

Esta guía es para ti si nunca has escrito un script. Vas a hacer unos cuantos
pequeños, paso a paso, y cada uno te servirá en una partida de verdad. No
hace falta saber programar: en cada paso verás el código, qué hace y qué vas
a ver y a oír.

Cuando quieras todos los detalles de una función, búscala en la
[referencia de scripts](scripting.es.md). Cuando quieras compartir tu trabajo
con otros jugadores, lee [cómo hacer un plugin](plugins.es.md).

## Contenido

1. [Dónde van los scripts](#dónde-van-los-scripts)
2. [Tu primer alias](#tu-primer-alias)
3. [Recordar cosas](#recordar-cosas)
4. [Tu primer disparador](#tu-primer-disparador)
5. [Decir las cosas en voz alta, bien](#decir-las-cosas-en-voz-alta-bien)
6. [Teclas y temporizadores](#teclas-y-temporizadores)
7. [Hacer las cosas por pasos](#hacer-las-cosas-por-pasos)
8. [El estado de tu personaje](#el-estado-de-tu-personaje)
9. [Encender y apagar](#encender-y-apagar)
10. [Cuando algo falla](#cuando-algo-falla)
11. [¿Cuál uso?](#cuál-uso)

## Dónde van los scripts

Un script es un archivo de texto corto con instrucciones para Muddie. Muddie
lo ejecuta al empezar la sesión, y sigue funcionando mientras juegas.

Los scripts se escriben en Luau, un lenguaje sencillo. Solo necesitas unas
pocas palabras, y esta guía te las enseña según aparecen.

Para hacer un script:

1. Abre el **Editor** con Ctrl+E.
2. Ve a la sección **Scripts**.
3. Pulsa **+** y elige **Script vacío**. El **+** también te ofrece
   ejemplos para empezar.
4. Escribe el código. Un momento después de que dejes de escribir, Muddie
   guarda el script y lo ejecuta. El triángulo junto a su nombre lo ejecuta
   en el momento.

Pulsa otra vez Ctrl+E, o Escape, para volver al juego. La sesión sigue en
marcha mientras estás en el editor.

Hay otras dos formas de ejecutar código:

- **Ejecutar Luau…**, en la paleta (Ctrl+K), ejecuta una línea en el momento.
  Viene bien para probar algo.
- **Reiniciar scripts**, en la paleta, vuelve a ejecutar todos los scripts
  desde el principio.

Un script del editor es de tu perfil. Si quieres un script en todos los
perfiles, escríbelo en `init.luau`, en la carpeta de configuración de Muddie.

## Tu primer alias

Un alias es un atajo para algo que escribes. Escribes una palabra corta, y
Muddie manda por ti la orden larga.

Imagina que estás harto de escribir `kill orc`. Con este alias, escribes
`k orc`:

```lua
alias("^k (.+)$", function(m)
    send("kill " .. m[1])
end)
```

Qué significa cada parte:

- `"^k (.+)$"` es el patrón: lo que escribes tiene que empezar por `k` y un
  espacio. El `(.+)` coge lo que venga detrás.
- `m[1]` es lo que cogió `(.+)`: `orc`.
- `send(...)` manda la orden al juego.
- `..` junta dos textos: `"kill "` y `"orc"` forman `"kill orc"`.

Escribe `k orc`, y al juego le llega `kill orc`.

Fíjate en el espacio dentro de `"kill "`. Sin él, al juego le llegaría
`killorc`:

```lua
-- Wrong: sends "killorc", with no space.
alias("^k (.+)$", function(m) send("kill" .. m[1]) end)
```

## Recordar cosas

`vars` te guarda cosas, como contra quién estás luchando. Lo que pones en
`vars` sigue ahí cuando cierras Muddie. También lo ves en la sección
**Variables** del editor.

Con estos dos alias eliges un objetivo una vez, y luego lo atacas con una
sola letra:

```lua
-- "t orc" chooses the target.
alias("^t (.+)$", function(m)
    vars.target = m[1]
    echo("Target: " .. m[1])
end)

-- "a" attacks it.
alias("^a$", function()
    if vars.target then
        send("kill " .. vars.target)
    else
        echo("Choose a target first, with t.")
    end
end)
```

`echo` muestra una línea en la salida. Si un lector de pantalla lee Muddie,
también lee esa línea.

El `if ... then ... else ... end` comprueba que hay objetivo antes de atacar.
Sin él, `a` sin objetivo daría un error.

## Tu primer disparador

Un disparador reacciona a lo que dice el juego. Cuando una línea encaja con
su patrón, Muddie ejecuta tu código.

### Reaccionar a una línea

Cuando el juego dice que tienes hambre, come:

```lua
trigger("^You are hungry", function()
    send("eat bread")
end)
```

El `^` significa «al principio de la línea». Así, el disparador salta con
`You are hungry.` pero no con `The orc says: You are hungry`.

### Cambiar cómo se ve y se oye una línea

El disparador también recibe la línea, como `line`. Con ella puedes cambiar
cómo se muestra y cómo se lee:

```lua
-- Paint tells in yellow.
trigger("tells you", function(m, line)
    line:paint("yellow")
end)

-- Hide the advertising lines.
trigger("^\\[Ad\\]", function(m, line)
    line:hide()
end)

-- Read a long line shorter, and show it as it is.
trigger("^You slash the (\\w+) with your sword", function(m, line)
    line:read("Hit " .. m[1])
end)
```

`line:read` solo cambia lo que se oye: en pantalla sigue la línea entera.

### Una línea no es su texto

Para comparar lo que dice una línea, usa `line.text`. Si comparas `line` a
secas, nunca coincide, y Luau no te avisa:

```lua
trigger("^Done", function(m, line)
    -- Wrong: a line is never equal to a text, so this never runs.
    if line == "Done." then send("rest") end
    -- Right: line.text is what the line says.
    if line.text == "Done." then send("rest") end
end)
```

## Decir las cosas en voz alta, bien

Muddie te da tres formas de decirte algo:

| Función | Se ve | Se lee en voz alta |
|---|---|---|
| `echo` | En la salida | Sí |
| `speak` | No | Sí |
| `notify` | En la esquina inferior derecha, y luego bajo la campana | Sí, aunque estés en otra sesión o tengas apagada la lectura del juego |

Las tres admiten `aloud`, que dice cuándo se oye:

- `aloud = "cut"` corta lo que se esté leyendo. Úsalo para lo urgente.
- `aloud = "wait"` espera a que el lector calle. Es lo que pasa si no dices
  nada.
- `aloud = "never"` no se oye.

Y con `always = true` se oye aunque hayas apagado la lectura del juego.
Guárdalo para lo que no te puedes perder nunca:

```lua
trigger("^You are bleeding badly", function()
    speak("Bleeding!", {aloud = "cut", always = true})
end)
```

### Di lo que cambia, no todo

Cuanto más habla un script, menos oyes el juego. Este script repite cada
línea, así que lo oyes todo dos veces:

```lua
-- Too much: every line is said again.
trigger(".", function(m, line)
    speak(line.text)
end)
```

Este dice solo lo que importa, una vez y en pocas palabras:

```lua
-- Better: one short word, only when it happens.
trigger("^The (\\w+) arrives", function(m)
    speak(m[1] .. " here")
end)
```

## Teclas y temporizadores

### Caminar con el teclado numérico

`macro` manda una orden al pulsar una tecla. Con estas, el teclado numérico
camina:

```lua
macro("numpad8", "north")
macro("numpad2", "south")
macro("numpad4", "west")
macro("numpad6", "east")
macro("numpad5", "look")
```

También sirven las teclas de función, y las teclas con Ctrl o Alt:
`macro("f5", "score")`, `macro("ctrl+h", "heal")`.

### Cambiar las teclas de Muddie

Las teclas de Muddie también se pueden cambiar: la que explora el mapa, las
que leen las líneas, todas. Dale a `shortcuts` el nombre de una y sus teclas
nuevas:

```lua
-- Explore the map with F9, and read the last line with keypad 5.
shortcuts.explore = "f9"
shortcuts.read_last = "numpad5"
```

Alt+E ya no explora: lo hace F9. Para ver cada nombre con sus teclas, ejecuta
`for _, s in shortcuts.all() do echo(s.name .. ": " .. table.concat(s.keys, ", ")) end`
en **Ejecutar Luau…**. Apaga el script y vuelven las teclas de Muddie. También
puedes cambiarlas en **Ajustes → Teclas**.

### Hacer algo cada cierto tiempo

`every` ejecuta código una y otra vez. Esto guarda tu personaje cada cinco
minutos:

```lua
every(300, function()
    send("save")
end)
```

`after` ejecuta código una sola vez, pasado un rato:
`after(10, function() send("stand") end)`.

## Hacer las cosas por pasos

Hay cosas que llevan varios pasos: bebes una poción, esperas a ver si ha
funcionado y lo intentas otra vez. Con `wait` las escribes una detrás de
otra:

```lua
alias("^cure$", function()
    send("drink potion")
    local m = wait("line", "^You feel better", {timeout = 3})
    if m then
        echo("Cured.")
        return
    end
    -- It did not work in 3 seconds: try once more.
    send("drink potion")
end)
```

`wait("line", ...)` para este alias hasta que el juego dice
`You feel better`, o hasta que pasan 3 segundos. Mientras tanto, el resto de
Muddie y tus otros scripts siguen funcionando.

`wait(2)` espera dos segundos.

## El estado de tu personaje

Un indicador muestra un valor, como tu vida, en forma de barra. También
habla cuando el valor baja. Con lector de pantalla, oyes todos tus
indicadores con Alt+0.

Muchos juegos mandan tu vida por GMCP, un canal de datos que viene junto al
texto. Si tu juego la manda en `Char.Vitals`, esto hace un indicador con ella:

```lua
on("gmcp", "Char.Vitals", function(vitals)
    gauges.set("Life", {value = vitals.hp, max = vitals.maxhp})
end)
```

`on` escucha algo, aquí el mensaje GMCP `Char.Vitals`. Los nombres `hp` y
`maxhp` cambian de un juego a otro. Para ver lo que manda el tuyo, arranca
Muddie con `--debug` (mira [Cuando algo falla](#cuando-algo-falla)).

Ahora, un aviso cuando la vida baja del 30 %. Salta una vez, y no vuelve a
saltar hasta que tu vida se recupere:

```lua
on("gauge", "Life", function()
    speak("Life low!", {aloud = "cut"})
end, {under = 30})
```

## Encender y apagar

A veces quieres que un script descanse: que no comas solo mientras haces una
misión, por ejemplo. Dale un sitio, y apaga ese sitio.

```lua
trigger("^You are hungry", function()
    send("eat bread")
end, {place = "Food/Eat"})

alias("^nofood$", function()
    triggers.disable("Food")
    echo("Automatic eating off.")
end)

alias("^food$", function()
    triggers.enable("Food")
    echo("Automatic eating on.")
end)
```

`place = "Food/Eat"` mete el disparador en una carpeta llamada `Food`. Si
apagas la carpeta, se apaga todo lo que hay dentro.

Las etiquetas hacen lo mismo con cosas de distinta clase: ponles
`tags = {"combat"}` a disparadores, alias y teclas, y `tags.disable("combat")`
los apaga todos a la vez.

## Cuando algo falla

Todo el mundo se equivoca. Muddie te dice dónde.

- **El mensaje de error.** Un error sale en la esquina inferior derecha, por
  ejemplo `combat.luau:12: there is no function gauges.sett`. Te dice el
  script, la línea y qué está mal. El editor también marca esa línea.
- **Probar un disparador sin el juego.** En **Ejecutar Luau…**, escribe
  `session.feed("You are hungry.")`. Muddie toma el texto como si lo hubiera
  mandado el juego, y tus disparadores reaccionan.
- **Verlo todo.** Arranca Muddie con `muddie --debug`. En la terminal ves
  cada línea que llega, cada orden que se manda, cada mensaje GMCP y cada
  error. Para guardarlo en un archivo: `muddie --debug | tee muddie.log`.
- **Pillar las erratas antes de ejecutar.** [muddie.d.luau](muddie.d.luau)
  tiene todas las funciones. Dáselo a `luau-analyze`, o a un editor con
  luau-lsp, y te marcará un nombre mal escrito según lo escribes.

## ¿Cuál uso?

| Quieres… | Usa |
|---|---|
| Escribir menos | Un alias: `alias` |
| Reaccionar a lo que dice el juego | Un disparador: `trigger` |
| Hacer algo con una tecla | Una tecla: `macro` |
| Hacer algo cada cierto tiempo, o más tarde | Un temporizador: `every`, `after` |
| Recordar algo, aunque cierres Muddie | `vars` |
| Mostrar una línea | `echo` |
| Decir algo sin mostrarlo | `speak` |
| Avisarte de algo importante | `notify` |
| Hacer algo por pasos | `wait` |
| Ver tu vida, tu energía, etc. | Un indicador: `gauges.set` |
| Reaccionar a los datos que manda el juego | `on` |
| Apagar cosas un rato | `place`, `tags`, `triggers.disable` |
| Cambiar las teclas de Muddie | `shortcuts` |

### Por dónde seguir

- La [referencia de scripts](scripting.es.md) tiene todas las funciones, con
  todas sus opciones. Por ejemplo:
  [`trigger`](scripting.es.md#triggerpattern-fn-options),
  [`speak`](scripting.es.md#speaktext-options),
  [`wait`](scripting.es.md#waitline-pattern-options) y
  [`gauges.set`](scripting.es.md#gaugessetname-gauge).
- [Cómo hacer un plugin](plugins.es.md) te enseña a reunir tus scripts y
  compartirlos con todos los jugadores de tu juego.
