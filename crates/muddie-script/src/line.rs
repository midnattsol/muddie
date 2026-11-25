//! The line a trigger is looking at, as it is given to it: what it says, and
//! what is done with it.

use super::*;
use mlua::{FromLua, MetaMethod, MultiValue, UserData, UserDataFields, UserDataMethods};

/// What is read aloud of a line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Heard {
    /// What it says.
    #[default]
    AsIs,
    /// Nothing.
    Not,
    /// This, in its place.
    Instead(Saying),
}

/// The line the triggers are run on, and what they have done with it.
#[derive(Default)]
pub(super) struct Looked {
    /// Which it is, counting from 1, to tell one given before from it.
    number: u64,
    /// Whether the triggers are being run on it.
    open: bool,
    pub(crate) line: Line,
    /// Where the pattern of the trigger being run matched, in the line as it
    /// came.
    matched: Option<(usize, usize)>,
    /// Where it was replaced, in the line as it came, and how long what was
    /// put there is.
    replaced: Vec<(usize, usize, usize)>,
    hidden: bool,
    heard: Heard,
}

impl Looked {
    /// Begins with another line.
    pub(super) fn begin(&mut self, line: Line) {
        *self = Self {
            number: self.number + 1,
            open: true,
            line,
            ..Self::default()
        };
    }

    /// What its triggers leave of it: nothing, if one hid it.
    pub(super) fn end(&mut self) -> Option<Line> {
        self.open = false;
        // What is shown in its place is what is read, unless told otherwise.
        if !self.replaced.is_empty() && self.heard == Heard::AsIs {
            self.heard = Heard::Instead(self.line.text().into());
        }
        (!self.hidden).then(|| std::mem::take(&mut self.line))
    }

    pub(super) fn heard(&self) -> &Heard {
        &self.heard
    }

    pub(super) fn matched(&mut self, range: (usize, usize)) {
        self.matched = Some(range);
    }

    /// Where what matched is in the line as it now is; nowhere, if a part of
    /// it was replaced.
    fn matched_now(&self) -> Option<std::ops::Range<usize>> {
        let (start, end) = self.matched?;
        let mut moved = 0;
        for &(from, to, long) in &self.replaced {
            if to <= start {
                moved += long as isize - (to - from) as isize;
            } else if from < end {
                return None;
            }
        }
        Some(start.checked_add_signed(moved)?..end.checked_add_signed(moved)?)
    }
}

/// A line as a trigger is given it.
pub(super) struct Given {
    text: String,
    prompt: bool,
    number: u64,
    state: Rc<RefCell<State>>,
}

impl Given {
    pub(super) fn new(text: &str, prompt: bool, state: &Rc<RefCell<State>>) -> Self {
        Self {
            text: text.to_owned(),
            prompt,
            number: state.borrow().looked.number,
            state: Rc::clone(state),
        }
    }

    /// Does something with the line, while its triggers are being run.
    fn with<T>(&self, what: &str, run: impl FnOnce(&mut Looked) -> T) -> mlua::Result<T> {
        let mut state = self.state.borrow_mut();
        if !state.looked.open || state.looked.number != self.number {
            return Err(mlua::Error::runtime(format!(
                "line:{what} is for the line a trigger is looking at, while it does"
            )));
        }
        Ok(run(&mut state.looked))
    }
}

impl UserData for Given {
    fn add_fields<F: UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("text", |_, line| Ok(line.text.clone()));
        fields.add_field_method_get("prompt", |_, line| Ok(line.prompt));
        fields.add_field_method_get("spans", |lua, line| {
            let spans = line.with("spans", |looked| looked.line.spans.clone())?;
            spans_of(lua, &spans)
        });
    }

    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("hide", |_, line, ()| {
            line.with("hide", |looked| looked.hidden = true)
        });
        methods.add_method("mute", |_, line, ()| {
            line.with("mute", |looked| looked.heard = Heard::Not)
        });
        methods.add_method("read", |_, line, (text, options): (Text, AfterText)| {
            let options = options.0;
            only(options.as_ref(), &["aloud", "always"])?;
            let aloud = aloud_of(options.as_ref(), false)?;
            line.with("read", |looked| {
                let nothing = text.0.trim().is_empty() || aloud.how == How::Never;
                looked.heard = match nothing {
                    true => Heard::Not,
                    false => Heard::Instead(Saying {
                        text: text.0,
                        aloud,
                    }),
                };
            })
        });
        methods.add_method("paint", |_, line, (fg, bg): (Value, Value)| {
            let (fg, bg) = (color_from(&fg)?, color_from(&bg)?);
            line.with("paint", |looked| {
                let Some(range) = looked.matched_now() else {
                    return;
                };
                looked.line.restyle(range, |style| {
                    style.fg = fg.unwrap_or(style.fg);
                    style.bg = bg.unwrap_or(style.bg);
                });
            })
        });
        // In place of what matched. Whether it could: not over what another
        // trigger replaced.
        methods.add_method("replace", |_, line, text: Text| {
            line.with("replace", |looked| {
                let (Some(range), Some((from, to))) = (looked.matched_now(), looked.matched) else {
                    return false;
                };
                looked.line.replace(range, &text.0);
                looked.replaced.push((from, to, text.0.len()));
                true
            })
        });
        methods.add_method("to_panel", |_, line, name: String| {
            line.with("to_panel", |_| ())?;
            (line.state.borrow_mut().actions).push(Action::ToPanel(name));
            Ok(())
        });
        // Its text is asked for by its name: taken for the line itself, it
        // would be compared with one too, which Luau never finds the same.
        methods.add_meta_method(MetaMethod::ToString, |_, _, ()| refuse::<()>());
        methods.add_meta_method(MetaMethod::Len, |_, _, ()| refuse::<()>());
        methods.add_meta_function(MetaMethod::Concat, |_, _: MultiValue| refuse::<()>());
        methods.add_meta_method(MetaMethod::Index, |lua, _, name: String| {
            let strings: Table = lua.globals().get("string")?;
            Err::<(), _>(mlua::Error::runtime(
                match strings.contains_key(name.as_str())? {
                    true => format!("there is no line:{name}: use line.text:{name}"),
                    false => format!(
                        "there is no line.{name}; there are text, spans, prompt, hide, mute, read, replace, paint and to_panel"
                    ),
                },
            ))
        });
    }
}

/// A line is not a text, where one is wanted.
fn refuse<T>() -> mlua::Result<T> {
    Err(mlua::Error::runtime("a line is not a text: use line.text"))
}

/// A text a script gives.
pub(super) struct Text(pub(super) String);

impl FromLua for Text {
    fn from_lua(value: Value, lua: &Lua) -> mlua::Result<Self> {
        match &value {
            Value::UserData(given) if given.is::<Given>() => refuse(),
            _ => String::from_lua(value, lua).map(Self),
        }
    }
}

/// The runs of text of a line, each with how it looks.
fn spans_of(lua: &Lua, spans: &[muddie_core::Span]) -> mlua::Result<Table> {
    let list = lua.create_table()?;
    for span in spans {
        let entry = lua.create_table()?;
        entry.set("text", span.text.as_str())?;
        entry.set("fg", color_value(lua, span.style.fg)?)?;
        entry.set("bg", color_value(lua, span.style.bg)?)?;
        entry.set("bold", span.style.bold)?;
        entry.set("italic", span.style.italic)?;
        entry.set("underline", span.style.underline)?;
        entry.set("inverse", span.style.inverse)?;
        list.push(entry)?;
    }
    Ok(list)
}
