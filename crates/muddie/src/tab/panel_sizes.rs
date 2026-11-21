//! How big the panels and the window are, in characters, for the scripts
//! and the MUD that draw to their size.

use super::*;

/// How many characters fit across and down a panel that can show a
/// drawing, what scripts were last told of it, and since when it has
/// been as it is.
pub(super) struct PanelSize {
    id: u32,
    /// Whether it is showing a drawing, or is the map's: only then are
    /// scripts told when its size changes, though they can always ask.
    drawing: bool,
    cells: (u32, u32),
    told: Option<(u32, u32)>,
    settling: Option<((u32, u32), Instant)>,
}

impl Tab {
    /// Tells a new script runtime the sizes the old one knew.
    pub(super) fn retell_panel_sizes(&mut self) {
        let sizes: Vec<(u32, (u32, u32))> = (self.sizes.panels.iter())
            .map(|size| (size.id, size.cells))
            .collect();
        for (id, cells) in sizes {
            self.panel_size_known(id, cells);
        }
    }

    /// The size in characters of a panel `size` points wide and tall, in
    /// the output's typeface.
    pub(super) fn cells(ui: &egui::Ui, size: egui::Vec2) -> (u32, u32) {
        let font = palette::font();
        let (wide, tall) =
            ui.fonts_mut(|fonts| (fonts.glyph_width(&font, 'm'), fonts.row_height(&font)));
        (
            (size.x / wide).floor().max(0.0) as u32,
            (size.y / tall).floor().max(0.0) as u32,
        )
    }

    /// Notes how many characters fit in the panel `id`, as it is drawn.
    /// Scripts can ask for it at once; they are told of a change once it
    /// has settled.
    pub(super) fn panel_measured(&mut self, id: u32, cells: (u32, u32), drawing: bool) {
        match self.sizes.panels.iter_mut().find(|size| size.id == id) {
            Some(size) => {
                size.drawing = drawing;
                if size.cells == cells {
                    return;
                }
                size.cells = cells;
            }
            None => self.sizes.panels.push(PanelSize {
                id,
                drawing,
                cells,
                told: None,
                settling: None,
            }),
        }
        self.panel_size_known(id, cells);
    }

    /// Lets scripts ask for the size of the panel `id`, by its name, and
    /// as `map` if it is the map's.
    pub(super) fn panel_size_known(&mut self, id: u32, cells: (u32, u32)) {
        let Some(panel) = self.layout.panel(id) else {
            return;
        };
        self.scripts.set_panel_size(&panel.name, cells.0, cells.1);
        if panel.kind == layout::Kind::Map {
            self.scripts.set_panel_size("map", cells.0, cells.1);
        }
    }

    /// Scripts are to be told the size of every panel again.
    pub(super) fn panels_untold(&mut self) {
        for size in &mut self.sizes.panels {
            size.told = None;
        }
    }

    /// Tells scripts how big each panel is once it has stopped changing,
    /// so a window being resized does not flood the server.
    pub(super) fn report_panel(&mut self, ctx: &egui::Context) {
        const SETTLED: Duration = Duration::from_millis(250);
        let mut settled = Vec::new();
        for size in &mut self.sizes.panels {
            let cells = size.cells;
            if size.told == Some(cells) || cells.0 == 0 || !size.drawing {
                size.settling = None;
                continue;
            }
            match size.settling {
                Some((settling, since)) if settling == cells && since.elapsed() >= SETTLED => {
                    size.settling = None;
                    size.told = Some(cells);
                    settled.push((size.id, cells));
                }
                Some((settling, _)) if settling == cells => ctx.request_repaint_after(SETTLED),
                _ => {
                    size.settling = Some((cells, Instant::now()));
                    ctx.request_repaint_after(SETTLED);
                }
            }
        }
        for (id, cells) in settled {
            // Which panel it is: by its name, and whether it is the map's,
            // whatever that one is called.
            let panel = self.layout.panel(id);
            let name = panel.map(|panel| panel.name.clone()).unwrap_or_default();
            let map = panel.is_some_and(|panel| panel.kind == layout::Kind::Map);
            let size = serde_json::json!({
                "name": name,
                "map": map,
                "columns": cells.0,
                "rows": cells.1,
            });
            self.scripts.on_event("panel", &size.to_string());
            self.run_actions();
        }
    }

