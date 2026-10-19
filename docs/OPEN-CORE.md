# Open core

This repository is the public Shotsmith app. It is MIT licensed and builds the free app by itself.

`shotsmith-pro` is a second git repository stored beside this one in local checkouts, under `shotsmith-pro/`. That folder is gitignored here. It has its own history and must not be pushed to the public remote. It has no remote until a private host exists.

Section 14.2 of the PRD describes `Packages/Pro/`. That means the contents of the private repository, not a normal directory in this tree.

## How the two trees meet

`crates/extensions` defines the only seam:

- `EditorCore` covers editor objects, styles, and the redaction pipeline.
- `CaptureKit` covers post-capture hooks.
- `HistoryProvider` covers extra search and sync providers.

Each trait has a no-op install (`install_builtin`). The free binary does not load a plugin and does not phone home to discover one.

The private crate depends on these traits by path (`../../crates/extensions`). This workspace has no path dependency back. Official signed builds, later, check out both repositories and apply a Cargo `[patch]` from the private release workflow so `shotsmith-extensions` resolves to the private crate. A clone of only this repository still builds the free app.

## Contributions

Public contributions are MIT with a [Developer Certificate of Origin](https://developercertificate.org/) sign-off (`git commit -s`). There is no CLA until the owner decides they may relicense the core.
