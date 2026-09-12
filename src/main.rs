// ============================================================================
// img2ico - converts any image format into a Windows ICO file (or a macOS
// ICNS file) with support for transparency (alpha channel).
//
// This file only contains the top-level orchestration (main() and run()).
// Everything else lives in its own module, grouped by feature:
//   - cli.rs         the Args struct and the two --preset/--output-format
//                     value enums (everything clap needs)
//   - chroma_key.rs   --chroma-key hex parsing and the flood-fill algorithm
//   - resize.rs       alpha-aware resizing and square-icon construction
//   - icns.rs         the macOS .icns container format
//   - ico_ops.rs      --merge, --inspect, --extract, --select (everything
//                     that reads/writes EXISTING .ico files)
//   - util.rs         small general-purpose helpers (--seed parsing,
//                     overwrite protection, --delete-source)
// ============================================================================

mod chroma_key;
mod cli;
mod icns;
mod ico_ops;
mod resize;
mod util;

use chroma_key::{apply_chroma_key, parse_hex_color};
use clap::Parser;
use cli::{Args, OutputFormat};
use icns::{write_icns, ICNS_SIZES};
use ico_ops::{extract_icons, inspect_icons, merge_icons, parse_indices, select_icons};
use resize::{has_transparency, make_square_icon, warn_about_upscaling};
use util::{check_overwrite, delete_source_files, parse_seed};

fn main() {
    // The actual work lives in run(). Reason for this split: run() returns
    // a Result<(), String>, which lets us use the "?" operator everywhere
    // inside it (see explanation below) - this saves us from repeating the
    // same "print error + exit" block at every single call site. main()
    // itself then only has to handle a potential error ONCE, centrally in
    // one place, printing it and exiting the program with exit code 1.
    if let Err(message) = run() {
        eprintln!("{message}");
        std::process::exit(1);
    }
}

