# DeskTuk 0.1.0 release candidate review

Date: 2026-10-23 (Europe/Istanbul)
Application revision: 9a37b0ba447a2ee8f8cdee1957112305e8a444fd (D55).
Review scope: recent search, keyboard, dock/drag and packaging paths. This is a
focused code/release review, not a complete independent security audit.

## Result

No concrete release-blocking code defect found in the reviewed paths. Owner reports
that the current build behaves very well and no definite defect was reproduced in
manual use. The tentative taskbar/open-circle drag feel is not a confirmed defect;
leave interaction behavior unchanged for this candidate.

## What was reviewed

- Dock list keyboard flow: results start unselected; Down focuses the native list
  and selects row zero. Enter uses the selected row, defaulting to the first result.
- Virtual list selection/count, explicit activation and reference-only shell launch.
- Ring result paging/highlight and Enter activation; native field selection and the
  denied-foreground fallback. The temporary keyboard route checks hovered ring/editor
  scope; ordinary shortcuts outside the relevant route are passed through.
- Popup class creation, repeat registration, font/brush/corner-cache release, and
  native call boundaries without model/active-state borrows across message pumping.
- Circle/dock drag thresholds, coalesced release routing, shifted ring origins,
  capture/foreground restoration and persisted rail/cancel paths.
- Configuration writes use an app-owned temporary file followed by rename. Launches
  use existing path references through ShellExecuteExW, without command concatenation.
- Installer uses per-user installation, stable AppId, normal application close,
  startup opt-in migration and backed-up DeskTuk-specific shortcut migration.
- Cargo and Windows version resources both identify 0.1.0. Package remains x64,
  installer minimum Windows version 10.0; platform coverage beyond this owner's
  current environment has not been revalidated by this review.

## Evidence and limits

- Working tree was clean/synchronized before review. No application code changed.
- cargo fmt and cargo clippy --release --locked -- -D warnings pass during review.
- D55 release build passed; executable and installer hashes rechecked and match
  the delivered candidate. No rebuild/restart was needed for this review.
- No cargo test, native/UI input fixture, screenshot capture or benchmark executed,
  respecting the existing manual-first workflow and this read-only usage review.
  Latest automated regression status is therefore not established by this review;
  D44-D55 commits intentionally skipped CI. No live CI pass is claimed.
- New popup/keyboard behavior still needs automated regression coverage; older
  native fixture assumptions about panel sizes/layout must be checked before using
  them as evidence. Current physical manual acceptance is not a substitute for
  upgrade/uninstall, alternate DPI, IME, reboot and Explorer-restart coverage.
- Get-AuthenticodeSignature reports NotSigned for the installer. Signing is a
  distribution decision, not a runtime code defect. No certificate was purchased
  or installed and no external release was created as part of review.

## Candidate artifacts

Executable: target/release/DeskTuk.exe
SHA256: 97F9B762F7205FA6259B3EC8400332FF9824CBE0F7691B234E74F862C437ADF2

Installer: dist/DeskTuk-0.1.0-Setup-x64.exe
SHA256: 8C8142D70E77A0F84016789FC182DBD3EE08071EEDA85A712C618ECE5CECE66B

Recommendation: freeze this candidate's interaction behavior, complete the final
automated/clean installation and upgrade/uninstall checks, then publish this exact
validated artifact with versioned release notes and checksum. Broader performance
claims require new measurements; this review makes none.
