//! Rule suggestions: "these 12 .wav files on the Desktop probably belong in Music".
//!
//! Pure. A suggestion is only ever *offered*; nothing is assigned until the owner clicks it.

use std::path::{Path, PathBuf};

use super::model::Board;

/// Most suggestions shown at once.
pub const MAX_SUGGESTIONS: usize = 3;
/// A circle must already hold at least this many files of an extension.
pub const MIN_EXAMPLES: usize = 2;
/// Extensions that say nothing about what a file is about.
const GENERIC: [&str; 3] = ["lnk", "url", "tmp"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// Lower-case extension without the dot.
    pub ext: String,
    pub circle: usize,
    pub paths: Vec<PathBuf>,
}

pub fn ext_of(p: &Path) -> Option<String> {
    let e = p.extension()?.to_string_lossy().to_lowercase();
    if e.is_empty() || GENERIC.contains(&e.as_str()) {
        None
    } else {
        Some(e)
    }
}

/// Suggestions for `unassigned` paths. An extension points at circle `c` when `c` holds at
/// least `MIN_EXAMPLES` files with it and strictly more than any other circle. Pairs in
/// `dismissed` (extension, circle) are not offered again.
pub fn suggest(board: &Board, unassigned: &[PathBuf], dismissed: &[(String, usize)]) -> Vec<Suggestion> {
    use std::collections::BTreeMap;
    // ext -> per-circle counts
    let mut counts: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (ci, c) in board.circles.iter().enumerate() {
        let all = c.items.iter().chain(c.groups.iter().flat_map(|g| g.items.iter()));
        for p in all {
            if let Some(e) = ext_of(p) {
                counts.entry(e).or_insert_with(|| vec![0; board.circles.len()])[ci] += 1;
            }
        }
    }
    let mut out: Vec<Suggestion> = Vec::new();
    for (ext, per) in counts {
        let Some((best, &n)) = per.iter().enumerate().max_by_key(|(_, &n)| n) else { continue };
        let unique_best = per.iter().filter(|&&m| m == n).count() == 1;
        if n < MIN_EXAMPLES || !unique_best || dismissed.iter().any(|(e, c)| *e == ext && *c == best) {
            continue;
        }
        let paths: Vec<PathBuf> = unassigned.iter().filter(|p| ext_of(p).as_deref() == Some(ext.as_str())).cloned().collect();
        if !paths.is_empty() {
            out.push(Suggestion { ext, circle: best, paths });
        }
    }
    out.sort_by(|a, b| b.paths.len().cmp(&a.paths.len()).then_with(|| a.ext.cmp(&b.ext)));
    out.truncate(MAX_SUGGESTIONS);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circles::model::Place;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn board() -> Board {
        let mut b = Board::default();
        let m = b.add_circle("Music", (0, 0));
        let k = b.add_circle("Books", (0, 0));
        let g = b.add_group(m, "Live").unwrap();
        b.assign(&p(r"C:\D\a.wav"), Place::Circle(m));
        b.assign(&p(r"C:\D\b.WAV"), Place::Group(m, g));
        b.assign(&p(r"C:\D\c.pdf"), Place::Circle(k));
        b.assign(&p(r"C:\D\d.pdf"), Place::Circle(k));
        b.assign(&p(r"C:\D\e.pdf"), Place::Circle(m));
        b.assign(&p(r"C:\D\x.lnk"), Place::Circle(m));
        b.assign(&p(r"C:\D\y.lnk"), Place::Circle(m));
        b
    }

    #[test]
    fn suggests_by_extension_majority() {
        let un = [p(r"C:\D\new.wav"), p(r"C:\D\n2.wav"), p(r"C:\D\book.pdf"), p(r"C:\D\app.lnk"), p(r"C:\D\README")];
        let s = suggest(&board(), &un, &[]);
        assert_eq!(s.len(), 2);
        assert_eq!((s[0].ext.as_str(), s[0].circle, s[0].paths.len()), ("wav", 0, 2), "groups count for their circle");
        assert_eq!((s[1].ext.as_str(), s[1].circle), ("pdf", 1));
    }

    #[test]
    fn no_suggestion_without_enough_examples_a_clear_winner_or_after_dismissal() {
        let mut b = board();
        b.assign(&p(r"C:\D\f.pdf"), Place::Circle(0)); // pdf: Music 2, Books 2 -> tie
        let un = [p(r"C:\D\book.pdf"), p(r"C:\D\new.wav")];
        let s = suggest(&b, &un, &[]);
        assert_eq!(s.iter().map(|s| s.ext.as_str()).collect::<Vec<_>>(), ["wav"]);
        assert!(suggest(&b, &un, &[("wav".into(), 0)]).is_empty());
        assert!(suggest(&Board::default(), &un, &[]).is_empty());
    }

    #[test]
    fn generic_extensions_are_ignored() {
        assert_eq!(ext_of(&p(r"C:\a.LNK")), None);
        assert_eq!(ext_of(&p(r"C:\a")), None);
        assert_eq!(ext_of(&p(r"C:\a.Wav")), Some("wav".into()));
    }
}
