//! What the user does on the map: the controls over its corners, zooming, panning,
//! moving a room and the room's menu.

use super::*;

impl MapView {
    /// The controls over the map's top corners, in a layer just above it:
    /// the way back to where the player is, at the left, while looking
    /// elsewhere; the levels at the right, a floor above another, where the
    /// area has more than one. Returns the area and level to show.
    pub(super) fn corner_controls(
        &mut self,
        ui: &mut Ui,
        map: &Map,
        (mut area, mut level): (String, i32),
        home: (String, i32),
        rect: Rect,
    ) -> (String, i32) {
        // Levels only where the area has more than one, or one without
        // rooms is being looked at.
        let levels = self.levels_of(map, &area);
        let several_levels = levels.len() > 1 || !levels.contains(&level);
        let layer = egui::LayerId::new(ui.layer_id().order, ui.scope_id().with("map corners"));
        ui.ctx().set_sublayer(ui.layer_id(), layer);
        let inside = rect.shrink(8.0);
        let corner = |align| {
            egui::UiBuilder::new()
                .layer_id(layer)
                .max_rect(inside)
                .layout(egui::Layout::top_down(align))
        };
        if self.browsing.is_some() {
            let mut left = ui.new_child(corner(egui::Align::Min));
            if crate::widgets::button(&mut left, tr("Back to my position")).clicked() {
                (area, level) = home.clone();
            }
        }
        if several_levels {
            // A narrow column at the right edge, the floors centred in it.
            let column =
                Rect::from_min_max(egui::pos2(inside.right() - 32.0, inside.top()), inside.max);
            let mut right = ui.new_child(corner(egui::Align::Max).max_rect(column));
            let said = egui::Frame::new()
                .fill(theme::current().raised)
                .corner_radius(egui::CornerRadius::same(8))
                .inner_margin(egui::Margin::same(3))
                .show(&mut right, |ui| level_steps(ui, &levels, &mut level));
            if let Some(said) = said.inner {
                self.level_said = Some(said);
            }
        }
        let shown = (area, level);
        if self.browsing.as_ref().unwrap_or(&home) != &shown {
            self.pan = Vec2::ZERO;
        }
        self.browsing = (shown != home).then(|| shown.clone());
        shown
    }

    /// The scroll wheel zooms over the map, and dragging pans it, but for a
    /// room being moved.
    pub(super) fn zoom_and_pan(&mut self, ui: &Ui, response: &Response) {
        if response.hovered() {
            let scroll = ui.input(|input| input.smooth_scroll_delta.y);
            let zoomed = self.cell * (1.0 + scroll * 0.002);
            self.cell = zoomed.clamp(*CELL_RANGE.start(), *CELL_RANGE.end());
        }
        if self.moving.is_none() {
            self.pan += response.drag_delta();
        }
    }

