//! The circle board: pure data and operations, no Win32.
//!
//! A board is a list of user circles. A circle holds item paths directly and in groups
//! (at most one level of groups: circle > group > item). Items are *references*: nothing
//! here touches the file system. Every path lives in at most one place.

use std::path::{Path, PathBuf};

use crate::watch::{self, Change};

/// Longest circle/group name, in characters.
pub const MAX_NAME: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Group {
    pub name: String,
    pub items: Vec<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circle {
    pub name: String,
    /// Top-left of the circle window in screen pixels.
    pub pos: (i32, i32),
    pub items: Vec<PathBuf>,
    pub groups: Vec<Group>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Board {
    pub circles: Vec<Circle>,
    /// Top-left of the built-in Recent circle; `None` = default position.
    pub recent_pos: Option<(i32, i32)>,
}

/// Where an item lives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Circle(usize),
    Group(usize, usize),
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.as_os_str().eq_ignore_ascii_case(b.as_os_str())
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// A name that is safe to store: no `|` (the config separator), no control characters,
/// trimmed, at most `MAX_NAME` characters, never empty.
pub fn clean_name(s: &str) -> String {
    let t: String = s.chars().map(|c| if c == '|' { '/' } else { c }).filter(|c| !c.is_control()).collect();
    let t: String = t.trim().chars().take(MAX_NAME).collect();
    let t = t.trim().to_string();
    if t.is_empty() {
        "Untitled".to_string()
    } else {
        t
    }
}

/// `wanted` made unique against `taken` (case-insensitive): "Music", "Music 2", "Music 3", ...
fn unique_among(wanted: &str, taken: &[&str]) -> String {
    let base = clean_name(wanted);
    if !taken.iter().any(|t| same_name(t, &base)) {
        return base;
    }
    for n in 2.. {
        let suffix = format!(" {n}");
        let keep = MAX_NAME.saturating_sub(suffix.chars().count());
        let candidate = format!("{}{suffix}", base.chars().take(keep).collect::<String>().trim_end());
        if !taken.iter().any(|t| same_name(t, &candidate)) {
            return candidate;
        }
    }
    unreachable!()
}

fn remove_path(v: &mut Vec<PathBuf>, path: &Path) -> bool {
    let before = v.len();
    v.retain(|p| !same_path(p, path));
    v.len() != before
}

impl Board {
    pub fn find_circle(&self, name: &str) -> Option<usize> {
        self.circles.iter().position(|c| same_name(&c.name, name))
    }

    /// `wanted` made unique among circle names, ignoring circle `except` (the one being renamed).
    pub fn unique_circle_name(&self, wanted: &str, except: Option<usize>) -> String {
        let taken: Vec<&str> = self.circles.iter().enumerate().filter(|(i, _)| Some(*i) != except).map(|(_, c)| c.name.as_str()).collect();
        unique_among(wanted, &taken)
    }

    /// Appends a circle and returns its index. The name is cleaned and made unique.
    pub fn add_circle(&mut self, name: &str, pos: (i32, i32)) -> usize {
        let name = self.unique_circle_name(name, None);
        self.circles.push(Circle { name, pos, items: Vec::new(), groups: Vec::new() });
        self.circles.len() - 1
    }

    pub fn rename_circle(&mut self, ci: usize, name: &str) {
        if ci < self.circles.len() {
            self.circles[ci].name = self.unique_circle_name(name, Some(ci));
        }
    }

    /// Removes a circle. Its items become unassigned (they show up in Recent again).
    pub fn remove_circle(&mut self, ci: usize) {
        if ci < self.circles.len() {
            self.circles.remove(ci);
        }
    }

    /// Appends a group to circle `ci` and returns its index (`None` if `ci` is invalid).
    pub fn add_group(&mut self, ci: usize, name: &str) -> Option<usize> {
        let c = self.circles.get_mut(ci)?;
        let taken: Vec<&str> = c.groups.iter().map(|g| g.name.as_str()).collect();
        let name = unique_among(name, &taken);
        c.groups.push(Group { name, items: Vec::new() });
        Some(c.groups.len() - 1)
    }

    pub fn rename_group(&mut self, ci: usize, gi: usize, name: &str) {
        let Some(c) = self.circles.get_mut(ci) else { return };
        if gi >= c.groups.len() {
            return;
        }
        let taken: Vec<&str> = c.groups.iter().enumerate().filter(|(i, _)| *i != gi).map(|(_, g)| g.name.as_str()).collect();
        c.groups[gi].name = unique_among(name, &taken);
    }

    /// Removes a group; its items move up to the circle itself.
    pub fn remove_group(&mut self, ci: usize, gi: usize) {
        let Some(c) = self.circles.get_mut(ci) else { return };
        if gi < c.groups.len() {
            let g = c.groups.remove(gi);
            c.items.extend(g.items);
        }
    }

    pub fn place_of(&self, path: &Path) -> Option<Place> {
        for (ci, c) in self.circles.iter().enumerate() {
            if c.items.iter().any(|p| same_path(p, path)) {
                return Some(Place::Circle(ci));
            }
            for (gi, g) in c.groups.iter().enumerate() {
                if g.items.iter().any(|p| same_path(p, path)) {
                    return Some(Place::Group(ci, gi));
                }
            }
        }
        None
    }

    pub fn is_assigned(&self, path: &Path) -> bool {
        self.place_of(path).is_some()
    }

    /// Removes `path` from wherever it is. Returns whether it was assigned.
    pub fn unassign(&mut self, path: &Path) -> bool {
        let mut any = false;
        for c in &mut self.circles {
            any |= remove_path(&mut c.items, path);
            for g in &mut c.groups {
                any |= remove_path(&mut g.items, path);
            }
        }
        any
    }

    /// Puts `path` at `to`, removing it from its previous place first. Returns `false`
    /// (and changes nothing) when `to` does not exist.
    pub fn assign(&mut self, path: &Path, to: Place) -> bool {
        let valid = match to {
            Place::Circle(ci) => ci < self.circles.len(),
            Place::Group(ci, gi) => self.circles.get(ci).map(|c| gi < c.groups.len()).unwrap_or(false),
        };
        if !valid {
            return false;
        }
        self.unassign(path);
        match to {
            Place::Circle(ci) => self.circles[ci].items.push(path.to_path_buf()),
            Place::Group(ci, gi) => self.circles[ci].groups[gi].items.push(path.to_path_buf()),
        }
        true
    }

    /// Follows a rename/move reported by the shell. Returns whether any stored path changed.
    pub fn follow(&mut self, change: &Change) -> bool {
        if !matches!(change, Change::Renamed { .. }) {
            return false;
        }
        let mut changed = false;
        for c in &mut self.circles {
            changed |= !watch::apply_change(&mut c.items, change).is_empty();
            for g in &mut c.groups {
                changed |= !watch::apply_change(&mut g.items, change).is_empty();
            }
        }
        changed
    }

    /// Applies one `key=value` line of the config file. Returns `false` for keys that
    /// are not board keys (the caller handles those).
    ///
    /// ```text
    /// circle=Music|120|340
    /// group=Music|Titled Project
    /// citem=Music||C:\Users\me\Desktop\notes.txt
    /// citem=Music|Titled Project|C:\Users\me\Desktop\song.wav
    /// recent=1600|80
    /// ```
    pub fn parse_line(&mut self, key: &str, value: &str) -> bool {
        match key {
            "circle" => {
                let mut f = value.splitn(3, '|');
                let name = clean_name(f.next().unwrap_or(""));
                let x = f.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                let y = f.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                match self.find_circle(&name) {
                    Some(ci) => self.circles[ci].pos = (x, y),
                    None => self.circles.push(Circle { name, pos: (x, y), items: Vec::new(), groups: Vec::new() }),
                }
            }
            "group" => {
                let Some((c, g)) = value.split_once('|') else { return true };
                let ci = self.circle_or_create(c);
                let g = clean_name(g);
                if !self.circles[ci].groups.iter().any(|x| same_name(&x.name, &g)) {
                    self.circles[ci].groups.push(Group { name: g, items: Vec::new() });
                }
            }
            "citem" => {
                let mut f = value.splitn(3, '|');
                let (Some(c), Some(g), Some(p)) = (f.next(), f.next(), f.next()) else { return true };
                let p = p.trim();
                if p.is_empty() {
                    return true;
                }
                let ci = self.circle_or_create(c);
                let to = if g.trim().is_empty() {
                    Place::Circle(ci)
                } else {
                    let g = clean_name(g);
                    let gi = match self.circles[ci].groups.iter().position(|x| same_name(&x.name, &g)) {
                        Some(gi) => gi,
                        None => {
                            self.circles[ci].groups.push(Group { name: g, items: Vec::new() });
                            self.circles[ci].groups.len() - 1
                        }
                    };
                    Place::Group(ci, gi)
                };
                self.assign(Path::new(p), to);
            }
            "recent" => {
                let parsed = value.split_once('|').and_then(|(x, y)| Some((x.trim().parse().ok()?, y.trim().parse().ok()?)));
                self.recent_pos = parsed;
            }
            _ => return false,
        }
        true
    }

    fn circle_or_create(&mut self, name: &str) -> usize {
        let name = clean_name(name);
        match self.find_circle(&name) {
            Some(ci) => ci,
            None => {
                self.circles.push(Circle { name, pos: (0, 0), items: Vec::new(), groups: Vec::new() });
                self.circles.len() - 1
            }
        }
    }

    /// The board as config lines (each ends with `\n`), in an order `parse_line` reads back.
    pub fn render(&self) -> String {
        let mut s = String::new();
        if let Some((x, y)) = self.recent_pos {
            s += &format!("recent={x}|{y}\n");
        }
        for c in &self.circles {
            s += &format!("circle={}|{}|{}\n", c.name, c.pos.0, c.pos.1);
            for g in &c.groups {
                s += &format!("group={}|{}\n", c.name, g.name);
            }
            for p in &c.items {
                s += &format!("citem={}||{}\n", c.name, p.display());
            }
            for g in &c.groups {
                for p in &g.items {
                    s += &format!("citem={}|{}|{}\n", c.name, g.name, p.display());
                }
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn parse(text: &str) -> Board {
        let mut b = Board::default();
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                b.parse_line(k.trim(), v.trim());
            }
        }
        b
    }

    #[test]
    fn names_are_cleaned_and_made_unique() {
        assert_eq!(clean_name("  Mu|sic \n"), "Mu/sic");
        assert_eq!(clean_name("   "), "Untitled");
        assert_eq!(clean_name(&"x".repeat(100)).chars().count(), MAX_NAME);
        let mut b = Board::default();
        assert_eq!(b.add_circle("Music", (0, 0)), 0);
        b.add_circle("music", (0, 0));
        b.add_circle("MUSIC", (0, 0));
        let names: Vec<&str> = b.circles.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Music", "music 2", "MUSIC 3"]);
        b.rename_circle(0, "Music");
        assert_eq!(b.circles[0].name, "Music", "renaming to its own name keeps it");
        b.rename_circle(2, "music 2");
        assert_eq!(b.circles[2].name, "music 2 2");
    }

    #[test]
    fn an_item_lives_in_exactly_one_place() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let k = b.add_circle("Books", (0, 0));
        let g = b.add_group(m, "Titled").unwrap();
        let f = p(r"C:\Users\x\Desktop\song.wav");
        assert!(b.assign(&f, Place::Circle(k)));
        assert!(b.assign(&p(r"c:\users\x\desktop\SONG.wav"), Place::Group(m, g)));
        assert_eq!(b.place_of(&f), Some(Place::Group(m, g)));
        assert!(b.circles[k].items.is_empty(), "moved, not copied");
        assert!(!b.assign(&f, Place::Group(m, 7)), "invalid target refused");
        assert_eq!(b.place_of(&f), Some(Place::Group(m, g)), "...and nothing changed");
        assert!(b.unassign(&f));
        assert!(!b.is_assigned(&f));
        assert!(!b.unassign(&f));
    }

    #[test]
    fn removing_a_group_keeps_its_items_in_the_circle() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let g = b.add_group(m, "Titled").unwrap();
        b.assign(&p(r"C:\a.wav"), Place::Group(m, g));
        b.remove_group(m, g);
        assert_eq!(b.place_of(&p(r"C:\a.wav")), Some(Place::Circle(m)));
        b.remove_circle(m);
        assert!(!b.is_assigned(&p(r"C:\a.wav")), "removing a circle unassigns, never deletes files");
    }

    #[test]
    fn group_names_are_unique_per_circle_only() {
        let mut b = Board::default();
        let a = b.add_circle("A", (0, 0));
        let c = b.add_circle("C", (0, 0));
        assert_eq!(b.add_group(a, "Old"), Some(0));
        assert_eq!(b.add_group(a, "old"), Some(1));
        assert_eq!(b.circles[a].groups[1].name, "old 2");
        b.add_group(c, "Old");
        assert_eq!(b.circles[c].groups[0].name, "Old");
        assert_eq!(b.add_group(9, "x"), None);
    }

    #[test]
    fn follows_renames_of_files_and_folders() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        b.assign(&p(r"C:\D\old.wav"), Place::Circle(m));
        assert!(b.follow(&Change::Renamed { from: p(r"c:\d\OLD.wav"), to: p(r"C:\D\new.wav") }));
        assert_eq!(b.place_of(&p(r"C:\D\new.wav")), Some(Place::Circle(m)));
        assert!(!b.follow(&Change::Removed(p(r"C:\D\new.wav"))), "a deleted file stays referenced");
        assert!(b.is_assigned(&p(r"C:\D\new.wav")));
    }

    #[test]
    fn config_lines_round_trip() {
        let mut b = Board::default();
        let m = b.add_circle("Music", (120, -40));
        let g = b.add_group(m, "Titled Project").unwrap();
        b.add_group(m, "Empty");
        b.assign(&p(r"C:\Users\x\Desktop\notes.txt"), Place::Circle(m));
        b.assign(&p(r"C:\Users\x\Desktop\a b\song.wav"), Place::Group(m, g));
        b.add_circle("Books", (5, 6));
        b.recent_pos = Some((1600, 80));
        let text = b.render();
        assert!(text.contains("circle=Music|120|-40\n"));
        assert!(text.contains("citem=Music||C:\\Users\\x\\Desktop\\notes.txt\n"));
        assert_eq!(parse(&text), b);
    }

    #[test]
    fn parsing_tolerates_hand_edited_files() {
        let b = parse("citem=Music|Live|C:\\x.wav\ncircle=Music|1|2\nbogus=1\ncitem=Music||\ncircle=|3|4\nrecent=garbage\n");
        assert_eq!(b.circles.len(), 2);
        assert_eq!(b.circles[0].name, "Music");
        assert_eq!(b.circles[0].pos, (1, 2), "a later circle= line sets the position");
        assert_eq!(b.circles[0].groups[0].name, "Live");
        assert_eq!(b.place_of(&p(r"C:\x.wav")), Some(Place::Group(0, 0)));
        assert_eq!(b.circles[1].name, "Untitled");
        assert_eq!(b.recent_pos, None);
        let mut b2 = Board::default();
        assert!(!b2.parse_line("item", r"C:\x"), "legacy keys are left to the caller");
    }
}
