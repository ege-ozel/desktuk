//! What an open ring shows: pure mapping from the board to slots. No Win32.

use std::path::PathBuf;

use super::model::{Board, Place};
use super::suggest::Suggestion;

/// What a ring is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The built-in Recent circle: Desktop files that are in no circle.
    Recent,
    Circle(usize),
    /// A group inside a circle (the third and last level).
    Group(usize, usize),
}

/// One slot of a ring, before paging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotRef {
    /// "Back to the circle" (first slot of a group view).
    Back,
    Group(usize),
    /// Index into the suggestion list (Recent only).
    Suggestion(usize),
    Item(PathBuf),
}

/// All slots of `target`, in display order. An invalid target yields no slots.
pub fn slot_refs(board: &Board, target: Target, recent: &[PathBuf], suggestions: &[Suggestion]) -> Vec<SlotRef> {
    match target {
        Target::Recent => (0..suggestions.len()).map(SlotRef::Suggestion).chain(recent.iter().cloned().map(SlotRef::Item)).collect(),
        Target::Circle(ci) => match board.circles.get(ci) {
            Some(c) => (0..c.groups.len()).map(SlotRef::Group).chain(c.items.iter().cloned().map(SlotRef::Item)).collect(),
            None => Vec::new(),
        },
        Target::Group(ci, gi) => match board.circles.get(ci).and_then(|c| c.groups.get(gi)) {
            Some(g) => std::iter::once(SlotRef::Back).chain(g.items.iter().cloned().map(SlotRef::Item)).collect(),
            None => Vec::new(),
        },
    }
}

/// Snapshot candidates only when search begins (or references change).
pub fn search_candidates(board: &Board, target: Target, mut refs: Vec<SlotRef>) -> Vec<SlotRef> {
    if let Target::Circle(ci) = target {
        if let Some(c) = board.circles.get(ci) {
            refs.extend(c.groups.iter().flat_map(|g| g.items.iter().cloned().map(SlotRef::Item)));
        }
    }
    refs
}

/// Two stable ranks in O(n), without sorting or cloning non-matching paths.
fn search_key(text: &str) -> String {
    // Tolerate both Turkish I forms without allocating a second normalized string.
    text.chars().map(|c| if matches!(c, '\u{130}' | '\u{131}') { 'i' } else { c }).flat_map(char::to_lowercase).collect()
}

pub fn filter_refs(refs: &[SlotRef], query: &str) -> Vec<SlotRef> {
    let query = search_key(query);
    let mut prefixes = Vec::new();
    let mut substrings = Vec::new();
    for slot in refs {
        let SlotRef::Item(path) = slot else { continue };
        let name = search_key(&path.file_name().unwrap_or(path.as_os_str()).to_string_lossy());
        match name.find(&query) {
            Some(0) => prefixes.push(slot.clone()),
            Some(_) => substrings.push(slot.clone()),
            None => {}
        }
    }
    prefixes.extend(substrings);
    prefixes
}

#[cfg(test)]
fn search_refs(board: &Board, target: Target, recent: &[PathBuf], suggestions: &[Suggestion], query: &str) -> Vec<SlotRef> {
    let refs = slot_refs(board, target, recent, suggestions);
    if query.is_empty() {
        refs
    } else {
        filter_refs(&search_candidates(board, target, refs), query)
    }
}

pub fn title(board: &Board, target: Target) -> String {
    match target {
        Target::Recent => "Recent".to_string(),
        Target::Circle(ci) => board.circles.get(ci).map(|c| c.name.clone()).unwrap_or_default(),
        Target::Group(ci, gi) => match board.circles.get(ci).and_then(|c| Some((c, c.groups.get(gi)?))) {
            Some((c, g)) => format!("{} / {}", c.name, g.name),
            None => String::new(),
        },
    }
}

/// Number shown on a closed circle: its items including those in groups.
pub fn circle_count(board: &Board, ci: usize) -> usize {
    board.circles.get(ci).map(|c| c.items.len() + c.groups.iter().map(|g| g.items.len()).sum::<usize>()).unwrap_or(0)
}

