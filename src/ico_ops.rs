// Everything that reads or writes EXISTING .ico files rather than
// converting a fresh source image: --merge, --inspect, --extract and
// --select. These four share a lot of plumbing (opening/parsing an .ico,
// the overwrite check, PNG re-encoding for consistent quality), which is
// why they live together in one module.

use crate::cli::RECOMMENDED_WINDOWS_SIZES;
use crate::icns::ICNS_SIZES;
use crate::util::check_overwrite;
use image::RgbaImage;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Opens and parses an existing .ico file, with the one consistent error
/// message used by every mode that reads one (--merge, --inspect,
/// --extract, --select) - instead of repeating the same open+read+map_err
/// block nearly verbatim in all four.
fn read_icon_dir(path: &Path) -> Result<ico::IconDir, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Could not open '{}': {e}", path.display()))?;
    ico::IconDir::read(file)
        .map_err(|e| format!("Could not read '{}' as an ICO file: {e}", path.display()))
}

/// Merges the icon entries of two or more existing .ico files into one.
///
/// Each entry is decoded back to raw RGBA pixels and then re-encoded as
/// PNG - same reasoning as for the normal conversion path: this guarantees
/// full 32-bit color depth and a clean alpha channel for every size in the
/// output, instead of silently inheriting whatever (possibly lower-quality
/// BMP) encoding the source file happened to use.
///
/// If two source files contain the same size, the first occurrence wins
/// and later duplicates are skipped (with a warning): an .ico file isn't
/// meant to contain the same size twice, and most consumers would only
/// ever look at one of them anyway.
pub fn merge_icons(paths: &[PathBuf], output_path: &Path, force: bool) -> Result<(), String> {
    if paths.len() < 2 {
        return Err("Merge mode needs at least two input .ico files.".to_string());
    }
    check_overwrite(output_path, force)?;

    let mut merged = ico::IconDir::new(ico::ResourceType::Icon);
    // Keeps track of which (width, height) pairs are already in the
    // output, so we can detect and skip duplicates across source files.
    let mut seen_sizes: HashSet<(u32, u32)> = HashSet::new();

    for path in paths {
        let source = read_icon_dir(path)?;

        for entry in source.entries() {
            let size = (entry.width(), entry.height());
            if !seen_sizes.insert(size) {
                eprintln!(
                    "Skipping {}x{} from '{}': that size is already present in the merged output.",
                    size.0,
                    size.1,
                    path.display()
                );
                continue;
            }

            let image = entry.decode().map_err(|e| {
                format!(
                    "Could not decode the {}x{} icon in '{}': {e}",
                    size.0,
                    size.1,
                    path.display()
                )
            })?;
            let new_entry = ico::IconDirEntry::encode_as_png(&image)
                .map_err(|e| format!("Could not re-encode the {}x{} icon: {e}", size.0, size.1))?;
            merged.add_entry(new_entry);
        }
    }

    if merged.entries().is_empty() {
        return Err("No icons found to merge - the resulting file would be empty.".to_string());
    }

    let out_file = std::fs::File::create(output_path)
        .map_err(|e| format!("Could not create output file: {e}"))?;
    merged
        .write(out_file)
        .map_err(|e| format!("Error writing merged ICO file: {e}"))?;

    println!(
        "Done: '{}' created from {} source file(s), containing {} icon(s) total.",
        output_path.display(),
        paths.len(),
        merged.entries().len()
    );

    Ok(())
}

/// Prints a human-readable report about one or more files: for an
/// existing .ico, which sizes it contains (with their zero-based index,
/// so you know what to pass to --select --index), at what color depth,
/// whether each entry is PNG- or (legacy) BMP-encoded, and a couple of
/// sanity warnings. For a regular source image (PNG/JPG/BMP/GIF - the
/// same formats the normal conversion mode accepts), prints its
/// resolution instead, along with which standard icon sizes it can
/// produce without upscaling versus which would come out soft/blurry.
/// Doesn't modify or create anything either way.
pub fn inspect_icons(paths: &[PathBuf]) -> Result<(), String> {
    for (file_index, path) in paths.iter().enumerate() {
        if file_index > 0 {
            println!();
        }

        match read_icon_dir(path) {
            Ok(dir) => inspect_ico_file(path, &dir),
            Err(ico_error) => {
                // Not a valid .ico - but --inspect is also happy to look
                // at a plain source image instead, so try that before
                // giving up. This mirrors what --output-format's
                // no-flags-given default does for macOS vs everyone else:
                // meeting the user where they already are, rather than
                // making them remember which mode to ask for.
                match image::open(path) {
                    Ok(img) => inspect_source_image(path, &img),
                    Err(image_error) => {
                        return Err(format!(
                            "'{}' is neither a readable .ico file ({ico_error}) nor a readable image ({image_error}).",
                            path.display()
                        ));
                    }
                }
            }
        }
    }

    Ok(())
}

