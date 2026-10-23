# Native README media

`desktop-overview.png` and `search-results.png` are offscreen exports of DeskTuk's
own native drawing code, using example ICO references from the licensed assets in icons/.
They replace the previous hand-drawn SVG mockups. The application supplies circle geometry, labels, colors, selection and
panel controls. No user Desktop files, config, input or visible app windows are used.

They are not live screenshots. The panel's client drawing is identical to normal
painting, while the window manager's outer DWM rounding/shadow is absent from an
offscreen client export. The square client outline is retained rather than inventing
a different rounded window. Fonts depend on the exporting Windows installation and text settings. The native
ICO loader supplies the licensed example icons at the actual display size.

The presentation background uses the earlier subtle slate/blue radial gradient.
It sits behind the native UI and does not change native surface colors or corners.
Icon credit: 3D BlueFX Desktop by [WallpaperFX](https://wallpaperfx.com/),
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). See [asset provenance](icons/README.md).

Rebuild on Windows with MSVC, Rust and the built-in System.Drawing PNG encoder:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build-readme-media.ps1
```

The command runs only the explicitly ignored native drawing exporter through Cargo's
existing preview convention. It does not run the app regression suite, move the
pointer, inject input, capture the live screen or change the installed app.
`native_export.rs` is compiled only under cfg(test); it adds no runtime dependency,
worker, media loading or export interface to the shipping executable.

The English brand card at ../../res/brand/card-en.png shares the original logo and
palette with the Turkish card. README images link directly to full-resolution PNGs
so clicking opens the same content, not an SVG viewer with a different intrinsic size.