/// Where a file dropped on `target` ends up, given the slot it was dropped on (if any).
/// `None` means "unassign" (back to Recent).
pub fn drop_place(target: Target, on: Option<&SlotRef>) -> Option<Place> {
    match (target, on) {
        (Target::Circle(ci), Some(SlotRef::Group(gi))) => Some(Place::Group(ci, *gi)),
        (Target::Group(ci, _), Some(SlotRef::Back)) => Some(Place::Circle(ci)),
        (Target::Circle(ci), _) => Some(Place::Circle(ci)),
        (Target::Group(ci, gi), _) => Some(Place::Group(ci, gi)),
        (Target::Recent, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn board() -> Board {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let g = b.add_group(m, "Live").unwrap();
        b.assign(&p(r"C:\D\a.wav"), Place::Circle(m));
        b.assign(&p(r"C:\D\b.wav"), Place::Group(m, g));
        b
    }

    #[test]
    fn search_large_lists_keep_rank_order_without_touching_directories() {
        let refs: Vec<_> =
            (0..10_000).map(|i| SlotRef::Item(p(&format!("{}-{i:05}.txt", if i % 2 == 0 { "ab" } else { "x-ab" })))).collect();
        let found = filter_refs(&refs, "AB");
        assert_eq!(found.len(), 10_000);
        assert_eq!(found[0], refs[0]);
        assert_eq!(found[4999], refs[9998]);
        assert_eq!(found[5000], refs[1]);
        assert_eq!(found[9999], refs[9999]);
    }

    #[test]
    fn search_ranks_prefixes_stably_and_includes_groups_without_mutation() {
        let mut b = board();
        b.circles[0].items = vec![p("xxAB.wav"), p("abZ.wav"), p("AbA.wav"), p("other.wav")];
        b.circles[0].groups[0].items = vec![p("group-ab.wav")];
        let before = b.render();
        assert_eq!(
            search_refs(&b, Target::Circle(0), &[], &[], "aB"),
            ["abZ.wav", "AbA.wav", "xxAB.wav", "group-ab.wav"].map(|s| SlotRef::Item(p(s)))
        );
        assert!(search_refs(&b, Target::Circle(0), &[], &[], "missing").is_empty());
        assert_eq!(search_refs(&b, Target::Circle(0), &[], &[], ""), slot_refs(&b, Target::Circle(0), &[], &[]));
        assert_eq!(search_refs(&b, Target::Group(0, 0), &[], &[], "ab"), vec![SlotRef::Item(p("group-ab.wav"))]);
        assert_eq!(search_refs(&b, Target::Recent, &[p("Ödev.txt")], &[], "öD"), vec![SlotRef::Item(p("Ödev.txt"))]);
        assert_eq!(filter_refs(&[SlotRef::Item(p("\u{130}sim.txt"))], "is"), vec![SlotRef::Item(p("\u{130}sim.txt"))]);
        assert_eq!(filter_refs(&[SlotRef::Item(p("I\u{15e}IK.txt"))], "\u{131}\u{15f}"), vec![SlotRef::Item(p("I\u{15e}IK.txt"))]);
        assert_eq!(b.render(), before);
    }

    #[test]
    fn circle_view_lists_groups_then_items() {
        let b = board();
        assert_eq!(slot_refs(&b, Target::Circle(0), &[], &[]), vec![SlotRef::Group(0), SlotRef::Item(p(r"C:\D\a.wav"))]);
        assert_eq!(slot_refs(&b, Target::Group(0, 0), &[], &[]), vec![SlotRef::Back, SlotRef::Item(p(r"C:\D\b.wav"))]);
        assert!(slot_refs(&b, Target::Circle(5), &[], &[]).is_empty());
        assert!(slot_refs(&b, Target::Group(0, 5), &[], &[]).is_empty());
        assert_eq!(circle_count(&b, 0), 2);
        assert_eq!(title(&b, Target::Group(0, 0)), "Music / Live");
        assert_eq!(title(&b, Target::Recent), "Recent");
    }

    #[test]
    fn recent_view_lists_suggestions_first() {
        let b = board();
        let s = vec![Suggestion { ext: "wav".into(), circle: 0, paths: vec![p(r"C:\D\c.wav")] }];
        let refs = slot_refs(&b, Target::Recent, &[p(r"C:\D\c.wav")], &s);
        assert_eq!(refs, vec![SlotRef::Suggestion(0), SlotRef::Item(p(r"C:\D\c.wav"))]);
    }

    #[test]
    fn drops_land_in_the_group_under_the_cursor_or_the_view_itself() {
        assert_eq!(drop_place(Target::Circle(1), Some(&SlotRef::Group(2))), Some(Place::Group(1, 2)));
        assert_eq!(drop_place(Target::Circle(1), Some(&SlotRef::Item(p(r"C:\x")))), Some(Place::Circle(1)));
        assert_eq!(drop_place(Target::Circle(1), None), Some(Place::Circle(1)));
        assert_eq!(drop_place(Target::Group(1, 2), None), Some(Place::Group(1, 2)));
        assert_eq!(drop_place(Target::Group(1, 2), Some(&SlotRef::Back)), Some(Place::Circle(1)));
        assert_eq!(drop_place(Target::Recent, None), None, "dropping on Recent unassigns");
    }
}