/// The .ico-specific half of --inspect's report - see inspect_icons()
/// above for the full picture, including when this isn't the branch that
/// runs.
fn inspect_ico_file(path: &Path, dir: &ico::IconDir) {
    println!("{}:", path.display());
    if dir.entries().is_empty() {
        println!("  (no icons found)");
        return;
    }

    let mut present_sizes: HashSet<u32> = HashSet::new();
    for (index, entry) in dir.entries().iter().enumerate() {
        let (w, h) = (entry.width(), entry.height());
        present_sizes.insert(w);
        let format = if entry.is_png() { "PNG" } else { "BMP" };
        let bpp = entry.bits_per_pixel();
        println!(
            "  [{index}] {w:>4}x{h:<4}  {bpp:>2}bpp  {format}  {} bytes",
            entry.data().len()
        );
        if !entry.is_png() {
            println!(
                "      warning: legacy BMP format - only a 1-bit transparency mask, no smooth alpha edges"
            );
            if bpp < 32 {
                println!(
                    "      warning: reduced color depth ({bpp}bpp instead of 32bpp) - likely visible color banding"
                );
            }
        }
    }

    let missing: Vec<u32> = RECOMMENDED_WINDOWS_SIZES
        .iter()
        .copied()
        .filter(|s| !present_sizes.contains(s))
        .collect();
    if !missing.is_empty() {
        println!(
            "  note: missing common Windows sizes (Windows will have to scale a nearby size for these): {missing:?}"
        );
    }
}

/// The source-image half of --inspect's report: instead of listing
/// existing icon entries (there are none yet - this isn't an .ico file),
/// reports the image's resolution and, for both the Windows and macOS
/// standard size sets, which sizes it could produce natively versus which
/// would need upscaling. Upscaling can't add detail that isn't there, so
/// this is the same distinction demonstrated visually earlier in this
/// project's history (Lanczos3 vs. nearest-neighbor on a tiny source
/// image) - here as a quick heads-up before you even run the conversion,
/// not an error, since upscaling still produces a valid (if softer) icon.
fn inspect_source_image(path: &Path, img: &image::DynamicImage) {
    let (w, h) = (img.width(), img.height());
    let native_max = w.max(h);

    println!("{} (source image, {w}x{h}):", path.display());

    let windows_sizes: Vec<u32> = RECOMMENDED_WINDOWS_SIZES.to_vec();
    let icns_sizes: Vec<u32> = ICNS_SIZES.iter().map(|&(size, _)| size).collect();

    let report_one = |label: &str, sizes: &[u32]| {
        let (native, upscaled): (Vec<u32>, Vec<u32>) =
            sizes.iter().copied().partition(|&s| s <= native_max);
        if !native.is_empty() {
            println!("  {label} sizes this image covers natively: {native:?}");
        }
        if !upscaled.is_empty() {
            println!(
                "  {label} sizes that would need upscaling (may look soft/blurry): {upscaled:?}"
            );
        }
    };

    report_one("Windows", &windows_sizes);
    report_one("macOS (.icns)", &icns_sizes);

    if native_max < 256 {
        println!(
            "  tip: for consistently sharp icons at every common size, a source of at least 256x256 (1024x1024 if you also need .icns) is recommended."
        );
    }
}

