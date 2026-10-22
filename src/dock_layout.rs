//! Pure four-edge rail geometry, in physical screen coordinates.
pub type Bounds = (i32, i32, i32, i32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
}

pub fn edge(monitor: Bounds, work: Bounds, at: (i32, i32)) -> Edge {
    let candidates = [
        (Edge::Left, work.0 - monitor.0, (at.0 - work.0).abs()),
        (Edge::Right, monitor.2 - work.2, (at.0 - work.2).abs()),
        (Edge::Top, work.1 - monitor.1, (at.1 - work.1).abs()),
        (Edge::Bottom, monitor.3 - work.3, (at.1 - work.3).abs()),
    ];
    // Prefer a reserved taskbar/appbar edge. Auto-hide/no-reservation falls back
    // to the nearest usable edge; never attach to undocumented Explorer children.
    candidates
        .iter()
        .filter(|(_, gap, _)| *gap > 0)
        .min_by_key(|(_, _, distance)| *distance)
        .or_else(|| candidates.iter().min_by_key(|(_, _, distance)| *distance))
        .map(|x| x.0)
        .unwrap_or(Edge::Right)
}

impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// The pointer can choose any work-area edge, irrespective of taskbar placement.
pub fn nearest_edge(work: Bounds, at: (i32, i32)) -> Edge {
    [
        (Edge::Left, (i64::from(at.0) - i64::from(work.0)).abs()),
        (Edge::Right, (i64::from(at.0) - i64::from(work.2)).abs()),
        (Edge::Top, (i64::from(at.1) - i64::from(work.1)).abs()),
        (Edge::Bottom, (i64::from(at.1) - i64::from(work.3)).abs()),
    ]
    .into_iter()
    .min_by_key(|(_, distance)| *distance)
    .map(|p| p.0)
    .unwrap_or(Edge::Right)
}

pub fn position(work: Bounds, edge: Edge, at: (i32, i32), size: (i32, i32), gap: i32, action_y: i32) -> (i32, i32) {
    let (w, h) = size;
    let left = (work.0 + gap).min((work.2 - w).max(work.0));
    let right = (work.2 - w - gap).max(left);
    let top = (work.1 + gap).min((work.3 - h).max(work.1));
    let bottom = (work.3 - h - gap).max(top);
    match edge {
        Edge::Left => (left, (at.1 - action_y).clamp(top, bottom)),
        Edge::Right => (right, (at.1 - action_y).clamp(top, bottom)),
        Edge::Top => ((at.0 - w / 2).clamp(left, right), top),
        Edge::Bottom => ((at.0 - w / 2).clamp(left, right), bottom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_all_taskbar_edges_and_clamps_both_menu_modes() {
        let monitor = (-1920, -1080, 0, 0);
        for (work, expected) in [
            ((-1860, -1080, 0, 0), Edge::Left),
            ((-1920, -1080, -60, 0), Edge::Right),
            ((-1920, -1020, 0, 0), Edge::Top),
            ((-1920, -1080, 0, -60), Edge::Bottom),
        ] {
            for at in [(-1900, -1000), (-1000, -500), (-10, -10)] {
                assert_eq!(edge(monitor, work, at), expected);
                for size in [(64, 64), (153, 300), (306, 600)] {
                    let (x, y) = position(work, expected, at, size, 12, 32);
                    assert!(x >= work.0 && y >= work.1 && x + size.0 <= work.2 && y + size.1 <= work.3);
                    match expected {
                        Edge::Right => assert_eq!(x + size.0, work.2 - 12),
                        Edge::Left => assert_eq!(x, work.0 + 12),
                        Edge::Top => assert_eq!(y, work.1 + 12),
                        Edge::Bottom => assert_eq!(y + size.1, work.3 - 12),
                    }
                }
            }
        }
        assert_eq!(edge(monitor, monitor, (-10, -500)), Edge::Right);
        assert_eq!(edge(monitor, monitor, (-1000, -1070)), Edge::Top);
    }
}
