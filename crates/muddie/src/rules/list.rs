//! The rules of one kind seen alike: what the editor does with all of them.

use super::*;

/// The order of `len` things once those at `moving` are taken out and put,
/// in the order they were, before what was at `to`; `len` puts them last.
pub fn moved_order(len: usize, moving: &[usize], to: usize) -> Vec<usize> {
    let mut moving: Vec<usize> = moving
        .iter()
        .copied()
        .filter(|&index| index < len)
        .collect();
    moving.sort_unstable();
    moving.dedup();
    let staying = (0..len).filter(|index| moving.binary_search(index).is_err());
    let before = (0..to.min(len))
        .filter(|index| moving.binary_search(index).is_err())
        .count();
    let mut order: Vec<usize> = staying.collect();
    order.splice(before..before, moving);
    order
}

/// Puts things in a new order: the one at `order[0]` first, and so on.
pub fn reorder<T>(items: &mut Vec<T>, order: &[usize]) {
    let mut taken: Vec<Option<T>> = std::mem::take(items).into_iter().map(Some).collect();
    items.extend(
        order
            .iter()
            .filter_map(|&index| taken.get_mut(index)?.take()),
    );
}

/// What every kind of rule has, to be changed in the editor.
pub struct Head<'a> {
    pub name: &'a mut String,
    pub enabled: &'a mut bool,
    pub folder: &'a mut String,
    pub tags: &'a mut Vec<String>,
}

/// The rules of one kind, whatever it is: what the editor does with all
/// of them alike.
pub trait RuleList {
    fn len(&self) -> usize;
    fn title(&self, index: usize) -> String;
    fn place(&self, index: usize) -> String;
    fn enabled(&self, index: usize) -> bool;
    fn folder(&self, index: usize) -> &str;
    fn set_folder(&mut self, index: usize, folder: String);
    fn tags(&self, index: usize) -> &[String];
    /// What every kind has, to be changed: its name, its switch, its
    /// folder and its tags.
    fn head_mut(&mut self, index: usize) -> Head<'_>;
    /// Adds a new one, in a folder; returns where it is.
    fn add(&mut self, folder: &str) -> usize;
    fn remove(&mut self, index: usize);
    /// Puts them in a new order: the one at `order[0]` first, and so on.
    fn reorder(&mut self, order: &[usize]);
}

/// One rule, whatever its kind: what [`RuleList`] needs of each.
pub trait Ruled {
    /// What it is called: its name, or else what it is made of.
    fn title(&self) -> String;
    fn head(&mut self) -> Head<'_>;
    fn is_on(&self) -> bool;
    fn folder(&self) -> &str;
    fn tags(&self) -> &[String];
    /// A new one, in a folder.
    fn new_in(folder: &str) -> Self;
}

impl<T: When> Ruled for Rule<T> {
    fn title(&self) -> String {
        Rule::title(self)
    }
    fn head(&mut self) -> Head<'_> {
        Head {
            name: &mut self.name,
            enabled: &mut self.enabled,
            folder: &mut self.folder,
            tags: &mut self.tags,
        }
    }
    fn is_on(&self) -> bool {
        self.enabled
    }
    fn folder(&self) -> &str {
        &self.folder
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
    fn new_in(folder: &str) -> Self {
        Rule {
            folder: folder.to_owned(),
            ..Rule::default()
        }
    }
}

impl Ruled for Gauge {
    fn title(&self) -> String {
        self.shown_name().to_owned()
    }
    fn head(&mut self) -> Head<'_> {
        Head {
            name: &mut self.name,
            enabled: &mut self.enabled,
            folder: &mut self.folder,
            tags: &mut self.tags,
        }
    }
    fn is_on(&self) -> bool {
        self.enabled
    }
    fn folder(&self) -> &str {
        &self.folder
    }
    fn tags(&self) -> &[String] {
        &self.tags
    }
    fn new_in(folder: &str) -> Self {
        Gauge {
            folder: folder.to_owned(),
            ..Gauge::starter()
        }
    }
}

impl<R: Ruled> RuleList for Vec<R> {
    fn len(&self) -> usize {
        <[R]>::len(self)
    }
    fn title(&self, index: usize) -> String {
        self[index].title()
    }
    /// Where it is among those of its kind, as `Folder/Title`, which is
    /// what it is switched by: one called as one before it in the same
    /// folder is told apart by a number, `Title #2`.
    fn place(&self, index: usize) -> String {
        let rule = &self[index];
        let (folder, title) = (Ruled::folder(rule).trim(), rule.title());
        let alike = (self[..index].iter())
            .filter(|other| Ruled::folder(*other).trim() == folder && other.title() == title)
            .count();
        match alike {
            0 => joined(folder, &title),
            n => joined(folder, &format!("{title} #{}", n + 1)),
        }
    }
    fn enabled(&self, index: usize) -> bool {
        self[index].is_on()
    }
    fn folder(&self, index: usize) -> &str {
        Ruled::folder(&self[index])
    }
    fn set_folder(&mut self, index: usize, folder: String) {
        *self[index].head().folder = folder;
    }
    fn tags(&self, index: usize) -> &[String] {
        Ruled::tags(&self[index])
    }
    fn head_mut(&mut self, index: usize) -> Head<'_> {
        self[index].head()
    }
    fn add(&mut self, folder: &str) -> usize {
        self.push(R::new_in(folder));
        <[R]>::len(self) - 1
    }
    fn remove(&mut self, index: usize) {
        Vec::remove(self, index);
    }
    fn reorder(&mut self, order: &[usize]) {
        reorder(self, order);
    }
}
