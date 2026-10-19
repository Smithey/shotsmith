# Roadmap

The short checklist is in the README. This file is the build order from the PRD, with requirement IDs. The full spec is [PRD.md](PRD.md).

## v0.1 (this build)

Shell, region / window / full screen / repeat last, loupe, donut, MVP editor tools, clipboard and save, presets, history, settings, onboarding.

Also timer and pins, because the menu, donut, and history actions in PRD sections 6.1 and 6.3 need them.

Requirement ids: APP-1 to APP-5, CAP-1 to CAP-10, CAP-13, CAP-14, DON-1 to DON-8 (slots limited to actions this version finishes), ED-1 to ED-4, ED-6, ED-8, ED-10 for the MVP tools, PRE-1 to PRE-5, HIS-1, HIS-2, PIN-1, PIN-2, DST-2, ONB-1, ONB-2.

Capture uses `xcap` behind the `Capturer` helpers. ScreenCaptureKit and Windows Graphics Capture can replace that backend later.

## v0.2

Copy text (OCR trait, both operating systems), secret detection and manual redaction (S09, RED-1 to RED-4), effects, magnify, callout, eyedropper, share sheet, Open in, print.

## v0.3

Beautify (S06, BTY-1 to BTY-3), measure (S07, MEA-1 and MEA-2), live text (S08, LT-1 to LT-3), scrolling capture (CAP-11), CLI (AUT-1), URL scheme (AUT-2), watch folders (AUT-4), object library (ED-12).

## v1.0

Plugin destinations (DST-3), localisation, signed updater (opt-in), Homebrew and winget, Mac App Intents (AUT-3), accessibility audit, docs site.

## Pro, after v1.0

Only in `shotsmith-pro`, in this order:

1. Account, subscription, and licence (PRO-11)
2. Cloud share links (PRO-12)
3. Style kits (PRO-10), inspect pro (PRO-7), translate overlay (PRO-4)
4. Smart redaction (PRO-1)
5. Element-aware snapping (PRO-2)
6. Step-guide recorder (PRO-3)
7. Search by meaning (PRO-8), visual diff (PRO-6)
8. Alt text and smart filenames (PRO-9)
9. Cloud library and sync (PRO-13)
