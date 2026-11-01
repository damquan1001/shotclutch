# macOS DMG window

The committed Finder layout and original ShotClutch artwork are copied into the DMG by
`packaging/macos/package.sh`. Packaging needs no mounted image, Finder scripting or image tools.

| File | Purpose |
|---|---|
| `background.svg` | Original 660 x 400 neutral background, native ShotClutch text and an installation arrow. No external application artwork. |
| `background.tiff` | Committed RGB/Deflate representations at 660 x 400 (72 dpi) and 1320 x 800 (144 dpi). |
| `dmg-layout.DS_Store` | Fixed Finder settings: 128 pt icons, ShotClutch.app at (180, 205), Applications at (480, 205), and the background alias on the ShotClutch volume. |
| `generate.py` | Deterministic layout writer using ds_store and mac_alias, without reading local volume/user metadata. |

The volume is named `ShotClutch`; the background alias uses that exact name. Finder draws black
icon labels even with a custom background, so the icon area stays light and the arrow sits between
the icon boxes. The original application icon comes from `assets/app-icon/shotclutch.svg`.

From the repository root, regenerate all artwork and the layout with:

```sh
python -m pip install PySide6 'Pillow>=12' ds_store==1.3.3 mac_alias==2.2.3
python scripts/generate-icons.py
```

The layout alone can be regenerated with `python packaging/macos/dmg/generate.py`. It uses a fixed
alias date and no local paths or volume UUID. Artwork variants are committed; normal builds do not
regenerate them. Text rendering explicitly loads the retained Inter-Regular.ttf from this repository,
whose authorship and OFL notice remain in the asset inventory and packages.

The inherited layout-writing approach follows VectorCraft's `packaging/macos/dmg/`
(storytold/vectorcraft#493); retained source copyright/license notices remain applicable.
