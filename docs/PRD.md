# Shotsmith — Product Requirements Document

**Version:** 0.8 (draft) · **Date:** 19 Oct 2026 · **Owner:** Ciaran · **Status:** Ready for build planning
**Name:** Shotsmith (decided) · bundle ID `com.shotsmith.Shotsmith` · URL scheme `shotsmith://` · CLI `shotsmith` · documents `.shotsmith`, guides `.shotguide`, style kits `.shotkit`. Check trademark registers (IP Australia, USPTO), domains and the App Store before launch.
**Design reference:** Design canvas “Shotsmith — macOS Screenshot Tool” — “v1 Core” page (screens 00–09, all free) and “Pro — post-v1” page (paid screens). Screen IDs below (`S01`…) map to the artboards.

---

## 1. Summary

Shotsmith is a free screenshot and annotation tool for **macOS and Windows**, built with **Rust + Tauri 2** so both platforms share one codebase and one design. It is **open core**: the full app is MIT-licensed and public; an optional paid **Pro** module is closed source. It delivers a complete power-user feature set — region/window/full-screen capture, a fast annotation editor with obfuscation, step counters, effects, filename patterns and multiple output destinations — with a Mac-quality look that adapts window chrome, fonts and shortcuts to each platform (§9.1).

From **Filee** (open-source file converter) it borrows three ideas:

1. **The donut** — a radial quick-action menu that opens at the cursor after each capture.
2. **Presets + automation** — named output recipes, a CLI, and watch folders.
3. **Offline, private, no account** — no network by default, no telemetry, no sign-in.

**Positioning:** the best free screenshot tool on Mac and Windows — everything paid competitors charge for is free (§5–§6). After v1.0, **Shotsmith Pro** (US$3.99/month, 14-day trial) adds paid professional, automation and team features, an account, and cloud share links (§14). The free app never needs an account or network.

## 2. Goals and non-goals

### Goals

- G1. Capture → delivered (clipboard or file) in **≤ 2 keystrokes** after the hotkey.
- G2. One design on both platforms that still feels at home: system font, vibrancy (Mac) / Mica (Windows), light/dark, system accent colour, reduced motion/transparency, screen readers (VoiceOver, Narrator).
- G6. Feature parity between Mac and Windows for the free app; documented exceptions only (§4.2).
- G3. Ship the full core feature baseline (see §6.4 baseline table).
- G4. Free app: zero network traffic unless the user enables updates or an uploader plugin. Pro: network only for sign-in, licence checks and cloud features the user explicitly uses.
- G5. Scriptable: CLI, URL scheme, watch folders on both platforms; Apple Shortcuts (App Intents) on Mac.

### Non-goals (v1)

- Screen **recording** / GIF / video (consider post-v1).
- Cloud hosting, accounts, team libraries in the free app (Pro adds accounts and share links post-v1, §14).
- Linux builds (Tauri makes this possible later).
- Mac App Store distribution in v1 (sandbox limits global capture workflows; revisit later).

## 3. Users

| Persona | Need | Key features |
|---|---|---|
| Support / IT tech | Annotated screenshots for tickets and guides, redact customer data | Counters, arrows, pixelate, Bug-report preset |
| Developer / QA | Fast bug captures, window shots with shadow, OCR of error text | Window capture, Copy text, repeat region, CLI |
| Docs / content writer | Consistent images, naming, folders | Presets, filename patterns, effects, scrolling capture |
| Mixed Mac/Windows teams | Same tool, same look, same files on both | Shared presets, `.shotsmith` files, style kits |

## 4. Platform and tech stack

**Decision:** Rust core + **Tauri 2** shell with a web UI rendered by each OS’s built-in web view (WKWebView on Mac, WebView2 on Windows). Platform-specific code sits behind Rust traits in a `platform` crate.

| Area | Decision |
|---|---|
| Minimum OS | **macOS 14 Sonoma** (ScreenCaptureKit screenshot API) · **Windows 11**; Windows 10 22H2 best-effort (no longer supported by Microsoft) |
| Architectures | Mac: universal (Apple silicon + Intel) · Windows: x64 + ARM64 |
| Core language | **Rust** (stable, 2021+ edition): capture, document model, export renderer, presets, history, OCR/redaction pipeline, CLI |
| App shell | **Tauri 2**: windows, tray/menu bar, global shortcuts, deep links, autostart, updater, IPC |
| UI | **TypeScript + React + Vite** inside Tauri; one component library implementing the canvas designs; CSS tokens per platform (§9.1) |
| Editor canvas | Live view drawn in the web view (Canvas 2D/WebGL) from the Rust document model; **export rendering in Rust** (`tiny-skia`) so files are identical on both platforms and redactions are flattened in Rust |
| Window effects | `window-vibrancy` (Mac vibrancy, Windows Mica/Acrylic) |
| Capture | Mac: ScreenCaptureKit (via `objc2` bindings or `screencapturekit` crate) · Windows: Windows Graphics Capture (`windows` crate), DXGI fallback. `xcap` acceptable for the first prototype |
| OCR | Mac: Apple Vision · Windows: `Windows.Media.Ocr` — both behind one `Ocr` trait |
| Accessibility tree (snapping, measure, step captions) | Mac: AX API · Windows: UI Automation |
| Image I/O | `image` crate (PNG, JPEG, TIFF), `webp` (lossy/lossless WebP), PDF via `printpdf`; HEIC via OS codecs (ImageIO on Mac, WIC + HEIF extension on Windows) and hidden when unavailable |
| Global shortcuts | `tauri-plugin-global-shortcut` |
| Clipboard | `tauri-plugin-clipboard-manager` (image + text) plus native code for rich clipboard (alt text, file promises) |
| Launch at login | `tauri-plugin-autostart` |
| URL scheme | `tauri-plugin-deep-link` (`shotsmith://`) |
| Updates | `tauri-plugin-updater` (signed), **off until user opts in** |
| Storage | SQLite via `rusqlite` with FTS5 (history + OCR search) |
| Mac Shortcuts | App Intents via a small Swift extension target (v1.0, Mac only) |
| Build/CI | Cargo workspace + pnpm; GitHub Actions matrix (macos-latest, windows-latest) |
| Distribution | Mac: Developer ID signing + notarisation, Homebrew cask · Windows: signed MSI/NSIS installer (Tauri bundler), winget |
| Licence | **Open core:** MIT core in a public repo; Pro module proprietary in a private repo, compiled into official signed builds (§14). Third-party notices file |

