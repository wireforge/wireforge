# App icons

This directory holds the application icons referenced by `tauri.conf.json`
(`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`).

They are **not committed yet** and must be generated before `tauri dev` or
`tauri build` will compile (Tauri embeds an app icon at build time).

Generate them from a single square source image (1024x1024 PNG recommended):

```sh
npm run tauri icon path/to/source.png
```

This produces all required sizes and formats in this directory.
