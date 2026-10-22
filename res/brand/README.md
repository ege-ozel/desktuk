# DeskTuk identity

The open D gathers three circles: files finding a place on the desktop.
`logo.png` is the original transparent source, generated for DeskTuk with the
imagegen skill. `card.png` redesigns the supplied brand-card reference.
The logo also supplies the executable/tray icon and native menu heading.

The approved palette revision swaps the two color roles: teal outer D and blue
inner dots. The build script applies a deterministic RGB reflection to the original
source during resource conversion, preserving its geometry, alpha and shading.
`logo.png` remains the original source; do not replace it with the generated mock.

Run `powershell -NoProfile -File scripts/build-brand-assets.ps1` to regenerate
the card and ICO (16–256 px). No application dependency is required.

The native interface uses Segoe UI at the user's Windows icon-title text size,
with semibold headings. Colors are shared in `src/theme.rs`:

| Role | Color |
|---|---|
| Background / surface | #121B24 / #192632 |
| Raised / border | #223341 / #2B3C4E |
| Periwinkle / mint | #7FAAFF / #71D8C2 |
| Text / muted | #ECF2F8 / #94A8BC |

Dock corners use the system DWM ROUND preference (Windows 11); earlier Windows
versions retain their native fallback. Toggle radius is 8 DIP; hover rows 6 DIP.
The presentation card uses an 8 px radius at its 1120×480 export size.
Circle surfaces retain their antialiased circular silhouette and native file icons.

The English README uses card-en.png; card.png retains the Turkish tagline. Both
are generated from the same source logo and palette, without touching the app UI.