### 4.1 Platform layer (Rust traits)

`Capturer` (displays, windows, freeze, scrolling), `Overlay` (per-display transparent windows), `Ocr`, `AxTree` (element under cursor, labels), `Clipboard`, `ShareSheet`, `Permissions`, `Tray`, `FileReveal`, `Printer`. Each has `mac` and `windows` modules; everything else is shared.

### 4.2 Platform differences (accepted)

| Feature | Mac | Windows |
|---|---|---|
| Capture permission | Screen Recording permission prompt | None needed |
| Share | System Share sheet | Windows Share UI |
| Apple Shortcuts (App Intents) | Yes (v1.0) | No — use CLI / URL scheme |
| HEIC export | Yes | Only if the HEIF extension is installed |
| Window shadow on window capture | Native shadow + transparency | Synthesised shadow |
| Pro: translate overlay | Apple Translation | Later (bundled on-device model) |
| Pro: alt text & smart names | Apple on-device model (macOS 26) | OCR-based fallback |

## 5. Scope by release

| Release | Contents |
|---|---|
| **Spike (v0.0)** | Two-week prototype on both OSes: hotkey → overlay latency, frozen-display capture on mixed-DPI multi-monitor, transparent overlay windows, export renderer parity. Go/no-go on Tauri (§12 R5) |
| **MVP (v0.1)** | Mac + Windows: menu bar/tray app, region / window / full-screen / repeat-last capture, loupe, donut, editor (select, crop, rect, ellipse, line, arrow, freehand, text, counter, highlight, pixelate/blur, undo/redo), clipboard + save + save-as, presets (format, folder, pattern), history (last 50), settings, onboarding/permissions |
| **v0.2** | Pins, Copy text (OCR), secret detection + manual redaction (S09), timed capture, effects (shadow, border, torn edges, rounded, grayscale, invert), magnify, callout, eyedropper, Share sheet, Open in…, print |
| **v0.3** | Beautify (S06), basic measure (S07), live text select/copy + copy as code (S08), scrolling capture, CLI, URL scheme, watch folders, `.shotsmith` editable documents, object library (save/load objects) |
| **v1.0** | Plugin destinations (opt-in uploaders), localisation, signed updater, Homebrew cask + winget, Mac App Intents, accessibility audit, docs site |
| **Pro (post-v1, paid)** | Smart redaction (auto + faces), element-aware snapping, step-guide recorder, translate overlay, visual diff, inspect pro (contrast + capture with marks), search by meaning, alt text & smart filenames, style kits — see §14 |

---

## 6. Functional requirements

Each requirement has an ID for traceability. **AC** = acceptance criteria.

**Platform note:** requirements apply to **both Mac and Windows**. Where they name Mac APIs or keys, use the Windows equivalent from §4 and §9.2 (e.g. `NSPanel` → transparent Tauri overlay window per display; Vision → `Ocr` trait; ⌘ → Ctrl, ⌥ → Alt; menu bar → system tray; `~/Library/Application Support/Shotsmith` → `%APPDATA%\\Shotsmith`; System Settings deep links → Windows Settings). Exceptions are listed in §4.2.

### 6.1 App shell and menu bar (S04)

- **APP-1** Runs as a menu bar agent (`LSUIElement = YES`). Optional “Show Dock icon” setting.
- **APP-2** Menu bar icon: template image (corner brackets + dot), highlights while capture overlay is active.
- **APP-3** Menu contents (top to bottom): Capture section (Region, Repeat last region, Window, Full screen, Scrolling…, Timed ›, Copy text from screen) each with its shortcut; Recent (3 thumbnails + “Show History ⌘Y”); Open image… ⌘O; Edit image from clipboard ⌘⇧V; Pinned images ›; Settings… ⌘,; Quit ⌘Q.
- **APP-4** Clicking a Recent thumbnail opens it in the editor; right-click offers Copy, Reveal in Finder, Pin, Delete.
- **APP-5** Post-action HUD (bottom-right, vibrant dark, 2.5 s, respects Reduce Motion): thumbnail, result (“Copied to clipboard”, “Saved to …”), and an **Edit** button.
- **AC:** App launches with no window; menu shows correct shortcuts reflecting user remaps; HUD dismisses on click or timeout.

### 6.2 Capture (S01)

- **CAP-1 Region.** Hotkey freezes all displays (screenshot each display first, then show selection over the frozen image) so menus/tooltips are captured exactly as shown.
- **CAP-2 Overlay.** One borderless, transparent, always-on-top overlay window per display (pre-created and hidden for speed) at screen-saver level; dims outside the selection at ~48% black; 1 pt white outline with 8 handles; selection is adjustable after first drag until confirmed.
- **CAP-3 Size label.** Above selection: `W × H pt` and `W × H px @2x`.
- **CAP-4 Loupe.** 11×11 px magnified grid near cursor with crosshair, coordinates and hex colour; ⌘C copies hex; Tab toggles loupe.
- **CAP-5 Modifiers.** Space = toggle window mode; ⇧ = lock aspect; ⌥ = resize from centre; arrows nudge 1 pt (⇧ = 10 pt); ↩ confirms; Esc cancels; double-click selection = confirm.
- **CAP-6 Window mode.** Hover highlights windows (from the platform window list), excluding Shotsmith’s own; click captures that window with optional native shadow and transparent corners (setting).
- **CAP-7 Full screen.** Captures the display under the cursor; setting for “all displays stitched”.
- **CAP-8 Repeat last region.** Re-captures the previous rect on the same display without UI.
- **CAP-9 Timed.** 3 / 5 / 10 s countdown HUD, then region or full screen per setting.
- **CAP-10 Cursor.** Option to include cursor as a separate, movable/deletable editor object.
- **CAP-11 Scrolling capture (v0.3).** User selects a region; Shotsmith scrolls the target (synthetic scroll events, requires Accessibility permission, prompted only when first used) and stitches frames by overlap matching; manual “Add frame” fallback.
- **CAP-12 Copy text from screen.** Region select → on-device OCR (`Ocr` trait) → text to clipboard; HUD shows first line; language auto-detect.
- **CAP-13 Bottom toolbar.** Floating pill with modes (Region, Window, Screen, Scrolling), Timer, Cursor toggle, Capture button; hint strip beneath. Can be hidden in Settings (“Minimal overlay”).
- **CAP-14 Sound** optional shutter sound (off by default).
- **AC:** Hotkey to overlay visible ≤ 150 ms on M1; captured pixels match the frozen frame exactly; Retina captured at native resolution; works across mixed-DPI multi-monitor setups; Esc leaves no residue windows.

