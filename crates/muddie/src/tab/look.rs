//! The appearance of the map and of its rooms: the colour and shape the
//! map draws rooms in, and a room's own colour, shape and what is written
//! in it, for it or every room called as it is. Chosen in a dialog over the
//! map, opened from the map's menu or a room's.

use super::*;
use crate::widgets;
use muddie_mapper::{Appearance, Colour, Shape};

/// What the appearance is chosen for.
#[derive(Clone, Copy, PartialEq, Eq)]
enum For {
    /// The whole map: how rooms of no look of their own are drawn.
    Map,
    Room(RoomId),
}

/// The appearance being chosen. For a room, `None` is the map's.
pub(super) struct Look {
    of: For,
    /// The room's name, to choose for every room called so.
    name: String,
    colour: Option<Colour>,
    shape: Option<Shape>,
    symbol: String,
    all: bool,
}

fn shape_name(shape: Shape) -> &'static str {
    match shape {
        Shape::Square => tr("Square"),
        Shape::Circle => tr("Circle"),
        Shape::Diamond => tr("Diamond"),
        Shape::Hexagon => tr("Hexagon"),
    }
}

impl Tab {
    /// Opens the appearance of the whole map, as it is.
    pub(super) fn open_map_appearance(&mut self) {
        let appearance = self.map.borrow().appearance();
        self.look = Some(Look {
            of: For::Map,
            name: String::new(),
            colour: appearance.colour,
            shape: Some(appearance.shape),
            symbol: String::new(),
            all: false,
        });
    }

    /// Opens the appearance of a room, as it is.
    pub(super) fn open_look(&mut self, id: RoomId) {
        let map = self.map.borrow();
        let Some(room) = map.room(id) else {
            return;
        };
        self.look = Some(Look {
            of: For::Room(id),
            name: room.name.clone(),
            colour: room.colour,
            shape: room.shape,
            symbol: room.symbol.clone(),
            all: false,
        });
    }

    /// The dialog of an appearance, while open: applied once chosen.
    pub(super) fn look_window(&mut self, ctx: &egui::Context) {
        let Some(look) = &mut self.look else {
            return;
        };
        let room = matches!(look.of, For::Room(_));
        let called = match room {
            true => self.map.borrow().called(&look.name).len(),
            false => 0,
        };
        let (mut open, mut apply, mut cancel) = (true, false, false);
        let title = match room {
            true => tr("Room appearance"),
            false => tr("Map appearance"),
        };
        // What stands for none: the map's, for a room; the theme's, for
        // the map's colour.
        let usual_colour = match room {
            true => tr("The map's colour"),
            false => tr("The theme's colour"),
        };
        widgets::dialog(
            ctx,
            "appearance",
            title,
            &mut open,
            crate::theme::size::DIALOG_M,
            |ui| {
                colour_choice(ui, look, usual_colour);
                shape_choice(ui, look, room);
                if room {
                    ui.add_space(4.0);
                    widgets::sub_label(ui, tr("Written in it"));
                    widgets::row_field(ui, &mut look.symbol, tr("A character or two"), 160.0);
                }
                if called > 1 {
                    ui.add_space(6.0);
                    let all = trf(
                        "All {0} rooms called {1}",
                        &[&called.to_string(), &look.name],
                    );
                    widgets::switch(ui, &mut look.all, &all);
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    apply = widgets::primary_button(ui, tr("Apply")).clicked();
                    cancel = widgets::quiet_button(ui, tr("Cancel")).clicked();
                });
            },
        );
        if apply {
            self.apply_look();
        }
        if !open || apply || cancel {
            self.look = None;
        }
    }

    /// Gives the map, a room, or every room called as it is, the
    /// appearance chosen.
    fn apply_look(&mut self) {
        let Some(look) = &self.look else {
            return;
        };
        let (colour, shape, symbol) = (look.colour, look.shape, look.symbol.clone());
        let done = match look.of {
            For::Map => self.map.borrow_mut().set_appearance(Appearance {
                colour,
                shape: shape.unwrap_or_default(),
            }),
            For::Room(id) => {
                let rooms = match look.all {
                    true => self.map.borrow().called(&look.name),
                    false => vec![id],
                };
                let mut map = self.map.borrow_mut();
                (rooms.iter()).try_for_each(|id| {
                    map.set_colour(*id, colour)?;
                    map.set_shape(*id, shape)?;
                    map.set_symbol(*id, &symbol).map(drop)
                })
            }
        };
        self.map_saved(done);
    }
}

/// The colours to choose from, and none, which is named `usual_colour`.
fn colour_choice(ui: &mut egui::Ui, look: &mut Look, usual_colour: &str) {
    widgets::sub_label(ui, tr("Colour"));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if widgets::swatch_none(ui, look.colour.is_none(), usual_colour).clicked() {
            look.colour = None;
        }
        for name in crate::panels::COLOUR_CHOICES {
            let Some(shown) = crate::panels::colour_named(name) else {
                continue;
            };
            let colour = [shown.r(), shown.g(), shown.b()];
            if widgets::swatch(ui, shown, look.colour == Some(colour), tr(name)).clicked() {
                look.colour = Some(colour);
            }
        }
    });
}

/// The shapes to choose from.
fn shape_choice(ui: &mut egui::Ui, look: &mut Look, room: bool) {
    ui.add_space(4.0);
    widgets::sub_label(ui, tr("Shape"));
    // A room may take the map's; the map has one always.
    let mut shapes: Vec<Option<Shape>> = Shape::ALL.into_iter().map(Some).collect();
    if room {
        shapes.insert(0, None);
    }
    let names: Vec<&str> = (shapes.iter())
        .map(|shape| shape.map_or(tr("The map's"), shape_name))
        .collect();
    let current = shapes
        .iter()
        .position(|shape| *shape == look.shape)
        .unwrap_or(0);
    if let Some(chosen) = widgets::segmented(ui, &names, current) {
        look.shape = shapes[chosen];
    }
}
