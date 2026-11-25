//! Tests of the map's view.

use super::*;
use muddie_mapper::RoomInfo;

/// What is drawn over what, as lines that cross, is always the same.
#[test]
fn the_rooms_of_a_level_are_drawn_in_order() {
    let mut map = Map::in_memory();
    for id in 1..=50 {
        let room = RoomInfo {
            id: RoomId::from(id),
            name: "Room".into(),
            area: "town".into(),
            exits: Default::default(),
            pos: Some(Pos { x: id, y: 0, z: 0 }),
            colour: None,
        };
        map.visit(room).unwrap();
    }
    let mut view = MapView::default();
    let shown: Shown = (map.revision(), "town".to_owned(), 0, false);
    view.keep_places_on_level(&map, shown, &|_| true);
    let (_, places) = view.on_level.unwrap();
    let drawn: Vec<RoomId> = places.iter().map(|(_, id)| *id).collect();
    assert_eq!(drawn, (1..=50).collect::<Vec<RoomId>>());
}