### 6.3 Donut quick-action menu (S02)

- **DON-1** Appears centred on the cursor immediately after capture confirm (unless “After capture” is set to a direct action).
- **DON-2** 8 slots, clockwise from top; defaults: 1 Copy (default ↩), 2 Save, 3 Annotate, 4 Pin, 5 Share, 6 Copy text, 7 [first preset], 8 Open in….
- **DON-3** Selection: hover/click, **flick** (mouse movement > 40 pt toward a slice within 300 ms then release), number keys 1–8, ↩ for default, Esc dismisses (capture stays in History).
- **DON-4** ⇧ + slice runs the action and keeps the donut open (chain e.g. Copy + Save).
- **DON-5** Centre shows thumbnail + pixel size.
- **DON-6** Slots configurable in Settings › Donut: any action or preset; slot count 4/6/8; drag to reorder with live preview.
- **DON-7** Visuals: dark vibrant ring, hovered slice filled with accent; spring-in animation ≤ 120 ms, replaced by fade when Reduce Motion is on.
- **DON-8** Accessibility: each slice is an accessibility element with label and key; screen-reader users (VoiceOver, Narrator) get a standard menu instead.
- **AC:** Donut fully on-screen near display edges (shift centre inward); keyboard-only flow works; no focus theft from the frontmost app after dismissal.

### 6.4 Annotation editor (S03)

Window: unified toolbar (title + edit state, tool group centred, Undo/Copy/Share, primary **Save · [preset] ▾** split button), canvas, right inspector (272 pt, collapsible ⌘⌥I), status bar (pixel size, scale, object count, hint, output folder), floating zoom control.

**Tools and one-key shortcuts**

| Tool | Key | Notes |
|---|---|---|
| Select | V / Esc | Multi-select ⇧-click, marquee, ⌘A; arrow-key nudge |
| Crop | C | Handles + ↩ apply; Auto-crop solid borders (menu) |
| Arrow | A | Curved by dragging midpoint; heads: end / both / none |
| Rectangle | R | Fill none/solid/translucent; corner radius |
| Ellipse | E | |
| Line | L | ⇧ snaps 15° |
| Freehand | F | Smoothed strokes |
| Text | T | Double-click to edit, ⇧↩ newline, fill None/White/Colour |
| Callout | B | Speech bubble with movable tail |
| Step counter | N | Auto-increments; renumber on delete; style circle/square |
| Highlight | H | Modes: marker, area (darken outside), grayscale outside, magnify |
| Obfuscate | O | Pixelate / Blur / Solid; block size / radius |
| Magnify | M | Circular zoom lens object, 2–4× |
| Eyedropper | I | Picks colour into current style |

- **ED-1** All annotations are vector objects until export; full undo/redo (⌘Z / ⇧⌘Z), unlimited within session.
- **ED-2** Modifiers: ⇧ constrain proportions / 15° angles; ⌥ draw from centre; ⌘-drag duplicate.
- **ED-3** Inspector shows properties of selection, else of active tool: colour (8 swatches + custom via the system colour picker), stroke width, dash, shadow, font, size, fill, opacity.
- **ED-4** Last-used style per tool persists.
- **ED-5 Image effects** (export-time, non-destructive): drop shadow, border, torn edges, rounded corners, grayscale, invert.
- **ED-6 Obfuscation is destructive on export**: exported files never contain original pixels under a pixelate/blur/solid region. A unit test must verify this.
- **ED-7** Drag in images/files as objects; paste images; “Insert window” capture.
- **ED-8** Canvas: zoom 10–800 %, ⌘0 fit, ⌘1 100 %, pinch/scroll zoom, space-drag pan, canvas expand when objects exceed bounds (option).
- **ED-9** Save formats: PNG, JPEG, HEIC, WebP, TIFF, PDF, and **`.shotsmith`** (editable: JSON + original image, zip package).
- **ED-10** Shortcuts: ⌘C copy flattened, ⌘S save with current preset, ⇧⌘S save as, ⌘P print, ⌘E share, ⌘W close (prompt if unsaved and not in History).
- **ED-11** Rotate 90° CW/CCW, flip, resize image (menu).
- **ED-12 Object library (v0.3):** save selected objects as a reusable stamp; load from Library popover.

**Core feature baseline**

| Feature | Release |
|---|---|
| Rectangle, ellipse, line, arrow, freehand, text | ✅ MVP |
| Highlight text/area/grayscale/magnify | ✅ MVP (magnify v0.2) |
| Obfuscate pixelize/blur | ✅ MVP (+ Solid) |
| Step counter, speech bubble | ✅ MVP / v0.2 |
| Crop, auto-crop, rotate | ✅ |
| Border, shadow, torn edges, grayscale, invert | ✅ v0.2 |
| Save/load objects | ✅ v0.3 |
| Capture mouse pointer as object | ✅ |
| Filename patterns, JPEG quality | ✅ via presets |
| Destination picker / direct export | ✅ donut / “After capture” setting |
| Print with fit/rotate options | ✅ v0.2 |
| Plugin destinations | ✅ v1.0 (opt-in) |

### 6.5 Presets (S05)

- **PRE-1** A preset = name, format, quality, scale (@1x / native), folder, filename pattern, after-save actions, effects, metadata stripping, donut slot, optional global shortcut.
- **PRE-2** Filename tokens: `{app}` `{window}` `{yyyy}` `{MM}` `{dd}` `{HH}` `{mm}` `{ss}` `{w}x{h}` `{n}` (counter); invalid filename characters sanitised; collisions append ` (2)`. Live preview under the field.
- **PRE-3** Built-in presets: Docs PNG (default), Clipboard only, Bug report (JPEG 80, cursor, border), Retina WebP, Print PDF.
- **PRE-4** One preset is default (used by ⌘S and ↩ “Save”). Import/export presets as JSON.
- **PRE-5** Strip metadata by default (no location/EXIF beyond colour profile).
- **AC:** Pattern preview matches saved filename byte-for-byte; quality slider disabled for lossless formats.

### 6.6 History and pins

