//! How a room looks on the map: its colour, shape and what is written in
//! it, chosen for it, or for every room called as it is, in a dialog over
//! the map opened from the room's menu.

use super::*;
use crate::widgets;
use muddie_mapper::{Colour, Shape};

/// The look being chosen for a room.
pub(super) struct Look {
    room: RoomId,
    /// The room's name, to choose for every room called so.
    name: String,
    colour: Option<Colour>,
    shape: Shape,
    symbol: String,
    all: bool,
}

/// The shapes as offered, in their order.
fn shape_name(shape: Shape) -> &'static str {
    match shape {
        Shape::Square => tr("Square"),
        Shape::Circle => tr("Circle"),
        Shape::Diamond => tr("Diamond"),
        Shape::Hexagon => tr("Hexagon"),
    }
}

impl Tab {
    /// Opens the choice of how a room looks, as it looks now.
    pub(super) fn open_look(&mut self, id: RoomId) {
        let map = self.map.borrow();
        let Some(room) = map.room(id) else {
            return;
        };
        self.look = Some(Look {
            room: id,
            name: room.name.clone(),
            colour: room.colour,
            shape: room.shape,
            symbol: room.symbol.clone(),
            all: false,
        });
    }

    /// The dialog of how a room looks, while open: applied to the room, or
    /// to every room called as it is, once chosen.
    pub(super) fn look_window(&mut self, ctx: &egui::Context) {
        let Some(look) = &mut self.look else {
            return;
        };
        let called = self.map.borrow().called(&look.name).len();
        let (mut open, mut apply, mut cancel) = (true, false, false);
        let title = tr("How the room looks");
        widgets::dialog(
            ctx,
            "room look",
            title,
            &mut open,
            crate::theme::size::DIALOG_M,
            |ui| {
                widgets::sub_label(ui, tr("Colour"));
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if widgets::swatch_none(ui, look.colour.is_none(), tr("The map's own"))
                        .clicked()
                    {
                        look.colour = None;
                    }
                    for name in crate::panels::GAUGE_COLOURS {
                        let Some(shown) = crate::panels::colour_named(name) else {
                            continue;
                        };
                        let colour = [shown.r(), shown.g(), shown.b()];
                        if widgets::swatch(ui, shown, look.colour == Some(colour), tr(name))
                            .clicked()
                        {
                            look.colour = Some(colour);
                        }
                    }
                });
                ui.add_space(4.0);
                widgets::sub_label(ui, tr("Shape"));
                let names: Vec<&str> = Shape::ALL.into_iter().map(shape_name).collect();
                let current = Shape::ALL
                    .iter()
                    .position(|shape| *shape == look.shape)
                    .unwrap_or(0);
                if let Some(chosen) = widgets::segmented(ui, &names, current) {
                    look.shape = Shape::ALL[chosen];
                }
                ui.add_space(4.0);
                widgets::sub_label(ui, tr("Written in it"));
                widgets::row_field(ui, &mut look.symbol, tr("A character or two"), 160.0);
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
            open = false;
        }
        if !open || cancel {
            self.look = None;
        }
    }

    /// Gives the room, or every room called as it is, the look chosen.
    fn apply_look(&mut self) {
        let Some(look) = &self.look else {
            return;
        };
        let rooms = match look.all {
            true => self.map.borrow().called(&look.name),
            false => vec![look.room],
        };
        let (colour, shape, symbol) = (look.colour, look.shape, look.symbol.clone());
        let done = {
            let mut map = self.map.borrow_mut();
            (rooms.iter()).try_for_each(|id| {
                map.set_colour(*id, colour)?;
                map.set_shape(*id, shape)?;
                map.set_symbol(*id, &symbol).map(drop)
            })
        };
        self.map_saved(done);
    }
}
