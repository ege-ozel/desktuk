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

/// Search names only; stable ranks keep the owner's current sort/manual order.
/// Circle searches also include references inside its groups.
pub fn search_refs(board: &Board, target: Target, recent: &[PathBuf], suggestions: &[Suggestion], query: &str) -> Vec<SlotRef> {
    if query.is_empty() {
        return slot_refs(board, target, recent, suggestions);
    }
    let mut refs = slot_refs(board, target, recent, &[]);
    if let Target::Circle(ci) = target {
        if let Some(c) = board.circles.get(ci) {
            refs.extend(c.groups.iter().flat_map(|g| g.items.iter().cloned().map(SlotRef::Item)));
        }
    }
    let query = query.to_lowercase();
    let mut matches: Vec<_> = refs
        .into_iter()
        .filter_map(|slot| {
            let SlotRef::Item(path) = &slot else { return None };
            let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().to_lowercase();
            name.find(&query).map(|offset| (usize::from(offset != 0), slot))
        })
        .collect();
    matches.sort_by_key(|(rank, _)| *rank);
    matches.into_iter().map(|(_, slot)| slot).collect()
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
