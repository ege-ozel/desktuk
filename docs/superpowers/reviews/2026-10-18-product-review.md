# DeskTuk product and scope review

This is a set of proposals for owner review, not an approved implementation plan. Current
work is finishing the circle-move interaction; after that, wait for the owner's priorities.

## Develop

- **Discoverable circle movement:** keep middle-button drag for moving an open circle, add a
  visible hint/handle, and consider a keyboard or settings-based alternative for trackpads
  without a clickable wheel.
- **First-use guidance:** briefly explain hover/click to open, centre click to page, middle
  drag to move, and grid snapping on release.
- **Interaction/accessibility settings:** offer keyboard navigation, adjustable circle/icon
  size, reduced animation, and clearer focus/selection feedback.
- **Desktop state diagnostics:** make missing or renamed shortcuts easier to recognize and
  ensure Recent/counts update predictably when Explorer changes the Desktop.
- **Multi-monitor review:** assess grid selection, saved positions, DPI changes and taskbar
  work areas across monitors before adding more layout options.

## Change

- **Circle dragging:** let the circle track the pointer freely, show the target cell, and
  snap only on release. D29 implements this; physical owner review remains important.
- **Dock typography:** use the same Windows icon-title font as the desktop labels. D29
  implements this at the dock window's DPI.
- **Visual consistency:** decide whether dock rows, circle labels and ring labels should
  share one scale/weight/contrast system while keeping the current blue/teal direction.
- **Topmost behavior:** circles currently remain above other windows by owner decision. If
  this ever feels intrusive, review an explicit, persisted mode rather than changing it
  implicitly.

## Consider removing after compatibility review

- **Legacy panel UI:** `--ui panel` and `ui=panel` keep the earlier panel available. If the
  circles UI is the only intended product, measure whether anyone still relies on panel mode
  before removing its UI path; preserve config migration either way.
- **Superseded implementation paths:** before deleting old host/order branches or migration
  code, check current configs and the isolated tray/Show Desktop fixtures. They protect
  startup, recovery and upgrade behavior even when users rarely see them.

## Review questions

1. Should middle-button drag be the primary way to move a circle while closed-circle left
   drag remains available?
2. Which two items from Develop or Change matter most for the next milestone?
3. Is the legacy panel still useful, or should it become a later removal candidate?