- **HIS-1** Every capture stored in History (`~/Library/Application Support/Shotsmith/History/`) with original + edit document; retention configurable (count and/or days; default 50 / 30 days); “Never keep history” option.
- **HIS-2** History window (⌘Y): grid of thumbnails, search by app/window name/OCR text (OCR indexed lazily, local only), filters (today, app), actions: open, copy, reveal, pin, delete.
- **PIN-1** Pin = borderless always-on-top panel; drag to move, scroll to resize, ⌥-scroll opacity, hover toolbar (opacity, click-through lock, annotate, close), double-click opens editor.
- **PIN-2** Pins persist across relaunch (setting).

### 6.7 Destinations and sharing

- **DST-1** Built-in: Clipboard, File (preset), Save as…, Share sheet (Mac Share sheet / Windows Share UI), Open in… (app picker, remembers recent), Print, Email (via Share), Pin.
- **DST-2** Settings › Destinations: choose “After capture” behaviour — Show donut (default) / Run a destination directly / Open editor.
- **DST-3 (v1.0)** Plugin destinations (e.g. Imgur, S3, custom HTTP/command). Disabled by default; each shows exactly what it sends and where; no plugin is bundled enabled.

### 6.8 Automation

- **AUT-1 CLI** `shotsmith` (installed via Settings › Advanced or Homebrew):
  - `shotsmith capture region|window|screen|last [--preset NAME] [--delay S] [--clipboard] [--output PATH]`
  - `shotsmith ocr [--region x,y,w,h]`
  - `shotsmith presets list|export|import`
  - `shotsmith watch PATH --preset NAME`
  - `--json` output and documented exit codes (0 ok, 1 cancelled, 2 permission denied, 3 invalid args, 4 I/O error).
- **AUT-2 URL scheme** `shotsmith://capture?mode=region&preset=Docs%20PNG`.
- **AUT-3 App Intents**: Take Screenshot (mode, preset), Copy Text from Screen, Apply Preset to Image.
- **AUT-4 Watch folders**: apply a preset (effects, format, rename, move) to images dropped in a folder — e.g. point at the macOS native screenshot folder to post-process ⌘⇧4 shots. Process only after file write completes (size stable for 1 s).

### 6.9 Settings (S05)

Settings window (web UI, sidebar layout) with: **General** (launch at login, Dock icon, sound, HUD, appearance), **Capture** (freeze screen, cursor, window shadow, loupe, overlay toolbar, multi-display), **Shortcuts** (all global + editor tool keys; conflict detection; guidance to disable macOS ⌘⇧3/4/5 in System Settings › Keyboard › Keyboard Shortcuts › Screenshots if the user wants Shotsmith on those keys), **Presets**, **Donut**, **Destinations**, **Automation** (CLI install, watch folders, URL scheme toggle), **Plugins**, **Advanced** (history location/retention, reset, export diagnostics locally), **About** (version, licence, notices, update check opt-in).

### 6.10 Onboarding and permissions

- **ONB-1** First run: 3-step sheet — Screen Recording permission (deep link to System Settings › Privacy & Security › Screen & System Audio Recording), choose shortcuts (offer to use ⌘⇧2 family), pick default after-capture behaviour.
- **ONB-2** Detect revoked permission at capture time; show a clear inline fix, never a silent failure.
- **ONB-3** Accessibility permission requested only when scrolling capture is first used.

### 6.11 Beautify, measure, live text and secret detection (free)

These were reviewed out of the Pro tier: competitors include them, and secret detection supports the “private by default” promise. Pro-only extensions are noted and specified in §14.

**Beautify (S06, v0.3)**

- **BTY-1** Beautify tab in the editor: size presets (Auto, 16:9, 4:3, 1:1, 1200×630, custom), backdrop (solid colours, transparent, user image), frame (none, window, browser, phone), padding, corner radius, shadow, auto-balance.
- **BTY-2** “Save look as preset” stores the settings in a preset (PRE-1).
- **BTY-3** Watermark from a style kit is Pro (PRO-10); a plain text watermark is free.
- **AC:** Output exactly matches the chosen size; transparent backdrop exports with alpha in PNG/WebP/HEIC.

**Measure (S07, v0.3)**

- **MEA-1** Measure mode (global hotkey or `M` in the capture overlay): rulers, element size on hover, ⌥ for distances to neighbours/edges, drag guides, ⇧ to lock axis, points with px toggle.
- **MEA-2** Inspector HUD: size, gap, corner radius, text size (where Accessibility exposes it), colour picker with hex copy.
- **MEA-3** Contrast checker and “Capture with marks” are Pro (PRO-7).
- **AC:** Measurements accurate to 1 pt on Retina and non-Retina displays.

**Live text (S08, v0.3)**

- **LT-1** “Live text” mode in the editor: all recognised text is selectable; popover offers Copy, Copy as code (language detected, indentation kept), Look up.
- **LT-2** Side panel shows all extracted text with Copy all.
- **LT-3** Translate and the translation overlay are Pro (PRO-4).
- **AC:** Works offline via Vision; copied code keeps indentation.

**Secret detection and redaction (S09, v0.2)**

- **RED-1** “Scan” in the editor detects names, email addresses, phone numbers, postal addresses, card numbers (Luhn check), IBAN/account-like numbers, and API keys/tokens (pattern + entropy check), using Vision OCR, `NSDataDetector` and NaturalLanguage, on-device.
- **RED-2** Side panel lists findings with type, preview and state; each can be toggled; “Redact remaining” applies pixelate / blur / solid. Secrets default to solid black.
- **RED-3** On save/copy, a non-blocking warning appears if likely secrets (keys, card numbers) are unredacted (setting, on by default).
- **RED-4** Face detection and auto-redact per preset are Pro (PRO-1).
- **AC:** Detection under 1 s for a 5K capture on Apple silicon; exported file has no original pixels in redacted regions (reuses ED-6 test); dismissed false positives stay dismissed for that capture.

---

## 7. Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-1 Performance | Hotkey → overlay ≤ 150 ms on both OSes (overlay windows pre-warmed and hidden; measured in the spike); confirm → donut ≤ 100 ms; editor open ≤ 300 ms for 5K image; idle CPU ~0 %, idle RAM < 120 MB (web view) |
| NFR-2 Privacy | Free app: no network calls by default (verify with a CI test that the app binary makes no connections in a smoke run); no analytics; crash reports only saved locally. Pro network use is limited to §14.7 |
| NFR-3 Accessibility | Semantic HTML + ARIA so VoiceOver and Narrator work; keyboard operation of every feature; contrast ≥ 4.5:1; reduced motion/transparency honoured on both OSes |
| NFR-4 Appearance | Light/dark, system accent colour, one icon set (SVG) on both OSes, vibrancy (Mac) / Mica (Windows); platform adaptations per §9.1 |
| NFR-5 Reliability | No lost captures: write to History before any destination runs; atomic file writes |
| NFR-6 Localisation | ICU message files shared by UI and Rust from day one; English first |
| NFR-7 Security | Mac hardened runtime + notarisation; Windows code signing; signed updater; strict Tauri capability allow-list (UI can only call listed Rust commands); CSP with no remote content; plugins run out-of-process |
| NFR-8 Size | Download < 20 MB per platform |

