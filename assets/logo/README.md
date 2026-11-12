# Lockin logo

The main logo is the **locked book**: an open book with a padlock shackle on top. The
**padlock O** wordmark is kept as an alternative. Both use only black and grey, and the
wordmark is Space Grotesk Bold (converted to outlines, so no font is needed).

Naming: `on-dark` / `on-light` files have a transparent background and are meant for a dark or
a light background. The app icons (`lockin-icon-dark` / `-light`) carry their own rounded square.

| File | What it is |
| --- | --- |
| `svg/lockin-mark-on-dark.svg`, `-on-light.svg` | The locked book alone |
| `svg/lockin-logo-on-dark.svg`, `-on-light.svg` | Locked book + "Lockin" (main logo) |
| `svg/lockin-wordmark-padlock-on-dark.svg`, `-on-light.svg` | "Lockin" with the padlock O (alternative) |
| `svg/lockin-icon-dark.svg`, `-light.svg` | App icon on a rounded square (the dark one is the app's icon) |
| `svg/tray/lockin-tray-{48,32,22}-*.svg` | Tray icons: heavier shackle and wider page gap per size |
| `png/` | PNG exports of the above: 1024 px icons and marks, logos 256 px high, tray icons at 1x and 2x |

| Colour | Hex |
| --- | --- |
| Ink on dark | `#F2F2F3` |
| Ink on light | `#111113` |
| Dark background | `#0E0E10` (edge `#2A2A2F`) |
| Light background | `#F4F4F5` |

## Regenerating

The SVGs come from `tools/gen_logo.py`; the PNGs are rendered with `rsvg-convert` (from `librsvg`).
The Tauri app icons in `app/src-tauri/icons/` are generated from the dark 1024 px icon:

```bash
cd app
pnpm tauri icon ../assets/logo/png/lockin-icon-dark-1024.png
rm -rf src-tauri/icons/android src-tauri/icons/ios   # desktop only
```

The daemon builds the tray PNGs (`png/tray/`) into its binary for the tray icon
(`crates/lockin-daemon/src/desktop.rs`): the `on-dark` ones for a dark Plasma panel, `on-light` for a light one.
