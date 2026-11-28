//! Where the gauges of a panel go: in the order they have, in rows as full
//! as they fit, worked out before anything is drawn, in whole pixels, so
//! that the same width always puts them the same way.

/// What a gauge needs: as narrow as it may be, as wide as it may grow, and
/// how tall it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Want {
    pub min: f32,
    pub max: f32,
    pub height: f32,
    /// Whether it is a bar: the bars of a row after the first are no wider
    /// than those of the first, so that they line up.
    pub bar: bool,
}

/// Where a gauge goes, from the panel's top left corner, and how wide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Placed {
    /// The row it is in, from 0.
    pub row: usize,
    pub x: f32,
    pub y: f32,
    pub width: f32,
}

/// Puts `wants` in rows `room` wide, `gap` apart side by side and `row_gap`
/// one row under another: each where it fits at its narrowest after the one
/// before, else at the start of the next row; then what is left of each
/// row shared among what may grow in it. Returns where each goes, and how
/// tall they are together.
pub(super) fn arrange(room: f32, gap: f32, row_gap: f32, wants: &[Want]) -> (Vec<Placed>, f32) {
    let room = room.floor().max(1.0) as i32;
    let gap = gap.round() as i32;
    let mut placed = vec![
        Placed {
            row: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0
        };
        wants.len()
    ];
    let (mut y, mut start, mut number) = (0.0, 0, 0);
    // The width of the bars of the first row that has any: those after it
    // are as wide, so that they line up as in a grid.
    let mut bars: Option<i32> = None;
    while start < wants.len() {
        let row = next_row(room, gap, wants, start, bars);
        start += row.len();
        let widths = share(room, gap, &row, wants, bars);
        if bars.is_none() {
            bars = (row.iter().zip(&widths))
                .filter(|(index, _)| wants[**index].bar)
                .map(|(_, &width)| width)
                // The narrowest of them, which a row like it fits.
                .min();
        }
        let tallest = (row.iter())
            .map(|&index| wants[index].height)
            .fold(0.0, f32::max);
        let mut x = 0;
        for (&index, &width) in row.iter().zip(&widths) {
            // Each in the middle of the height of its row.
            let lower = ((tallest - wants[index].height) / 2.0).floor();
            placed[index] = Placed {
                row: number,
                x: x as f32,
                y: y + lower,
                width: width as f32,
            };
            x += width + gap;
        }
        y += tallest + row_gap;
        number += 1;
    }
    let height = (y - row_gap).max(0.0);
    (placed, height)
}

/// The gauges of the row that begins at `start`: each after the one before
/// where it fits at its narrowest, the first whatever it needs.
fn next_row(room: i32, gap: i32, wants: &[Want], start: usize, bars: Option<i32>) -> Vec<usize> {
    let mut row = vec![start];
    let mut used = narrowest(&wants[start], room, bars);
    for (index, want) in wants.iter().enumerate().skip(start + 1) {
        let min = narrowest(want, room, bars);
        if used + gap + min > room {
            break;
        }
        row.push(index);
        used += gap + min;
    }
    row
}

/// The narrowest a gauge may be, no wider than the panel: a bar, once the
/// width of bars is known, as wide as the others.
fn narrowest(want: &Want, room: i32, bars: Option<i32>) -> i32 {
    match (want.bar, bars) {
        (true, Some(bars)) => bars.clamp(1, room),
        _ => (want.min.ceil() as i32).clamp(1, room),
    }
}

/// How wide each gauge of a row is: its narrowest, and of what is left, as
/// much as each may grow, in turns, the same for each, until none is left
/// or none may grow more. Bars, once their width is known, do not grow.
fn share(room: i32, gap: i32, row: &[usize], wants: &[Want], bars: Option<i32>) -> Vec<i32> {
    let widest = |want: &Want| match (want.bar, bars) {
        (true, Some(_)) => narrowest(want, room, bars),
        _ => (want.max.floor() as i32).clamp(narrowest(want, room, bars), room),
    };
    let mut widths: Vec<i32> = (row.iter())
        .map(|&index| narrowest(&wants[index], room, bars))
        .collect();
    let mut left = room - widths.iter().sum::<i32>() - gap * (row.len() as i32 - 1);
    loop {
        let growing: Vec<usize> = (0..row.len())
            .filter(|&at| widths[at] < widest(&wants[row[at]]))
            .collect();
        if left <= 0 || growing.is_empty() {
            break;
        }
        // The same for each, and a pixel more for the first few when it
        // does not share out evenly.
        let each = left / growing.len() as i32;
        let more = left % growing.len() as i32;
        let mut given = 0;
        for (turn, &at) in growing.iter().enumerate() {
            let wanted = each + i32::from((turn as i32) < more);
            let grown = wanted.min(widest(&wants[row[at]]) - widths[at]);
            widths[at] += grown;
            given += grown;
        }
        left -= given;
        if given == 0 {
            break;
        }
    }
    widths
}