    /// Tells the server how many characters fit across and down the text
    /// of the game (NAWS), on connecting and once a change of size has
    /// settled, so that it wraps and pages what it sends to fit.
    pub(super) fn report_window(&mut self, ctx: &egui::Context) {
        const SETTLED: Duration = Duration::from_millis(250);
        let cells = self.sizes.window;
        #[cfg(test)]
        if self.sizes.window_untold {
            return;
        }
        let Some(connection) = &self.connection else {
            return self.sizes.window_settling = None;
        };
        if self.sizes.window_reported == Some(cells) || cells.0 == 0 || cells.1 == 0 {
            return self.sizes.window_settling = None;
        }
        match self.sizes.window_settling {
            Some((settling, since)) if settling == cells && since.elapsed() >= SETTLED => {
                self.sizes.window_settling = None;
                self.sizes.window_reported = Some(cells);
                let size = |cells: u32| u16::try_from(cells).unwrap_or(u16::MAX);
                let _ = (connection.commands).send(Command::Resize(size(cells.0), size(cells.1)));
            }
            Some((settling, _)) if settling == cells => ctx.request_repaint_after(SETTLED),
            _ => {
                self.sizes.window_settling = Some((cells, Instant::now()));
                ctx.request_repaint_after(SETTLED);
            }
        }
    }

    /// What the map's panel shows: the map; failing that, what a script
    /// put there, such as a map the MUD draws itself; failing that, why
    /// there is nothing yet.
    pub(super) fn map_body(&mut self, ui: &mut egui::Ui, id: u32) -> Option<MapAction> {
        // What a drawing sent by the server has to fit in: the panel, less
        // its title.
        let room = ui.available_size() - egui::vec2(0.0, 22.0);
        let cells = Self::cells(ui, room);
        self.panel_measured(id, cells, true);
        if self.map.borrow().current().is_some() {
            let map = self.map.borrow();
            let walk = self.walk.as_ref().map(|(walk, _)| walk);
            return self.map_view.show(ui, &map, walk);
        }
        let drawn = !self.text_of(id).fixed.is_empty() || self.text_of(id).log.len() > 0;
        if drawn {
            self.text_body(ui, id, true);
            return None;
        }
        // Nothing to show yet: say why, and where to go from here.
        let theme = theme::current();
        ui.add_space((ui.available_height() * 0.28).max(12.0));
        let mut scripts = false;
        ui.vertical_centered(|ui| {
            let title = RichText::new(tr("No map yet")).font(theme::font(theme::Weight::Medium, 14.5)).color(theme.text);
            ui.add(Label::new(title).selectable(false));
            ui.add_space(4.0);
            let about = tr("It is drawn as you walk, if the MUD says which room you are in. If yours tells it another way, or draws the map itself, a script can take it from there.");
            ui.label(RichText::new(about).font(theme::font(theme::Weight::Regular, 12.5)).color(theme.text_dim));
            ui.add_space(12.0);
            scripts = crate::widgets::button(ui, tr("Set up a mapper")).clicked();
            ui.add_space(4.0);
            if crate::widgets::quiet_button(ui, tr("Hide the map")).clicked() {
                self.set_map_shown(false);
            }
        });
        scripts.then_some(MapAction::Scripts)
    }
}

/// How big the panels and the window are, in characters, and what was told
/// of it.
#[derive(Default)]
pub(super) struct Sizes {
    /// The size in characters of the panel beside the output: as it is,
    /// as scripts were last told, and since when it has been as it is.
    pub(super) panels: Vec<PanelSize>,
    /// How many characters fit across and down the text of the game,
    /// what the server was last told of it, and since when it has been
    /// as it is.
    pub(super) window: (u32, u32),
    pub(super) window_reported: Option<(u32, u32)>,
    pub(super) window_settling: Option<((u32, u32), Instant)>,
    /// Tests of what is sent to the server are not about its size: they
    /// switch the telling of it off.
    #[cfg(test)]
    pub(super) window_untold: bool,
}
