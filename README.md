# img2ico

A command-line tool that converts any image into a Windows `.ico` file (or a macOS `.icns` file), with proper transparency support, background removal, and a few extra tools for working with existing icon files.

Built in Rust: a single, dependency-free binary — no runtime to install, nothing to configure.

> Developed in an extended pair-programming session with [Claude Sonnet 5](https://www.anthropic.com/claude) (Anthropic) — every feature, fix, and piece of documentation in this repo went through iterative review and testing during that process.

## Features

- Convert PNG/JPG/BMP/GIF to `.ico`, with full 32-bit color + alpha channel (never a reduced-color-depth legacy format)
- Remove a solid background color ("chroma key") with a soft, anti-aliased edge instead of a hard cutout
- Export macOS `.icns` files from the same source image (chosen automatically as the default when running on macOS)
- Merge multiple `.ico` files into one
- Extract every size out of an existing `.ico` as separate PNGs
- Pull out one or more specific sizes from an existing `.ico` (by index) as standalone `.ico` files
- Inspect an existing `.ico` file's contents (sizes, color depth, format, common problems)
- Built-in overwrite protection and optional source-file cleanup, both opt-in

## Platform Support

img2ico is pure, platform-neutral Rust — no OS-specific code or dependencies anywhere in it. It builds and runs identically on **Windows, macOS, and Linux** with the exact same `cargo build --release` command; there's no separate "Windows version" or "Mac version" of the source.

- **Windows:** an unsigned `.exe` will trigger a SmartScreen warning on first run ("Windows protected your PC"). This is expected for a binary without a paid code-signing certificate — click "More info" → "Run anyway".
- **macOS:** similarly, Gatekeeper may refuse to open an unsigned/non-notarized binary downloaded from the internet. Allow it once via System Settings → Privacy & Security, or build it locally with `cargo build --release` instead of downloading a pre-built binary (a binary you compile yourself isn't subject to this check).

## Installation

You need a working Rust toolchain (install via [rustup](https://rustup.rs) if you don't have one).

```
git clone <this-repo>
cd img2ico
cargo build --release
```

The compiled binary will be at `target/release/img2ico` (or `img2ico.exe` on Windows).

> **Note:** `Cargo.toml` pins a few dependency versions with `=`. This isn't required on a normal, up-to-date Rust installation — those pins exist to work around an unrelated, older build environment used while developing this tool. If you want the newest compatible versions instead, remove the `=` in front of the pinned version numbers and run `cargo update`.

## Quick Start

```
# Windows (cmd.exe or PowerShell)
img2ico.exe logo.png

# macOS / Linux
./img2ico logo.png
```

That's it — no other arguments needed. This creates `logo.ico` right next to `logo.png`, with a sensible default set of sizes (16, 32, 48, 64, 128, 256).

> **A note on the examples below:** every other example in this document uses the short form `img2ico` for readability. On Windows that means `img2ico.exe` (or just `img2ico` if you're in the same folder as it — Windows resolves the `.exe` automatically); on macOS/Linux it means `./img2ico` if you're running the freshly built binary directly from `target/release/`, or plain `img2ico` if you've moved it somewhere on your `PATH` (e.g. `/usr/local/bin`). The flags themselves — `-o`, `-c`, `--preset`, and so on — are identical on all three platforms; only how you invoke the binary itself differs.

## Command Reference

```
Usage: img2ico [OPTIONS] <INPUT>...
```

| Flag | Short | Takes a value | Purpose |
|---|---|---|---|
| `--output` | `-o` | path | Where to write the result (file or directory, depending on mode) |
| `--sizes` | `-s` | comma list | Which icon sizes to generate |
| `--preset` | | `windows` / `favicon` / `minimal` | A predefined size set, instead of `--sizes` |
| `--chroma-key` | `-c` | hex color | Remove this background color |
| `--tolerance` | `-t` | 0–100 | How strictly `--chroma-key` matches colors |
| `--seed` | | `x,y` (repeatable) | Extra starting point(s) for background removal |
| `--padding` | | 0–100 | Transparent margin around the artwork |
| `--output-format` | | `ico` / `icns` | Force a specific icon container format |
| `--merge` | | | Combine several `.ico` files into one |
| `--extract` | | | Pull every size out of an `.ico` as PNGs |
| `--select` | | | Pull specific size(s) out of an `.ico` as `.ico` file(s) |
| `--index` | | comma list | Which size(s) `--select` should pull out |
| `--combine` | | | With `--select` + multiple `--index`: one file instead of several |
| `--force` | `-f` | | Allow overwriting existing output |
| `--delete-source` | | | Delete the input file(s) after a successful run |
| `--inspect` | | | Print a report about `.ico` file(s) or source image(s) |
| `--help` | `-h` | | Full built-in help text |

`--merge`, `--inspect`, `--extract`, `--select` and `--output-format` are **mutually exclusive** — each one changes what the tool does with its input, and only one mode can be active per run. Note that img2ico picks `icns` automatically on macOS even without an explicit `--output-format` — see [section 3.0](#30-automatic-per-platform-default-new) — so this exclusivity rule is checked against what you actually typed, not that automatic default: running `--merge` on a Mac without ever mentioning `--output-format` works completely normally. Everything else (`--sizes`/`--preset`, `--chroma-key`/`--tolerance`/`--seed`, `--padding`, `--force`, `--delete-source`) can be freely combined with each other and with whichever single mode you're using, as shown below.

---

## 1. Basic conversion

### 1.1 Simplest possible use

```
img2ico logo.png
```

No flags at all. Converts `logo.png` into `logo.ico` in the same folder, using the default sizes (16, 32, 48, 64, 128, 256). This also works if you drag-and-drop an image file directly onto the compiled `.exe` on Windows.

### 1.2 Custom output path

```
img2ico logo.png -o build/icons/app.ico
```

Writes the result to a specific path instead of next to the input file. The target folder must already exist.

### 1.3 Custom sizes

```
img2ico logo.png -s 16,32,256
```

Only generates exactly the sizes you list, instead of the default six. Useful when you know exactly which sizes you need and want a smaller file.

### 1.4 Using a preset instead of listing sizes

```
img2ico logo.png --preset windows
```

Generates Microsoft's full recommended set (16, 20, 24, 32, 40, 48, 64, 96, 128, 256) so Windows never has to stretch a nearby size at any DPI scaling level. `--preset favicon` (16, 32, 48) and `--preset minimal` (16, 32) are also available. `--preset` completely replaces `--sizes` when both are given — `-s` is simply ignored in that case.

### 1.5 Adding padding around the artwork

```
img2ico logo.png --padding 15
```

Leaves roughly a 15% transparent margin on every side instead of filling the canvas edge-to-edge. Useful when your source artwork already touches its own edges and looks cramped as a small icon.

### 1.6 Preset + padding together

```
img2ico logo.png --preset favicon --padding 10
```

Any two independent options like these combine freely: use the favicon size set, and add a 10% margin to all of them.

### 1.7 Automatic warning when upscaling a small source image

```
img2ico small-logo.png --preset windows
```

If your source image is smaller than one or more of the requested sizes, img2ico still produces a valid icon — it just can't invent detail that isn't in the source. You'll see a warning like this on stderr rather than a silent, softer-than-expected result:

```
Warning: the source image is 64x64 pixels, smaller than [96, 128, 256] - those sizes will be upscaled and may look soft or blurry rather than sharp. For crisp results at every size, use a higher-resolution source image.
```

This is not an error and doesn't stop the conversion — it's a heads-up so a blurry large icon doesn't come as a surprise. Use `--inspect` on your source image beforehand (see [section 8.3](#83-a-source-image-before-converting-it)) if you'd like to check this before running the conversion at all.

---

## 2. Removing a background color (chroma key)

### 2.1 Basic background removal

```
img2ico logo.png -c 00C800 -o icon.ico
```

Finds a connected region of the color `#00C800` starting from the image's border and makes it transparent, with a soft anti-aliased edge rather than a hard, jagged cutout.

### 2.2 Adjusting tolerance

```
img2ico logo.png -c 00C800 -t 30
```

Higher tolerance (default is `20`) catches a wider range of similar shades as "background" — useful for backgrounds with slight gradients, JPEG compression artifacts, or anti-aliasing. Too high a value risks eating into the actual artwork.

### 2.3 Reaching a background color that's enclosed by the artwork

```
img2ico logo.png -c FFFFFF --seed 128,64
```

Normally, background removal only starts from the image's outer border. If your background color is also enclosed *inside* the artwork (e.g. a circular badge with a colored ring, and the same background color visible again inside that ring), the border-based search can't reach it. `--seed x,y` gives it an additional starting point — use pixel coordinates from your **original** image (e.g. by hovering the cursor in an image editor).

### 2.4 Multiple seed points

```
img2ico logo.png -c FFFFFF --seed 128,64 --seed 200,300
```

`--seed` can be given more than once, for multiple separate enclosed areas.

### 2.5 Chroma key + custom tolerance + seed, all together

```
img2ico logo.png -c FFFFFF -t 25 --seed 128,64 --seed 200,300
```

All chroma-key-related options combine freely with each other.

### 2.6 Chroma key + sizes/preset

```
img2ico logo.png -c 00C800 --preset windows
```

```
img2ico logo.png -c 00C800 -s 16,32,64,256
```

Background removal is entirely independent of which sizes get generated — combine it with either `-s` or `--preset`.

### 2.7 Chroma key + padding

```
img2ico logo.png -c 00C800 --padding 10
```

Padding is added *after* the background has already been made transparent, so you get a transparent margin around whatever's left of the artwork.

### 2.8 The full combination

```
img2ico logo.png -c 00C800 -t 25 --seed 128,64 --preset windows --padding 10 -o icon.ico --force
```

Every one of the options above can be combined into a single command.

---

## 3. macOS icons (`--output-format`)

### 3.0 Automatic per-platform default

```
img2ico logo.png
```

If you don't pass `--output-format` explicitly, img2ico picks the output format automatically based on the platform the *binary was built for*: **`icns` on macOS**, **`ico` everywhere else**. This means the exact same command produces the right format on every platform without you having to remember a flag — a Mac build of img2ico writes `logo.icns` here, while a Windows or Linux build writes `logo.ico`.

### 3.1 Basic `.icns` export (explicit)

```
img2ico logo.png --output-format icns
```

Creates `logo.icns` with Apple's full recommended size set (16, 32, 64, 128, 256, 512, 1024 pixels — including the 2x "Retina" variants), completely independent of `--sizes`/`--preset` (which are ignored whenever the output is `icns`).

### 3.2 Forcing `.ico` output on a Mac

```
img2ico logo.png --output-format ico
```

If you're running a macOS build but specifically want a Windows-compatible `.ico` file (e.g. you're preparing assets for a cross-platform app), this overrides the automatic macOS default.

### 3.3 `.icns` with a custom output path

```
img2ico logo.png --output-format icns -o build/AppIcon.icns
```

### 3.4 `.icns` with background removal

```
img2ico logo.png --output-format icns -c FFFFFF -t 20
```

`--chroma-key`, `--tolerance`, `--seed` and `--padding` all still apply whenever the output is `icns`, since they affect the image content itself rather than the output container format.

### 3.5 `.icns` with the full chroma-key + padding combination

```
img2ico logo.png --output-format icns -c FFFFFF -t 25 --seed 128,64 --padding 10
```

> **Note:** the `.icns` container format was verified byte-by-byte against the public specification and cross-checked with an independent parser (and, via this project's GitHub Actions workflow, with Apple's own `iconutil` on a real macOS runner). A real-world test on your own Mac is still worth doing before relying on it for anything important.

---

## 4. Overwrite protection and cleanup

These two flags work the same way across every mode described in this document.

### 4.1 Refusing to overwrite (the default)

```
img2ico logo.png -o icon.ico
img2ico logo.png -o icon.ico
```

Running the same command twice fails the second time with a clear error (`'icon.ico' already exists. Use --force to overwrite it.`) instead of silently destroying the first result.

### 4.2 Explicitly allowing overwrite

```
img2ico logo.png -o icon.ico --force
```

### 4.3 Deleting the source file after a successful conversion

```
img2ico logo.png -o icon.ico --delete-source
```

Only deletes `logo.png` **after** `icon.ico` has been written completely — if anything fails first, nothing gets deleted. If the source and output paths happen to resolve to the same file, that file is *not* deleted (with a warning instead), since that would delete the very result you just created.

### 4.4 Both together

```
img2ico logo.png -o icon.ico --force --delete-source
```

---

## 5. Merging existing `.ico` files (`--merge`)

### 5.1 Basic merge

```
img2ico --merge small.ico large.ico -o combined.ico
```

Combines the icon sizes from both files into one. Needs at least two input files, and `-o` is required (there's no single obvious output name to derive when merging multiple sources). If the same size exists in more than one input file, the first occurrence wins and the rest are skipped with a warning. Every entry is re-encoded as PNG on the way in, so the result has consistent full-quality output even if one of the source files used an older, lower-quality format.

### 5.2 Merge with overwrite protection / force

```
img2ico --merge small.ico large.ico -o combined.ico --force
```

### 5.3 Merge and clean up the sources afterward

```
img2ico --merge small.ico large.ico -o combined.ico --delete-source
```

Deletes `small.ico` and `large.ico` once `combined.ico` has been written successfully.

---

## 6. Extracting every size as PNGs (`--extract`)

### 6.1 Basic extraction

```
img2ico --extract icon.ico
```

Pulls every size out of `icon.ico` and saves each as a separate PNG (e.g. `icon_16x16.png`, `icon_32x32.png`, ...) into a new folder named `icon_extracted/` next to the input file.

### 6.2 Custom output directory

```
img2ico --extract icon.ico -o pngs/
```

In this mode, `-o` names a **directory**, not a file — it's created automatically if it doesn't exist.

### 6.3 Extraction with overwrite protection / force

```
img2ico --extract icon.ico -o pngs/ --force
```

Without `--force`, if *any* of the target PNG files already exists, the whole command is refused before anything is written (so you never end up with a half-extracted folder).

### 6.4 Extract and delete the source `.ico`

```
img2ico --extract icon.ico --delete-source
```

---

## 7. Pulling out specific sizes as `.ico` files (`--select`)

This is the ICO-to-ICO counterpart of `--extract`: instead of exporting every size as a PNG, it re-exports one or more chosen sizes as standalone `.ico` file(s). Use `--inspect` first to see the index of each size.

### 7.1 Default: just the first icon

```
img2ico --select icon.ico
```

With no `--index` given, this pulls out index `0` (the first entry in the file) into a single file, e.g. `icon_16x16.ico`.

### 7.2 A specific single size

```
img2ico --inspect icon.ico
img2ico --select icon.ico --index 3
```

Run `--inspect` first to see which `[index]` corresponds to which size, then pull out exactly that one.

### 7.3 Multiple sizes, as separate files (the default for multiple indices)

```
img2ico --select icon.ico --index 0,3,5
```

Creates one `.ico` file per selected size, into a folder named `icon_selected/` (same shape as `--extract`, just `.ico` output instead of `.png`).

### 7.4 Multiple sizes, combined into one file

```
img2ico --select icon.ico --index 0,3,5 --combine
```

Instead of separate files, bundles the three selected sizes into a single multi-size `icon_selected.ico`.

### 7.5 Select with a custom output path

```
img2ico --select icon.ico --index 3 -o app-icon.ico
```
```
img2ico --select icon.ico --index 0,3,5 -o chosen/
```

`-o` means a file path when exactly one size ends up being written (a single index, or `--combine`), and a directory when several separate files are written — same distinction as `--extract`.

### 7.6 Select with overwrite protection / force

```
img2ico --select icon.ico --index 0,3,5 --combine -o app-icon.ico --force
```

### 7.7 Select and delete the source `.ico`

```
img2ico --select icon.ico --index 0 --delete-source
```

---

## 8. Inspecting files (`--inspect`)

### 8.1 A single `.ico` file

```
img2ico --inspect icon.ico
```

Prints every size, its index, color depth, whether it's stored as PNG or the legacy BMP format, and warnings about common problems (reduced color depth, missing standard Windows DPI sizes). Doesn't write or change anything.

### 8.2 Several files at once

```
img2ico --inspect one.ico two.ico three.ico
```

Prints a separate report for each file in turn.

### 8.3 A source image, before converting it

```
img2ico --inspect logo.png
```

`--inspect` also accepts a regular source image (anything the normal conversion mode accepts) instead of an existing `.ico`. Rather than listing icon entries, it prints the image's resolution and, for both the Windows and macOS standard size sets, which sizes it can produce **natively** versus which would need **upscaling**:

```
logo.png (source image, 64x64):
  Windows sizes this image covers natively: [16, 20, 24, 32, 40, 48, 64]
  Windows sizes that would need upscaling (may look soft/blurry): [96, 128, 256]
  macOS (.icns) sizes this image covers natively: [16, 32, 64]
  macOS (.icns) sizes that would need upscaling (may look soft/blurry): [128, 256, 512, 1024]
```

This is a good way to check *before* running a conversion whether your source image is actually large enough for the sizes you want — see [section 9.4](#94-checking-a-source-image-before-committing-to-a-large-icon) for a worked example, and the note below on the upscaling warning you'd otherwise only see at conversion time.

### 8.4 Mixing `.ico` files and source images in one call

```
img2ico --inspect logo.png icon.ico
```

Each file is inspected using whichever report fits it — img2ico figures out which is which automatically; you don't need to tell it.

---

## 9. Real-world combined examples

A few end-to-end examples showing several independent options combined in ways you'd actually use in practice.

**Turn a logo with a plain background into a ready-to-ship Windows icon, overwriting any previous attempt:**
```
img2ico logo.png -c FFFFFF -t 20 --preset windows --padding 8 -o dist/app.ico --force
```

**Same source image, producing both a Windows icon and a macOS icon in one go (two separate commands, since each run has one output format):**
```
img2ico logo.png -c FFFFFF --preset windows -o dist/app.ico
img2ico logo.png -c FFFFFF --output-format icns -o dist/app.icns
```

**Pull just the two largest sizes out of a big vendor-supplied `.ico`, and clean up afterward:**
```
img2ico --inspect vendor-icon.ico
img2ico --select vendor-icon.ico --index 8,9 --combine -o our-icon.ico --delete-source
```

**Batch-combine two teams' separately-generated icons into one shared file:**
```
img2ico --merge team-a.ico team-b.ico -o shared.ico --force --delete-source
```

### 9.4 Checking a source image before committing to a large icon

```
img2ico --inspect logo.png
```
```
logo.png (source image, 64x64):
  Windows sizes this image covers natively: [16, 20, 24, 32, 40, 48, 64]
  Windows sizes that would need upscaling (may look soft/blurry): [96, 128, 256]
  macOS (.icns) sizes this image covers natively: [16, 32, 64]
  macOS (.icns) sizes that would need upscaling (may look soft/blurry): [128, 256, 512, 1024]
  tip: for consistently sharp icons at every common size, a source of at least 256x256 (1024x1024 if you also need .icns) is recommended.
```

Seeing this, you might decide to get a higher-resolution version of `logo.png` before generating a `.icns` (which always includes a 1024×1024 size) — or just proceed anyway and accept the upscaling warning, if a slightly soft large icon is fine for your use case:
```
img2ico logo.png --output-format icns --force
```

---

## A note on quality and safety

- Every icon size this tool writes is 32-bit PNG-encoded with a full alpha channel — never the older, lower-quality BMP-with-reduced-palette format that some other tools (and older Windows conventions) fall back to.
- Resizing is alpha-aware (premultiplied), so shrinking a transparent image doesn't leave a colored fringe around soft edges.
- This tool has been checked with `cargo clippy` (clean), `cargo audit` (no known vulnerabilities in its dependencies at the time of writing), and includes automated property-based tests (`cargo test`) covering its input-parsing logic against malformed/adversarial input.

## License

MIT — see [LICENSE](LICENSE) for the full text.