---

## 8. Architecture

```
shotsmith/                       (public repo, MIT)
├─ Cargo.toml                    # workspace
├─ crates/
│  ├─ core/                      # document model, objects, undo, presets, filename patterns
│  ├─ render/                    # export renderer (tiny-skia), effects, redaction flattening
│  ├─ capture/                   # capture orchestration, freeze, scrolling stitcher
│  ├─ platform/                  # traits + mac/ and windows/ implementations (§4.1)
│  ├─ ocr/                       # OCR pipeline, secret detection (RED-1)
│  ├─ history/                   # SQLite store, thumbnails, FTS index
│  ├─ output/                    # encoders, destinations
│  ├─ automation/                # CLI (`shotsmith` binary), URL scheme, watch folders
│  └─ plugin-host/               # out-of-process plugin protocol (v1.0)
├─ app/                          # Tauri 2 app (src-tauri) — commands, windows, tray, capabilities
├─ ui/                           # React + TypeScript: overlay, donut, editor, settings, HUD
│  └─ tokens/                    # design tokens: shared + mac.css + windows.css
├─ mac-intents/                  # Swift App Intents extension (Mac only)
└─ tests/                        # Rust unit, render snapshot, Playwright UI tests

shotsmith-pro/                   (private repo, proprietary) — `pro` crate + UI package, linked in official builds
```

Data models are Rust structs serialised with `serde` (JSON), mirrored to TypeScript types generated with `ts-rs` or `specta`.

### Key data models (Rust sketch)

```rust
#[derive(Serialize, Deserialize)]
pub struct Preset {
    pub id: Uuid,
    pub name: String,
    pub format: ImageFormat,            // Png, Jpeg, Heic, Webp, Tiff, Pdf
    pub quality: f32,                   // 0.0..=1.0, ignored for lossless
    pub scale: OutputScale,             // Points (@1x) | Native
    pub folder: Option<PathBuf>,        // None = clipboard only
    pub filename_pattern: String,       // "{app}-{yyyy}-{MM}-{dd} {HH}.{mm}.{ss}"
    pub after_save: Vec<AfterSaveAction>, // CopyImage, CopyPath, Reveal, OpenEditor
    pub effects: Vec<Effect>,           // Shadow, Border, TornEdges, Rounded, Grayscale, Invert
    pub strip_metadata: bool,
    pub donut_slot: Option<u8>,
    pub shortcut: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct CaptureRecord {
    pub id: Uuid,
    pub date: DateTime<Utc>,
    pub mode: CaptureMode,              // Region, Window, Screen, Scrolling, Ocr
    pub source_app: Option<String>,
    pub window_title: Option<String>,
    pub pixel_size: (u32, u32),
    pub scale: f32,
    pub original_path: PathBuf,
    pub document_path: Option<PathBuf>, // .shotsmith if edited
    pub ocr_text: Option<String>,
    pub platform: Platform,             // Mac | Windows
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnnotationObject {             // all vector; rendered by `render` crate
    Rect(RectObject), Ellipse(EllipseObject), Line(LineObject), Arrow(ArrowObject),
    Freehand(PathObject), Text(TextObject), Callout(CalloutObject), Counter(CounterObject),
    Highlight(HighlightObject), Obfuscate(ObfuscateObject), Magnify(MagnifyObject),
    Image(ImageObject), Cursor(ImageObject),
}
```

`.shotsmith` document = zip package (identical on Mac and Windows): `document.json` (version, canvas size, objects, effects) + `original.png`.

### Capture pipeline

1. Hotkey → `platform::Capturer::freeze_all_displays()` → one image per display (ScreenCaptureKit on Mac, Windows Graphics Capture on Windows).
2. Pre-warmed overlay windows show the frozen images; user selects rect / window in the web UI; selection sent to Rust over Tauri IPC.
3. Crop frozen image (or capture window image directly for transparent shadow) → `CaptureRecord` written to History.
4. Route by “After capture” setting → Donut / direct destination / Editor.

---

## 9. Design spec (matches canvas)

| Token | Value |
|---|---|
| App icon | Charcoal rounded square; white anvil with a dashed ember (`#FF9F0A`) selection frame above it and a small spark. Menu bar: monochrome template version (anvil + dashed frame) |
| Font | Platform system font (§9.1); mono for sizes, shortcuts, patterns |
| Accent | System accent colour (designs use `#0A84FF`) |
| Annotation default colour | `#FF3B30`, 4 pt stroke, shadow on |
| Annotation swatches | `#FF3B30 #FF9500 #FFCC00 #34C759 #007AFF #AF52DE #1D1D1F` + custom |
| Overlay dim | black 48 % |
| HUD / donut / overlay toolbar material | dark translucent (Mac vibrancy / Windows Acrylic), 1 px inner white 10 % stroke |
| Radii | windows 12 (system), panels/menus 12, controls 6–9, pills 999 |
| Donut | outer Ø 360 pt, inner Ø 140 pt, 8 slices, 1.6° gaps, centre Ø 120 pt |
| Inspector width | 272 pt |
| Editor canvas background | `#E3E3E8` light / `#1E1E1E` dark |

Screens: **S00** Overview · **S01** Region overlay · **S02** Donut · **S03** Editor · **S04** Menu bar, pins, HUD · **S05** Settings › Presets. Free screens S06–S09 (Beautify, Measure, Live text, Secret detection) are on the v1 Core page, with Pro-only controls marked PRO. Paid screens are on the “Pro — post-v1” page (§14). Not yet drawn (build to the same patterns): History window, Settings › Donut, Settings › Shortcuts, Onboarding sheet, dark-mode variants.

### 9.1 Platform adaptations