/// Contains the complete program logic.
///
/// Rust note on the "?" operator: when a "?" follows an expression that
/// returns a Result, the following happens automatically:
///   - Ok(value) -> gets unwrapped to "value", the program continues normally
///   - Err(e)    -> the function is exited IMMEDIATELY, returning Err(e)
///
/// This is shorthand for exactly the pattern we previously rebuilt by hand
/// everywhere with unwrap_or_else(|e| { eprintln!(...); exit(1); }) -
/// except the error is now "passed through" up to main(), instead of
/// ending the whole program immediately at every single call site. This
/// requires the function itself to return a Result (hence
/// "-> Result<(), String>" right below).
fn run() -> Result<(), String> {
    // Args::parse() reads argv, validates it against the struct in cli.rs,
    // and automatically exits the program with an error message if, say,
    // the input file is missing.
    let args = Args::parse();

    // --merge, --inspect, --extract and --select are mutually exclusive -
    // each one replaces the normal image-conversion pipeline with
    // something else entirely. --output-format is different: it's still
    // part of the normal conversion pipeline (just picking a different
    // output container at the end), so they only conflict with the other
    // four modes, not with "no flag at all". Note this check deliberately
    // looks at the RAW args.output_format (whether the user typed
    // --output-format at all), not the platform-based default computed
    // further down - a Mac user running --merge without ever typing
    // --output-format shouldn't trip this just because icns happens to be
    // their platform's default format.
    let mode_count = [args.merge, args.inspect, args.extract, args.select]
        .iter()
        .filter(|&&on| on)
        .count();
    if mode_count > 1 || (mode_count == 1 && args.output_format.is_some()) {
        return Err(
            "--merge, --inspect, --extract, --select and --output-format are mutually exclusive - please use only one at a time."
                .to_string(),
        );
    }

    if args.inspect {
        return inspect_icons(&args.input);
    }

    if args.extract {
        let [input_path] = args.input.as_slice() else {
            return Err(format!(
                "--extract expects exactly one input .ico file, got {}.",
                args.input.len()
            ));
        };
        extract_icons(input_path, args.output.as_deref(), args.force)?;
        if args.delete_source {
            // The output here is a directory, not a file, so it can never
            // collide with the (file) source path - no output_path needed
            // for the same-file safety check.
            delete_source_files(std::slice::from_ref(input_path), None);
        }
        return Ok(());
    }

    if args.select {
        let [input_path] = args.input.as_slice() else {
            return Err(format!(
                "--select expects exactly one input .ico file, got {}.",
                args.input.len()
            ));
        };
        let indices = parse_indices(&args.index)?;
        let written_file = select_icons(
            input_path,
            &indices,
            args.combine,
            args.output.as_deref(),
            args.force,
        )?;
        if args.delete_source {
            // written_file is Some(path) when a single combined file was
            // written (protect against deleting it if it happens to equal
            // the source), or None when a whole directory of separate
            // files was written instead (can't collide with a file path).
            delete_source_files(std::slice::from_ref(input_path), written_file.as_deref());
        }
        return Ok(());
    }

    // Merge mode branches off immediately: it has its own, much simpler
    // pipeline (no resizing, no chroma-key) and doesn't touch any of the
    // image-conversion logic below at all.
    if args.merge {
        // Merging several files into one has no natural "obvious" output
        // name the way single-image conversion does (input name + .ico),
        // so we require an explicit -o here instead of guessing.
        let output_path = args.output.ok_or_else(|| {
            "Merge mode requires an explicit output path (-o/--output).".to_string()
        })?;
        merge_icons(&args.input, &output_path, args.force)?;
        if args.delete_source {
            delete_source_files(&args.input, Some(&output_path));
        }
        return Ok(());
    }

    // Normal (non-merge) mode expects exactly one input image.
    let [input_path] = args.input.as_slice() else {
        return Err(format!(
            "Expected exactly one input image, got {} (use --merge to combine multiple existing .ico files instead).",
            args.input.len()
        ));
    };

    // Whether to produce .icns or .ico. If the user gave --output-format
    // explicitly, that wins outright. Otherwise, fall back to a
    // platform-based default: .icns on macOS, .ico everywhere else - this
    // is what most people building on a given platform actually want,
    // without having to remember to pass --output-format every time on a
    // Mac.
    //
    // cfg!(target_os = "macos") is a compile-time check (baked into the
    // binary depending on what platform it was BUILT for), not a runtime
    // one - which is exactly what we want here: a binary built natively
    // on macOS should default to .icns, regardless of where it's later
    // copied to and run from.
    let use_icns = match args.output_format {
        Some(OutputFormat::Icns) => true,
        Some(OutputFormat::Ico) => false,
        None => cfg!(target_os = "macos"),
    };

    // Determine the target path: either given explicitly, or the input
    // name with a ".ico" (or ".icns", depending on use_icns) extension.
    // Not an error case, but a default value - hence still
    // unwrap_or_else() instead of "?".
    let output_path = args.output.unwrap_or_else(|| {
        let mut p = input_path.clone();
        p.set_extension(if use_icns { "icns" } else { "ico" });
        p
    });

    // Fail fast, before doing any actual work (loading/resizing the
    // image), if the resolved output already exists and --force wasn't
    // given. This one check covers both the normal ICO path and --icns,
    // since they share this same output_path.
    check_overwrite(&output_path, args.force)?;

    // Turn the sizes string ("16,32,48") into a list of numbers. Each
    // individual size can fail (not a valid number) - so the map() closure
    // here returns a Result instead of a plain u32.
    // collect::<Result<Vec<u32>, String>>() then gathers everything into
    // ONE Result: as soon as any size is invalid, collect() immediately
    // returns that one Err (and stops), instead of still processing the
    // rest of the list.
    //
    // Only relevant for the normal ICO path - icns output uses its own fixed
    // set of sizes instead (see ICNS_SIZES) - but parsing it here anyway
    // is cheap and harmless even when it ends up unused.
    //
    // --preset short-circuits all of this: it replaces whatever --sizes
    // says with a predefined list outright, so --sizes is simply never
    // parsed/looked at in that case.
    let sizes: Vec<u32> = if let Some(preset) = args.preset {
        preset.sizes().to_vec()
    } else {
        args.sizes
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<u32>()
                    .map_err(|_| format!("Invalid size: '{s}' (must be a positive number)"))
            })
            .collect::<Result<Vec<u32>, String>>()?
    };

    if !use_icns && sizes.is_empty() {
        return Err("At least one size must be given.".to_string());
    }

    // Load the input image. image::open detects the format automatically
    // from the file header (not just the file extension).
    let img = image::open(input_path).map_err(|e| format!("Could not read input file: {e}"))?;

    // If --chroma-key was given: apply it once to the original image at
    // full resolution (not once per icon size), so the flood-fill
    // detection can work with as much detail as possible. All icon sizes
    // generated afterward are then downscaled from this already
    // transparent-made image.
    //
    // IMPORTANT (optimization): img.to_rgba8() converts/copies the entire
    // image. This intentionally happens only ONCE here, regardless of
    // whether --chroma-key is set or not - and NOT (as in an earlier
    // version) again for every single icon size. For a large source image
    // and the 6 default sizes, that would otherwise have been 6 full
    // copies of the source image, even though the source image doesn't
    // change between sizes.
    let mut rgba_source = img.to_rgba8();
    if let Some(hex) = &args.chroma_key {
        let target = parse_hex_color(hex).map_err(|e| format!("Invalid --chroma-key value: {e}"))?;
        let seeds: Vec<(u32, u32)> = args
            .seeds
            .iter()
            .map(|s| parse_seed(s).map_err(|e| format!("Invalid --seed value: {e}")))
            .collect::<Result<Vec<(u32, u32)>, String>>()?;
        apply_chroma_key(&mut rgba_source, target, args.tolerance, &seeds);
    }

    // Computed ONCE here and passed down to every make_square_icon call
    // (for every single icon size) instead of re-checking per size - see
    // has_transparency()'s doc comment for why this matters.
    let has_alpha = has_transparency(&rgba_source);

    // A heads-up (not an error - upscaling still produces a valid icon,
    // just a softer one) if any requested size exceeds what the source
    // image actually has to offer. Resizing up can't invent detail that
    // isn't there; Lanczos3 (what make_square_icon uses) makes that
    // smooth rather than blocky, but it's still fundamentally a guess,
    // not real detail.
    let (source_w, source_h) = rgba_source.dimensions();
    if use_icns {
        let icns_sizes: Vec<u32> = ICNS_SIZES.iter().map(|&(size, _)| size).collect();
        warn_about_upscaling(source_w, source_h, &icns_sizes);
    } else {
        warn_about_upscaling(source_w, source_h, &sizes);
    }

    // .icns branches off here (whether from an explicit --output-format icns or from
    // the platform-based default computed above): it has its own
    // container format (see write_icns) and doesn't use the ICO-specific
    // --sizes list at all.
    if use_icns {
        write_icns(&rgba_source, args.padding, has_alpha, &output_path)?;
        if args.delete_source {
            delete_source_files(std::slice::from_ref(input_path), Some(&output_path));
        }
        return Ok(());
    }

    // An IconDir collects all the resolutions that will be written
    // together into ONE .ico file at the end.
    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);

    for &size in &sizes {
        // ICO files officially only support edge lengths up to 256px.
        if size == 0 || size > 256 {
            eprintln!("Skipping size {size} (valid range: 1-256).");
            continue;
        }

        let square = make_square_icon(&rgba_source, size, args.padding, has_alpha);
        let (w, h) = square.dimensions();

        // into_raw() gives us the raw pixel bytes in RGBA order (red,
        // green, blue, alpha, red, green, blue, alpha, ...) - exactly the
        // format the ico crate expects as input.
        let raw_rgba = square.into_raw();

        let icon_image = ico::IconImage::from_rgba_data(w, h, raw_rgba);

        // IMPORTANT: we deliberately force PNG encoding for EVERY size,
        // instead of trusting the default "IconDirEntry::encode()" method.
        // Reason: ico::encode() internally decides via a heuristic between
        // PNG and the old BMP format (for compatibility with very old
        // Windows versions). For small and/or fully opaque images it
        // chooses BMP - and that means:
        //   - only a 1-bit transparency mask (a pixel is either fully
        //     visible or fully invisible, no more soft edges)
        //   - often only 8-bit color depth (256-color palette instead of
        //     true color), which causes visible color banding/"pixelation"
        // PNG, on the other hand, always keeps full 32-bit color depth
        // including a clean alpha channel - exactly what was required for
        // "transparency as a feature". PNG-in-ICO has been supported by
        // Windows since Vista (2007), so it's safe for practically any
        // use case.
        let entry = ico::IconDirEntry::encode_as_png(&icon_image)
            .map_err(|e| format!("Could not encode size {size}: {e}"))?;
        icon_dir.add_entry(entry);
    }

    // Create the target file and write all the collected resolutions into it.
    let file =
        std::fs::File::create(&output_path).map_err(|e| format!("Could not create output file: {e}"))?;
    icon_dir
        .write(file)
        .map_err(|e| format!("Error writing ICO file: {e}"))?;

    println!(
        "Done: '{}' created with sizes {:?}.",
        output_path.display(),
        sizes
    );

    if args.delete_source {
        delete_source_files(std::slice::from_ref(input_path), Some(&output_path));
    }

    Ok(())
}
