# Roadmap

The README is the short version. This is the same plan with a bit more detail.

## v0.1, now

Mac menu bar app on macOS 14 and newer. Releases also include a 64-bit Windows installer for Windows 10 and Windows 11. Windows 7 and Windows 8 are refused, because the current WebView2 runtime does not install there. The Windows build uses a tray menu, a native title bar, File Explorer, and the Windows share and print windows.

Region, window, full screen, scrolling, repeat last region, loupe, timer, and measure. Copy text from the screen and from a capture. The donut after a capture, or a normal menu when VoiceOver or Narrator is on. The editor, including crop, magnify, an eyedropper, and shapes you can edit again later. Selectable text, and Look Up on Mac. Obfuscate by hand, plus a manual secret scan. Beautify. Presets, clipboard, save, share, print, and Open in. History, with thumbnails that follow edits and search across recognized text. Pins. Settings, including recorded shortcuts, watch folders, plugins that stay off, and an update check that stays off. Onboarding. More than one editor window.

The `shotsmith` command, `shotsmith://` links, and Apple Shortcuts on Mac are in this tree. Homebrew and winget manifests are in `packaging/`.

## Next

Nothing queued ahead of Pro. HEIC export stays off until both platforms can encode it.

## Pro

Not for sale. No account and no trial yet.

When it exists, it stays in the private repository and is only added to official builds. Planned pieces: share links, a step-guide recorder, automatic redaction, element snapping, visual diff, search, translate, alt text, style kits, and teams.