/// Extracts every icon size out of an existing .ico file and saves each
/// one as a separate PNG file into a target directory.
pub fn extract_icons(input: &Path, output_dir: Option<&Path>, force: bool) -> Result<(), String> {
    let dir = read_icon_dir(input)?;

    if dir.entries().is_empty() {
        return Err(format!("'{}' contains no icons to extract.", input.display()));
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("icon");

    // Target directory: either given explicitly, or "<stem>_extracted"
    // created right next to the input file.
    let target_dir = match output_dir {
        Some(dir) => dir.to_path_buf(),
        None => {
            let mut dir = input.parent().map(|p| p.to_path_buf()).unwrap_or_default();
            dir.push(format!("{stem}_extracted"));
            dir
        }
    };
    std::fs::create_dir_all(&target_dir).map_err(|e| {
        format!(
            "Could not create output directory '{}': {e}",
            target_dir.display()
        )
    })?;

    // First pass: figure out the (deduplicated) output filename for every
    // entry, and make sure NONE of them would silently overwrite an
    // existing file - before we write anything at all. Without this
    // separate pass, a conflict discovered halfway through would leave a
    // half-extracted directory behind (some sizes written, some not).
    //
    // Guards against the (rare, but possible) case of a broken .ico file
    // that lists the same size more than once - without this we'd
    // silently overwrite one extracted file with another.
    let mut used_names: HashSet<String> = HashSet::new();
    let mut names: Vec<String> = Vec::with_capacity(dir.entries().len());
    for entry in dir.entries() {
        let (w, h) = (entry.width(), entry.height());
        let mut name = format!("{stem}_{w}x{h}.png");
        let mut suffix = 2;
        while !used_names.insert(name.clone()) {
            name = format!("{stem}_{w}x{h}_{suffix}.png");
            suffix += 1;
        }
        check_overwrite(&target_dir.join(&name), force)?;
        names.push(name);
    }

    // Second pass: now that we know none of the target files will be
    // silently clobbered, actually decode and write every one of them.
    let mut extracted_count = 0u32;
    for (entry, name) in dir.entries().iter().zip(names.iter()) {
        let (w, h) = (entry.width(), entry.height());
        let image = entry.decode().map_err(|e| {
            format!(
                "Could not decode the {w}x{h} icon in '{}': {e}",
                input.display()
            )
        })?;

        let rgba = RgbaImage::from_raw(w, h, image.rgba_data().to_vec())
            .ok_or_else(|| format!("Unexpected pixel data size for the {w}x{h} icon"))?;

        let out_path = target_dir.join(name);
        rgba.save(&out_path)
            .map_err(|e| format!("Could not save '{}': {e}", out_path.display()))?;
        extracted_count += 1;
    }

    println!(
        "Done: extracted {extracted_count} icon(s) from '{}' into '{}'.",
        input.display(),
        target_dir.display()
    );

    Ok(())
}

/// Parses a "--index" value ("0,2,4") into a list of zero-based indices.
/// Falls back to just index 0 if the argument was omitted entirely - the
/// agreed-on default for --select when the user doesn't care which exact
/// icon they get.
pub fn parse_indices(input: &Option<String>) -> Result<Vec<usize>, String> {
    let Some(input) = input else {
        return Ok(vec![0]);
    };
    let indices: Vec<usize> = input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<usize>()
                .map_err(|_| format!("Invalid index: '{s}' (must be a non-negative whole number)"))
        })
        .collect::<Result<Vec<usize>, String>>()?;

    if indices.is_empty() {
        return Err(
            "At least one index must be given (or omit --index to use the default: 0)."
                .to_string(),
        );
    }
    Ok(indices)
}

