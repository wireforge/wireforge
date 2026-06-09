# App icons

The application icons referenced by `tauri.conf.json` (`32x32.png`,
`128x128.png`, `128x128@2x.png`, `icon.icns`, `icon.ico`) are generated from
`app-icon.svg` — the wireforge brand mark.

To regenerate after editing the source:

```sh
npm run tauri -- icon src-tauri/icons/app-icon.svg
```

Mobile (`android/`, `ios/`) and Windows Store (`Square*Logo.png`, `StoreLogo.png`)
variants are intentionally omitted; v1 targets desktop only. `tauri icon`
recreates them if a mobile or Store target is added later.