| Element | Mac | Windows |
|---|---|---|
| Window controls | Traffic lights, top-left | Minimise / maximise / close, top-right |
| Font | System (SF Pro / SF Mono) | Segoe UI Variable / Cascadia Mono |
| Background material | Vibrancy | Mica (windows), Acrylic (HUD, donut) |
| App home | Menu bar icon (template glyph) | System tray icon |
| Modifier glyphs | ⌘ ⌥ ⇧ ⌃ | Ctrl Alt Shift Win |
| Settings | Sidebar window | Same layout, Windows title bar |
| Everything else | Shared: donut, overlay, editor, inspector, icons, colours, radii, spacing | Shared |

### 9.2 Default global shortcuts

| Action | Mac | Windows |
|---|---|---|
| Region | ⌘⇧2 | Ctrl+Shift+2 (option: Print Screen) |
| Repeat last region | ⌘⇧1 | Ctrl+Shift+1 |
| Window | ⌥⌘⇧2 | Ctrl+Alt+Shift+2 |
| Full screen | ⌃⌘⇧2 | Ctrl+Win+Shift+2 |
| Scrolling | ⌥⌘⇧S | Ctrl+Alt+Shift+S |
| Copy text from screen | ⌥⌘⇧O | Ctrl+Alt+Shift+O |
| History | ⌘Y (in-app) | Ctrl+H (in-app) |

All remappable; conflicts flagged. On Windows 11, using Print Screen requires turning off “Use the Print screen key to open screen capture” in Windows Settings; onboarding explains this.

---

## 10. Testing and quality

- Unit: filename pattern engine, preset encoding, obfuscation flattening (assert no original pixels in output region), counter renumbering, stitcher overlap detection.
- Snapshot tests for renderer output of each object type and effect (light/dark).
- UI tests (Playwright against the web UI + Tauri WebDriver): capture → donut → copy; editor tool shortcuts; settings persistence.
- Render parity: the web canvas preview and the Rust export renderer produce matching images for every object type (pixel-diff threshold).
- Manual matrix — Mac: multi-display, mixed Retina/non-Retina, Stage Manager, full-screen apps, Spaces, hot-plug. Windows: per-monitor DPI (100–200 %), mixed scaling, HDR displays, virtual desktops, ARM64.
- Performance benchmarks in CI for NFR-1 on Mac and Windows runners.

## 11. Success metrics (local-only, no telemetry)

GitHub stars, Homebrew + winget installs, issue resolution time, % of core feature baseline shipped, community plugin count. No in-app analytics.

## 12. Risks and open questions

| # | Item | Notes |
|---|---|---|
| R1 | Screen Recording permission friction and macOS periodic re-consent prompts | Clear onboarding + inline fix; document behaviour |
| R2 | Scrolling capture reliability across apps | Ship as “beta”; manual frame fallback |
| R3 | System ⌘⇧3/4/5 cannot be overridden while enabled | Guide user to disable in System Settings |
| R4 | Notarisation/Developer ID cost for an OSS project | Needs an Apple Developer account owner — **open question: who holds it?** |
| R5 | Web-view overlay too slow or flickers (NFR-1) | Pre-warm hidden overlay windows; spike in v0.0 with go/no-go; fallback: native overlay windows per platform drawn from Rust, keep web UI elsewhere |
| R6 | Preview vs export rendering differ | Single document model in Rust; render-parity tests |
| R7 | Mac feel through a web view | Platform tokens, native window effects, native menus/tray; review against S01–S05 designs on real Macs |
| R8 | Windows code-signing cost and SmartScreen reputation | Budget for a signing certificate or a cloud signing service; build reputation before launch |
| Q1 | Recording/GIF in scope post-v1? | Decide after v1.0 |

---

## 13. Build plan for a coding agent

Feed these as sequential tasks; each ends with passing tests and a runnable app on **both** macOS and Windows (CI matrix).

0. **Spike** — Tauri 2 + React scaffold; global shortcut; freeze all displays; pre-warmed transparent overlay windows; measure hotkey → overlay on both OSes; draw one arrow in web canvas and export via `tiny-skia`, compare. Go/no-go (R5).
1. **Scaffold** — Cargo workspace and crates from §8, tray/menu bar app, Settings window shell with sidebar, platform tokens (§9.1), onboarding with permission check (Mac).
2. **Capture + overlay** — region selection with handles, size label, loupe, modifiers, window mode, full screen, repeat last; write captures to History.
3. **Output** — Preset model + JSON persistence, filename pattern engine with tests, encoders, clipboard/save/save-as, HUD.
4. **Donut** — radial view, flick recogniser, key handling, configurable slots, edge-aware placement.
5. **Core + render** — document model, undo, Rust export renderer, obfuscation flattening tests, render-parity tests.
6. **Editor UI** — canvas, each tool from §6.4, inspector, zoom, status bar, Save split button.
7. **History + pins** — SQLite store, window, search, pin windows.
8. **v0.2 features** — OCR (`Ocr` trait, both OSes), secret detection, timed, effects, magnify/callout/eyedropper, share/open-in/print.
9. **v0.3 features** — Beautify, measure (AX / UI Automation), live text, scrolling capture, CLI, URL scheme, watch folders, `.shotsmith` format.
10. **Release** — Mac signing + notarisation, Windows signing + MSI, signed updater (opt-in), Homebrew + winget, Mac App Intents, accessibility audit with VoiceOver and Narrator, docs.

---

## 14. Shotsmith Pro — post-v1 paid features (open core)

**Status:** decided — open core, subscription via Stripe, accounts + cloud share links. Build none of this before v1.0 ships.

### 14.1 Principles

- The free app (§5–§6, MVP to v1.0) is complete on its own. Pro adds features; it never locks, nags inside, or degrades free ones.
- All image and text **processing** (redaction, OCR, translation, search, alt text) runs on the user’s computer. Images leave the computer only when the user explicitly shares or syncs them (PRO-12, PRO-13).
- The free app never asks for sign-in. Signing in is only offered when starting a trial or opening a Pro control.
- Pro files stay openable and exportable in the free app if a licence ends (Pro-only objects flatten to images).

### 14.2 Business model — open core (decided)

- **Public repo (MIT):** the whole free app, including the extension points Pro plugs into (protocols in `EditorCore`, `CaptureKit`, `History`).
- **Private repo (proprietary):** `Packages/Pro/` and the licence verifier. Official signed/notarised builds include Pro, locked until a licence is present.
- **Rules:** never commit Pro code to the public repo — anything published under MIT can be forked and reused. Self-builds from the public repo are the free app without Pro.
- **Licence:** account-based (PRO-11). The app checks the subscription online and caches a signed entitlement token; Pro keeps working offline for a 30-day grace period.
- **Contributions:** accept public contributions under MIT with a Developer Certificate of Origin (DCO) sign-off; consider a CLA only if you may relicense the core later.
- Get legal advice on the licence terms and EULA before launch; this PRD is not legal advice.

