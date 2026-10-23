# Contributing

Bug reports, translations, docs, and code are welcome. Anyone can open a pull request. `main` accepts changes through a pull request, and it needs a review from [@Smithey](https://github.com/Smithey) before it merges. This repository is the free Shotsmith app under the MIT licence.

Commits need a [Developer Certificate of Origin](https://developercertificate.org/) sign-off:

```sh
git commit -s
```

That certifies you can contribute the change under the project licence.

## What belongs here

Capture, the editor, presets, history, and the other free features in [docs/ROADMAP.md](docs/ROADMAP.md). Extension traits live in `crates/extensions`. The free app ships empty implementations.

## What does not belong here

Shotsmith Pro (accounts, billing, share links, and the other paid features) lives in the private `shotsmith-pro` repository. Do not open a pull request that adds that code. Anything committed here is MIT and can be forked.

## Checks

```sh
cargo test --workspace
pnpm typecheck
```
