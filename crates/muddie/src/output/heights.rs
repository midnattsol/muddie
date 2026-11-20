//! Where each row of the output ends, added up as they come, so that
//! finding the rows on screen does not go through every row each frame:
//! only those after the first whose height changed are added up again.

use std::collections::VecDeque;

use super::rows::Row;

#[derive(Default)]
pub(super) struct Ends {
    /// Where each row ends, counted from where the first one ever kept
    /// began; as many as are worked out.
    ends: VecDeque<f64>,
    /// Where the first row kept begins, on that count.
    base: f64,
    /// How many of `ends` are right.
    valid: usize,
}

impl Ends {
    /// The rows from `index` on are to be added up again.
    pub(super) fn changed_from(&mut self, index: usize) {
        self.valid = self.valid.min(index);
    }

    /// The first row was dropped: the others begin where it ended.
    pub(super) fn dropped_first(&mut self) {
        if self.valid > 0
            && let Some(end) = self.ends.pop_front()
        {
            self.base = end;
        }
        self.valid = self.valid.saturating_sub(1);
    }

    /// Forgets every row.
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Adds up again the rows that changed, those not yet measured by a
    /// guess.
    pub(super) fn update(&mut self, rows: &VecDeque<Row>, guess: &dyn Fn(&Row) -> f32) {
        self.ends.truncate(self.valid);
        let mut end = self.ends.back().copied().unwrap_or(self.base);
        for row in rows.iter().skip(self.valid) {
            end += f64::from(row.height.unwrap_or_else(|| guess(row)));
            self.ends.push_back(end);
        }
        self.valid = rows.len();
    }

    /// Where a row begins, from the top of the first.
    pub(super) fn top(&self, index: usize) -> f32 {
        let begins = match index {
            0 => self.base,
            index => self.ends[index - 1],
        };
        (begins - self.base) as f32
    }

    /// How tall the rows are together.
    pub(super) fn total(&self) -> f32 {
        (self.ends.back().copied().unwrap_or(self.base) - self.base) as f32
    }

    /// The first row that reaches below `y`; as many as there are if none
    /// does.
    pub(super) fn first_below(&self, y: f32) -> usize {
        let y = f64::from(y) + self.base;
        self.ends.partition_point(|end| *end <= y)
    }
}