### 14.3 Features

| ID | Feature | Screen | Effort | Depends on |
|---|---|---|---|---|
| PRO-1 | Smart redaction — faces + auto-redact | S09 (PRO controls) | S–M | RED-1…4, Vision faces |
| PRO-2 | Element-aware snapping | Pro page | M | Accessibility API, Vision rectangles |
| PRO-3 | Step-guide recorder | Pro page | L | CaptureKit, PRO-2, PRO-1 |
| PRO-4 | Translate overlay | S08 (PRO controls) | S–M | LT-1, Translation framework (macOS 15+) |
| PRO-6 | Visual diff | Pro page | M | Core Image, History |
| PRO-7 | Inspect pro — contrast + capture with marks | S07 (PRO controls) | S | MEA-1/2 |
| PRO-8 | Search history by meaning | Pro page | M | History, OCR index, on-device embeddings |
| PRO-9 | Alt text & smart filenames | Pro page | S–M | Foundation Models framework (macOS 26 + Apple Intelligence), OCR fallback |
| PRO-10 | Style kits | Pro page | S | Presets, EditorCore styles; team sharing via PRO-11 |
| PRO-11 | Account, subscription & licence | Pro page: Upgrade & trial, Settings › Account | M | Backend (§14.6), Stripe |
| PRO-12 | Cloud share links | Pro page: Share link, Shared link web page | M | PRO-11, object storage |
| PRO-13 | Cloud library & sync (optional) | Not drawn | M–L | PRO-11, PRO-12 |

(PRO-5 Beautify moved to the free app as BTY-1…3. IDs are not reused.)

#### PRO-1 Smart redaction (S09, PRO controls)

- Adds face detection to the RED-1 scan.
- “Auto-redact with this preset”: runs the scan on every capture made with that preset and applies redactions before any destination runs.
- Bulk redaction across a step guide (PRO-3) or a History selection.
- **AC:** With auto-redact on, no unredacted file or clipboard image is ever produced for that preset (integration test).

#### PRO-2 Element-aware snapping

- In region mode, hovering highlights the UI element under the cursor (from the Accessibility tree, with Vision rectangle detection as a fallback for apps without accessibility data).
- Scroll to move up/down the element tree; breadcrumb shows the path. ⇧-click adds elements to a union selection. Padding presets 0/8/16/32 pt and `[ ]` keys.
- Holding ⌥ gives a free region temporarily.
- **AC:** Highlight follows the cursor at 60 fps; Accessibility permission requested only when first used, with a working fallback when denied.

#### PRO-3 Step-guide recorder

- Recording HUD (Add note, Pause, Finish). Each mouse click captures the screen, crops around the click (configurable), adds a numbered counter and a click ripple, and drafts a caption from the clicked control’s accessibility label (“Click **Export**”).
- Guide editor: reorder, delete, merge steps; edit captions; add headings and notes; per-step annotation using the normal editor.
- Options: auto-crop, auto-redact (PRO-1), show keystrokes.
- Export: Markdown + images folder, single-file HTML, PDF, and `.shotguide` (editable package).
- **AC:** A 20-step guide records with no dropped clicks; exports open correctly in a Markdown viewer and a browser; pausing stops all capture.

#### PRO-4 Translate overlay (S08, PRO controls)

- Translate action in the live-text popover; “Show translation overlay” draws translated text over the original region; optional “bake into export”; Copy translation.
- **AC:** Uses Apple’s Translation framework; language packs download from Apple on first use only with the user’s consent (the only network use, and it’s by the OS).

#### PRO-6 Visual diff

- Compare any two captures (select two in History, ⌘D). Modes: side by side, slider, onion skin, changes only.
- Auto-align (translation/scale), sensitivity, ignore anti-aliasing, mask dynamic areas. Lists changes as numbered regions, with text changes described where OCR can tell.
- Export the comparison as PNG or PDF.
- **AC:** Identical captures report 0 changes; a 1 px colour change is caught at max sensitivity; different-size captures are aligned or a clear message is shown.

#### PRO-7 Inspect pro (S07, PRO controls)

- Contrast checker: foreground/background ratio with WCAG AA/AAA (body and large text) pass/fail, and the nearest passing colour.
- “Capture with marks” bakes measurements into a capture as editable annotation objects.
- **AC:** Contrast ratios match the WCAG 2.x formula to one decimal place (unit-tested).

#### PRO-8 Search history by meaning

- One search field across History. Combines exact text (free HIS-2 search), dates parsed from natural language (“last week”), and similarity using on-device image/text embeddings.
- Shows how the query was understood (chips) and why each result matched. “Exact text only” switch.
- Index stored locally; size shown; rebuild in Settings › Advanced.
- **AC:** Results in under 300 ms for 10,000 captures; deleting a capture removes it from the index; no network calls.

#### PRO-9 Alt text & smart filenames

- On save, suggests a descriptive filename and two alternatives, plus alt text (short / medium / detailed).
- Options: write alt text into file metadata (PNG `iTXt` / XMP description), copy alt text with the image as rich clipboard content, always name files this way.
- Uses Apple’s on-device Foundation Models framework where available (macOS 26, Apple Intelligence enabled); otherwise falls back to OCR-based names (first heading + app).
- **AC:** Feature hides cleanly on unsupported Macs; generated filenames are sanitised and unique; nothing is sent off-device.

#### PRO-10 Style kits

- A kit = palette, text font, arrow style (straight / curved / hand-drawn), counter style (circle / square / letters), stroke width, watermark (image or text, position, opacity) and bundled presets.
- Kits export/import as a single `.shotkit` file (zip: `kit.json` + assets). “Lock kit when shared” prevents edits to imported kits. Can be deployed by Mac management tools via a known folder.
- Option to hide colours outside the kit in the editor.
- **AC:** Importing a kit on a fresh install reproduces identical annotation output (snapshot test).

#### PRO-11 Account, subscription & licence