/// Pulls one or more specific icon(s) out of an existing .ico file BY
/// INDEX and re-exports them as standalone .ico file(s) - the ICO-to-ICO
/// counterpart of --extract (which always exports every size as PNG).
///
/// Every selected entry is decoded and re-encoded as PNG, for the same
/// reason as everywhere else in this program: guarantees full 32-bit
/// color depth and a clean alpha channel, regardless of how the source
/// file happened to encode it.
///
/// Returns the single output file path if exactly one file was written
/// (either because only one index was selected, or --combine was used),
/// or None if a whole directory of separate files was written instead -
/// the caller uses this to know whether --delete-source's "don't delete
/// the file we just wrote as output" safety check even applies here.
pub fn select_icons(
    input: &Path,
    indices: &[usize],
    combine: bool,
    output: Option<&Path>,
    force: bool,
) -> Result<Option<PathBuf>, String> {
    let dir = read_icon_dir(input)?;
    let entries = dir.entries();

    if entries.is_empty() {
        return Err(format!("'{}' contains no icons to select from.", input.display()));
    }

    // Validate every requested index up front, with a message that tells
    // the user the valid range instead of a generic "out of bounds" - and
    // points them at --inspect, which is how they'd find a valid index in
    // the first place.
    for &i in indices {
        if i >= entries.len() {
            return Err(format!(
                "Index {i} is out of range for '{}' - it contains {} icon(s), so valid indices are 0..{}. Use --inspect to see them.",
                input.display(),
                entries.len(),
                entries.len() - 1
            ));
        }
    }

    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("icon");

    if combine || indices.len() == 1 {
        // A single output .ico file: either because the user only picked
        // one icon (no point creating a whole directory for that), or
        // because --combine was explicitly given for multiple.
        let output_path = match output {
            Some(p) => p.to_path_buf(),
            None => {
                let mut p = input.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                if let [only_index] = indices {
                    let entry = &entries[*only_index];
                    p.push(format!("{stem}_{}x{}.ico", entry.width(), entry.height()));
                } else {
                    p.push(format!("{stem}_selected.ico"));
                }
                p
            }
        };
        check_overwrite(&output_path, force)?;

        let mut out_dir = ico::IconDir::new(ico::ResourceType::Icon);
        for &i in indices {
            let entry = &entries[i];
            let image = entry.decode().map_err(|e| {
                format!(
                    "Could not decode icon at index {i} in '{}': {e}",
                    input.display()
                )
            })?;
            let new_entry = ico::IconDirEntry::encode_as_png(&image)
                .map_err(|e| format!("Could not re-encode icon at index {i}: {e}"))?;
            out_dir.add_entry(new_entry);
        }

        let file = std::fs::File::create(&output_path)
            .map_err(|e| format!("Could not create output file: {e}"))?;
        out_dir
            .write(file)
            .map_err(|e| format!("Error writing ICO file: {e}"))?;

        println!(
            "Done: '{}' created with {} icon(s) selected from '{}'.",
            output_path.display(),
            indices.len(),
            input.display()
        );

        Ok(Some(output_path))
    } else {
        // Default for multiple indices: one separate .ico file per
        // selected icon, written into a directory - same shape as
        // --extract, just .ico output instead of .png.
        let target_dir = match output {
            Some(p) => p.to_path_buf(),
            None => {
                let mut p = input.parent().map(|p| p.to_path_buf()).unwrap_or_default();
                p.push(format!("{stem}_selected"));
                p
            }
        };
        std::fs::create_dir_all(&target_dir).map_err(|e| {
            format!(
                "Could not create output directory '{}': {e}",
                target_dir.display()
            )
        })?;

        // First pass: compute every target filename and check all of them
        // for --force conflicts before writing anything at all - same
        // reasoning as --extract, so a conflict never leaves a
        // half-written directory behind.
        let mut used_names: HashSet<String> = HashSet::new();
        let mut names: Vec<String> = Vec::with_capacity(indices.len());
        for &i in indices {
            let entry = &entries[i];
            let mut name = format!("{stem}_{}x{}.ico", entry.width(), entry.height());
            let mut suffix = 2;
            while !used_names.insert(name.clone()) {
                name = format!("{stem}_{}x{}_{suffix}.ico", entry.width(), entry.height());
                suffix += 1;
            }
            check_overwrite(&target_dir.join(&name), force)?;
            names.push(name);
        }

        // Second pass: now actually decode and write each one.
        for (&i, name) in indices.iter().zip(names.iter()) {
            let entry = &entries[i];
            let image = entry.decode().map_err(|e| {
                format!(
                    "Could not decode icon at index {i} in '{}': {e}",
                    input.display()
                )
            })?;
            let new_entry = ico::IconDirEntry::encode_as_png(&image)
                .map_err(|e| format!("Could not re-encode icon at index {i}: {e}"))?;

            let mut single = ico::IconDir::new(ico::ResourceType::Icon);
            single.add_entry(new_entry);

            let out_path = target_dir.join(name);
            let out_file = std::fs::File::create(&out_path)
                .map_err(|e| format!("Could not save '{}': {e}", out_path.display()))?;
            single
                .write(out_file)
                .map_err(|e| format!("Error writing '{}': {e}", out_path.display()))?;
        }

        println!(
            "Done: {} icon(s) selected from '{}' into '{}'.",
            indices.len(),
            input.display(),
            target_dir.display()
        );

        Ok(None)
    }
}
