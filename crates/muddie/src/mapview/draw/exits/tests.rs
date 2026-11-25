//! Tests of how exits are drawn, by the colour seen at a point of them.

use super::*;
use eframe::egui::{Context, LayerId, pos2};
use muddie_mapper::RoomInfo;

const CELL: f32 = 80.0;

/// A room by its number, its place in cells and its exits.
type Laid<'a> = (RoomId, (i32, i32), &'a [(&'a str, RoomId)]);

fn map_of(rooms: &[Laid<'_>]) -> Map {
    let mut map = Map::in_memory();
    for (id, (x, y), exits) in rooms {
        map.visit(RoomInfo {
            id: *id,
            name: format!("Room {id}"),
            area: "town".into(),
            exits: (exits.iter())
                .map(|(way, to)| ((*way).to_owned(), *to))
                .collect(),
            pos: Some(Pos { x: *x, y: *y, z: 0 }),
            colour: None,
        })
        .unwrap();
    }
    map
}

/// Two rooms side by side, 1 and 2, with a way there and a way back.
fn two_rooms() -> Map {
    map_of(&[(1, (0, 0), &[("e", 2)]), (2, (1, 0), &[("w", 1)])])
}

/// What is drawn of the exits of the rooms `visible`, taken in that order,
/// with cells of that size.
fn drawn(map: &Map, visible: &[RoomId], cell: f32, colours: &Colours) -> Vec<Shape> {
    let rooms: Vec<&Room> = visible.iter().filter_map(|id| map.room(*id)).collect();
    let grid = Grid {
        centre: Pos2::ZERO,
        origin: (0, 0),
        cell,
        level: 0,
    };
    let mut output = Context::default().run_ui(Default::default(), |ui| {
        let painter = Painter::new(ui.ctx().clone(), LayerId::background(), Rect::EVERYTHING);
        draw_exits(&painter, map, &rooms, &grid, colours, &|_| true);
    });
    output.textures_delta.clear();
    (output.shapes.into_iter())
        .map(|clipped| clipped.shape)
        .collect()
}

/// The colours, the same for the whole of a test: the theme is the whole
/// program's, and tests run at the same time may change it.
fn colours() -> Colours {
    Colours::of(theme::current())
}

/// The colour seen at a point: that of the last shape drawn over it.
fn seen(shapes: &[Shape], point: Pos2) -> Option<Color32> {
    (shapes.iter().rev()).find_map(|shape| match shape {
        Shape::LineSegment { points, stroke } => {
            let [from, to] = *points;
            let along = ((point - from).dot(to - from) / (to - from).length_sq()).clamp(0.0, 1.0);
            let near = from + (to - from) * along;
            (near.distance(point) < 0.5).then_some(stroke.color)
        }
        Shape::Rect(square) if square.rect.contains(point) => Some(square.fill),
        _ => None,
    })
}

/// A point so far of the way from room 1 to room 2, with cells of `cell`
/// points.
fn at(part: f32, cell: f32) -> Pos2 {
    pos2(cell * part, 0.0)
}

#[test]
fn an_exit_blocked_one_way_is_so_only_from_its_room() {
    let colours = colours();
    let mut map = two_rooms();
    map.edit_exit(1, "e", |exit| exit.blocked = true).unwrap();
    for order in [[1, 2], [2, 1]] {
        let shapes = drawn(&map, &order, CELL, &colours);
        assert_eq!(seen(&shapes, at(0.3, CELL)), Some(colours.blocked));
        assert_eq!(seen(&shapes, at(0.7, CELL)), Some(colours.exit));
    }
}

/// The line goes on to a room out of sight, its far half in the colour of
/// the way back.
#[test]
fn an_exit_to_a_room_out_of_sight_is_drawn_whole() {
    let colours = colours();
    let mut map = two_rooms();
    map.edit_exit(2, "w", |exit| exit.blocked = true).unwrap();
    let shapes = drawn(&map, &[1], CELL, &colours);
    assert_eq!(seen(&shapes, at(0.3, CELL)), Some(colours.exit));
    assert_eq!(seen(&shapes, at(0.7, CELL)), Some(colours.blocked));
}

/// Where halves could not be told apart, the block shows.
#[test]
fn far_out_an_exit_blocked_one_way_is_so_whole() {
    let colours = colours();
    let mut map = two_rooms();
    map.edit_exit(1, "e", |exit| exit.blocked = true).unwrap();
    let cell = DETAILED_FROM - 1.0;
    for order in [[1, 2], [2, 1]] {
        let shapes = drawn(&map, &order, cell, &colours);
        for part in [0.2, 0.8] {
            assert_eq!(seen(&shapes, at(part, cell)), Some(colours.blocked));
        }
    }
}

/// An exit with no way back is of one colour, its own.
#[test]
fn a_one_way_exit_is_drawn_in_its_own_colour() {
    let colours = colours();
    let mut map = map_of(&[(1, (0, 0), &[("e", 2)]), (2, (1, 0), &[])]);
    map.edit_exit(1, "e", |exit| exit.blocked = true).unwrap();
    for order in [[1, 2], [2, 1]] {
        let shapes = drawn(&map, &order, CELL, &colours);
        for part in [0.3, 0.7] {
            assert_eq!(seen(&shapes, at(part, CELL)), Some(colours.blocked));
        }
    }
}

/// Of two doors in the same place, one on each way, the one more shut is
/// seen, over the lines.
#[test]
fn of_two_doors_in_a_place_the_more_shut_is_seen() {
    let colours = colours();
    let mut map = two_rooms();
    let door = |map: &mut Map, room, way: &str, door| {
        map.edit_exit(room, way, |exit| exit.door = Some(door))
            .unwrap();
    };
    door(&mut map, 1, "e", Door::Locked);
    door(&mut map, 2, "w", Door::Closed);
    for order in [[1, 2], [2, 1]] {
        let shapes = drawn(&map, &order, CELL, &colours);
        assert_eq!(seen(&shapes, at(0.5, CELL)), Some(colours.blocked));
    }
    door(&mut map, 1, "e", Door::Open);
    for order in [[1, 2], [2, 1]] {
        let shapes = drawn(&map, &order, CELL, &colours);
        assert_eq!(seen(&shapes, at(0.5, CELL)), Some(colours.exit));
    }
}

/// An open door is hollow: no line shows through it.
#[test]
fn a_door_is_drawn_over_the_line_of_the_way_back() {
    let colours = colours();
    let mut map = two_rooms();
    map.edit_exit(1, "e", |exit| exit.door = Some(Door::Open))
        .unwrap();
    for order in [[1, 2], [2, 1]] {
        let shapes = drawn(&map, &order, CELL, &colours);
        assert!(matches!(shapes.last(), Some(Shape::Rect(_))));
    }
}