- Sign in with Apple, or email magic link (no passwords). Account screen in Settings › Account: plan, trial days left, storage used, Manage billing (opens Stripe Customer Portal), Sign out, Delete account.
- Start trial from any PRO control or Settings: 14 days, full Pro, **no card required**; reminder 3 days and 1 day before it ends.
- Checkout via Stripe Checkout in the browser; return to the app via `shotsmith://` URL. Webhooks update entitlements server-side.
- App fetches a signed entitlement token (plan, expiry, storage quota) on launch and every 24 h; valid offline for 30 days. Expired/cancelled → Pro controls show the upgrade sheet; nothing already made is lost (§14.1).
- Up to 3 computers (Mac or Windows, any mix) per account; manage devices in Settings › Account.
- **AC:** Pro works offline for 30 days after the last check; cancelling in the portal removes Pro at period end; deleting an account deletes cloud data within 30 days.

#### PRO-12 Cloud share links

- New donut slot and editor button **Share link**: uploads the flattened image (never the editable document unless chosen), copies a short URL, shows a HUD.
- Link options: expiry (1 day / 7 days / **30 days — default** / never), password, allow download, view count. Defaults set in Settings › Sharing.
- Before upload: runs the RED-1 scan and warns if likely secrets are unredacted; strips metadata.
- Web viewer: fast single-image page (and step-guide pages for PRO-3), `noindex`, report-abuse link, owner can delete from the app or web.
- Links managed in History (shared badge, copy link, revoke).
- **AC:** Link copied within 2 s of upload start for a typical 1 MB PNG; revoking a link makes it return 404 within 60 s, including CDN caches.

#### PRO-13 Cloud library & sync (optional, later)

- Opt-in sync of History, presets and style kits between the user’s Macs; selective (e.g. pinned and shared only).
- Team space (later): shared style kits and a shared folder of captures.
- **AC:** Sync is off by default; turning it off offers to delete cloud copies.

### 14.4 Pro build order (after v1.0)

1. Private repo, Pro package scaffold, backend (§14.6), PRO-11 account + Stripe subscription + trial, upgrade sheet (shown only when a Pro control is clicked).
2. PRO-12 Cloud share links (main reason to sign in).
3. PRO-10 Style kits, PRO-7 Inspect pro, PRO-4 Translate overlay (small, build on free features).
4. PRO-1 Smart redaction.
5. PRO-2 Element-aware snapping.
6. PRO-3 Step-guide recorder (uses PRO-1 and PRO-2) — launch headline; guides shareable via PRO-12.
7. PRO-8 Search by meaning, PRO-6 Visual diff.
8. PRO-9 Alt text & smart filenames (gated on OS support).
9. PRO-13 Cloud library & sync, team spaces.

### 14.5 Pricing (proposed)

| Plan | Price (USD) | Includes |
|---|---|---|
| Free | $0 | Whole free app, no account |
| Pro monthly | **$3.99 / month** (confirmed) | All Pro features, 5 GB cloud storage for share links, 3 computers (Mac or Windows) |
| Pro yearly | **$36 / year** (= $3/month, confirmed) | Same; pushed as the default choice |
| Storage add-on | +$1.99/month for +50 GB · +$4.99/month for +250 GB | Billed on the same Stripe subscription (one charge) |
| Trial | 14 days, **no card** (confirmed) | Full Pro, 5 GB |
| Teams | 15% off per seat for 5+ seats (proposed; small discount as decided) | Same as Pro, plus shared style kits; one invoice; admin can add/remove seats |

**Why yearly should be the default — fees on small charges.** Using Stripe’s published US rates (2.9% + 30¢ per card charge, plus 0.7% for Stripe Billing; international cards +1.5%):

| Charge | Stripe fees (US card) | Fees (international card) | You keep (approx.) |
|---|---|---|---|
| $3.99 monthly | ≈ $0.44 (11%) | ≈ $0.50 (13%) | $3.49–$3.55 / month |
| $36 yearly | ≈ $1.60 (4.4%) | ≈ $2.14 (6%) | $33.86–$34.40 / year |

Fees differ by the country your Stripe account is in; check Stripe’s pricing page for your account. Storage cost basis: Cloudflare R2 standard storage is $0.015 per GB-month with free egress, so a full 5 GB costs ≈ $0.08/month and a full +50 GB add-on ≈ $0.75/month.

**Tax (decided): Stripe Tax.** It calculates and collects GST/VAT/US sales tax at checkout; you remain responsible for registering in each jurisdiction where you pass thresholds and for filing. Get accounting advice before launch.

### 14.6 Backend (proposed)

| Part | Recommendation | Notes |
|---|---|---|
| API + share-link viewer | Cloudflare Workers | Same platform as storage; global edge |
| Database | Cloudflare D1 (SQLite) | Accounts, devices, links, entitlements |
| Object storage | Cloudflare R2 | Pre-signed uploads straight from the app; free egress suits public links |
| Auth | Sign in with Apple + email magic link | No passwords to store |
| Payments | Stripe Checkout, Customer Portal, Billing webhooks | Trial without payment method; proration for storage add-ons |
| Email | Transactional provider (magic links, trial reminders, receipts from Stripe) | |

Alternative: Supabase (Postgres + auth + storage) if you prefer one managed service. Keep the backend in the private repo.

### 14.7 Pro network use (complete list)

Sign-in; entitlement check on launch and every 24 h; Stripe Checkout/Portal opened in the browser; uploads, link management and sync the user starts; Apple’s Translation language-pack downloads (by the OS). Nothing else. Show this list in Settings › Account › Privacy.

### 14.8 Legal and operational

- Privacy policy and terms (covering account data, cloud storage, deletion, data location); Australian Privacy Act and GDPR at minimum.
- Public share links mean hosting user content: needs an abuse-report and takedown process, rate limits, and storage/link quotas.
- Get legal advice on the EULA, privacy policy and hosting obligations; this PRD is not legal advice.

### 14.9 Decisions and open questions

Decided: $3.99/month and $36/year; storage add-ons as above; 14-day trial with no card; teams get a small per-seat discount; Stripe Tax; share links expire after 30 days by default.

| # | Open question |
|---|---|
| PQ1 | Exact team discount and minimum seats (proposed 15% for 5+). |

## Sources

- Filee releases (donut menu, presets, watch folders, CLI): https://github.com/KnifeLemon/Filee/releases
- Filee site: https://filee.sh
- Stripe fees (NerdWallet, updated Jul 2026): https://www.nerdwallet.com/business/software/learn/stripe-fees
- Cloudflare R2 pricing: https://developers.cloudflare.com/r2/pricing