    /// Shift+drag picks a room up and shows where it would go; releasing
    /// drops it on the nearest cell. Returns the move, once dropped.
    pub(super) fn move_room<'a>(
        &mut self,
        ui: &Ui,
        response: &Response,
        painter: &Painter,
        grid: &Grid,
        room_at: &dyn Fn(Pos2) -> Option<&'a Room>,
        colours: &Colours,
    ) -> Option<MapAsk> {
        let pointer = response.interact_pointer_pos();
        if response.drag_started() && ui.input(|input| input.modifiers.shift) {
            let pressed_at = ui.input(|input| input.pointer.press_origin());
            self.moving = pressed_at.and_then(room_at).map(|room| room.id);
        }
        if let (Some(_), Some(point)) = (self.moving, pointer) {
            let ghost = Rect::from_center_size(
                grid.on_screen(grid.cell_under(point)),
                Vec2::splat(grid.half() * 2.0),
            );
            let stroke = Stroke::new(1.5, colours.moving);
            painter.rect_stroke(ghost, grid.corner(), stroke, StrokeKind::Outside);
        }
        if response.drag_stopped()
            && let Some(room) = self.moving.take()
        {
            return pointer.map(|point| MapAsk::Move(room, grid.cell_under(point)));
        }
        None
    }

    /// The items of a room's menu that do something with the room: go
    /// there, say the way, what the map knows, mark, note, keep routes
    /// off, and change its look.
    fn room_items(&mut self, ui: &mut Ui, map: &Map, room: &Room, chosen: &mut Option<MapAsk>) {
        widgets::menu_heading(ui, &numbered(map, room));
        // The heading is not reached by the keyboard: the first
        // item says which room the menu is of.
        let go = widgets::menu_item(ui, tr("Go here"), "Enter");
        let of = numbered(map, room);
        (ui.ctx()).accesskit_node_builder(go.id, |node| node.set_description(of));
        if go.clicked() {
            *chosen = Some(MapAsk::Walk(room.id));
            ui.close();
        }
        let mut item = |ui: &mut Ui, label: &str, keys: &str, action: MapAsk| {
            if widgets::menu_item(ui, label, keys).clicked() {
                *chosen = Some(action);
                ui.close();
            }
        };
        item(ui, tr("The way there"), "Shift+Enter", MapAsk::Way(room.id));
        let info = crate::shortcuts::ROOM_INFO.written();
        item(
            ui,
            tr("What the map knows of it"),
            info,
            MapAsk::Info(room.id),
        );
        let mut asked = None;
        if widgets::menu_item(ui, tr("Mark…"), crate::shortcuts::MARK.written()).clicked() {
            asked = Some(explore::Asked::Mark);
        }
        if widgets::menu_item(ui, tr("Note…"), crate::shortcuts::NOTE.written()).clicked() {
            asked = Some(explore::Asked::Note);
        }
        if let Some(asked) = asked {
            self.ask(map, asked, room.id);
            ui.close();
        }
        let mut avoid = room.avoid;
        if widgets::menu_check(ui, &mut avoid, tr("Keep routes off")).clicked() {
            *chosen = Some(MapAsk::Avoid(room.id, avoid));
            ui.close();
        }
        if widgets::menu_item(ui, tr("Appearance…"), "").clicked() {
            *chosen = Some(MapAsk::Look(room.id));
            ui.close();
        }
    }

    /// The items of a room's menu that choose its exits: each exit to a
    /// room the map has, with a separator before them if there are any.
    fn exit_items(&mut self, ui: &mut Ui, map: &Map, room: &Room, chosen: &mut Option<MapAsk>) {
        // Each exit to a room the map has, to follow with the
        // keyboard as the arrows follow the compass: in, out, and
        // those of a name of their own.
        let exits: Vec<(String, RoomId)> = (ordered_exits(room).into_iter())
            .filter_map(|way| {
                let to = map.room(room.exits[way].to)?;
                let said = trf(
                    "Through {0}, to {1}",
                    &[&direction_words(map, way), &place_words(map, to)],
                );
                Some((said, to.id))
            })
            .collect();
        if !exits.is_empty() {
            widgets::menu_separator(ui);
        }
        for (said, to) in exits {
            if widgets::menu_item(ui, &said, "").clicked() {
                self.choose(to);
                *chosen = Some(MapAsk::Selected(to, Some(room.id)));
                ui.close();
            }
        }
    }

    /// The menu of a room, right-clicked or, while exploring, opened with
    /// Shift+F10 or the menu key: all that is done with a room, the same
    /// either way, then its area. Returns what was chosen.
    pub(super) fn room_menu(
        &mut self,
        response: &Response,
        map: &Map,
        area: &str,
    ) -> Option<MapAsk> {
        let mut chosen = None;
        crate::widgets::context_menu_of(response, |ui| {
            widgets::menu_width(ui);
            let room = self.menu_room.and_then(|id| map.room(id));
            if let Some(room) = room {
                self.room_items(ui, map, room, &mut chosen);
                self.exit_items(ui, map, room, &mut chosen);
                widgets::menu_separator(ui);
            }
            let tidied = room.map_or_else(|| area.to_owned(), |room| room.area.clone());
            if widgets::menu_item(ui, tr("Tidy this area"), "").clicked() {
                chosen = Some(MapAsk::Tidy(tidied));
                ui.close();
            }
            widgets::menu_check(ui, &mut self.all_areas, tr("Show all areas"));
        });
        chosen
    }
}

impl Tools {
    /// What the tools on the panel's header asked of the map.
    pub(super) fn action(self, area: &str) -> Option<MapAsk> {
        if self.stop {
            Some(MapAsk::Command("/map stop"))
        } else if self.tidy {
            Some(MapAsk::Tidy(area.to_owned()))
        } else if self.locate {
            Some(MapAsk::Command("/map where"))
        } else {
            None
        }
    }
}
