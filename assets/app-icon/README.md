# AutoKill app icon

The mark: two crossed rifles (AK silhouettes) on a rounded near-black tile (#0d1119), the front
one in #f6faff, the back one in #8098c4, separated by a dark gap so both read as rifles.
Original to this fork, MIT OR Apache-2.0 (see LICENSE.txt).

| File | Used by |
|---|---|
| `autokill-1024.png` | the repository logo, eframe window icon source |
| `autokill.svg` | vector source, web and docs |
| `autokill.ico` | Windows executable resources (16-256) |
| `autokill.icns` | macOS app bundle |
| `hicolor/<N>x<N>/apps/ai.autokill.app.png` | Linux, freedesktop icon theme |

`build-icons.ps1` draws every PNG size (16 to 1024); 32 px and below use a simplified
silhouette so the mark still reads. The .ico and .icns are packed from those PNGs.

```powershell
powershell -ExecutionPolicy Bypass -File build-icons.ps1
```
