# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/) for its
tags (`vMAJOR.MINOR.PATCH`) - note that the version tag and the `version`
field in `Cargo.toml` are maintained independently; keep them in sync by hand
when cutting a release.

## [Unreleased]

Nothing yet.

## [1.0.0] - 2026-09-12

Initial release.

### Added

- Core conversion of PNG/JPG/BMP/GIF source images into Windows `.ico`
  files, always encoded as full 32-bit PNG (never the legacy, lower-quality
  BMP-with-reduced-palette format some tools fall back to).
- Alpha-aware ("premultiplied") resizing, so shrinking a transparent image
  never leaves a colored fringe around soft edges.
- `--chroma-key` background removal: flood-fill from the image border (so
  an identically-colored spot elsewhere in the artwork is left untouched),
  with a soft, anti-aliased transition at the edge instead of a hard cutout.
  `--tolerance` to tune the color match, `--seed` for background regions
  enclosed by a frame/ring that the border-based flood-fill can't reach on
  its own.
- `--padding` to leave a transparent margin around the artwork instead of
  filling the canvas edge-to-edge.
- `--sizes` for a custom size list, plus `--preset windows/favicon/minimal`
  for common size sets.
- `--output-format icns` for macOS icon export (Apple's full recommended
  size set, including 2x "Retina" variants), with an automatic
  platform-based default (`icns` on macOS, `ico` everywhere else) that can
  be overridden explicitly either way.
- A warning when a requested size exceeds the source image's resolution
  (upscaling can't add detail that isn't there).
- `--merge` to combine the icon entries of multiple existing `.ico` files
  into one.
- `--extract` to pull every size out of an existing `.ico` as separate
  PNGs.
- `--select` (with `--index` and `--combine`) to pull specific size(s) out
  of an existing `.ico` as standalone `.ico` file(s).
- `--inspect` to report on an existing `.ico`'s contents (sizes, color
  depth, PNG vs. legacy BMP encoding) or, given a plain source image
  instead, which standard sizes it can produce natively versus which would
  need upscaling.
- `--force` overwrite protection (refuses to clobber existing output
  unless explicitly told to) and `--delete-source` (removes the source
  file(s) after a successful run, with a safety net against deleting a
  source that turns out to also be the output path).
- Hardening: fixed a crash caused by slicing a `--chroma-key` value at an
  invalid UTF-8 byte boundary; verified graceful handling of malformed/
  corrupt `.ico` files and of oversized ("decompression bomb") source
  images; `cargo clippy` clean; `cargo audit` clean; property-based tests
  (`cargo test`, via `proptest`) covering the command-line input-parsing
  functions against arbitrary/adversarial input.
- Split into modules (`cli`, `chroma_key`, `resize`, `icns`, `ico_ops`,
  `util`) instead of one large file.
- MIT license.
- GitHub Actions workflow: native builds on Windows, macOS and Linux on
  every push/PR; an additional macOS job that generates a real `.icns`
  file and verifies it with Apple's own `iconutil`; and a release job
  that, on pushing a `vX.Y.Z` tag, publishes a GitHub Release with all
  three platform binaries attached.