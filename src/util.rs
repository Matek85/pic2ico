// Small, general-purpose helpers that don't belong to any one feature:
// parsing a "--seed" value, the shared overwrite-protection check, and
// safe source-file deletion for --delete-source.

use std::path::{Path, PathBuf};

/// Parses a "--seed" value in the format "x,y" (e.g. "200,50") into a
/// coordinate pair. Returns an understandable error message if the format
/// doesn't match.
pub fn parse_seed(input: &str) -> Result<(u32, u32), String> {
    let parts: Vec<&str> = input.split(',').map(|s| s.trim()).collect();
    if parts.len() != 2 {
        return Err(format!(
            "'{input}' is not a valid seed point (expected format is 'x,y', e.g. '200,50')"
        ));
    }
    let x = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("'{}' is not a valid x coordinate", parts[0]))?;
    let y = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("'{}' is not a valid y coordinate", parts[1]))?;
    Ok((x, y))
}

/// Refuses to silently overwrite an existing file: returns an error
/// (naming --force as the way around it) if `path` already exists and
/// `force` is false. A no-op if `force` is true, or if nothing exists at
/// `path` yet, so callers can just unconditionally call this right before
/// writing.
pub fn check_overwrite(path: &Path, force: bool) -> Result<(), String> {
    if !force && path.exists() {
        return Err(format!(
            "'{}' already exists. Use --force to overwrite it.",
            path.display()
        ));
    }
    Ok(())
}

/// Checks whether two paths point at the same file on disk, resolving
/// symlinks/relative components first (`/a/./b.png` and `/a/b.png` should
/// count as "the same file" even though they're different strings).
/// Falls back to plain path equality if either path can't be resolved
/// (e.g. because it doesn't exist) - a non-existent path can't be "the
/// same file" as anything by resolution anyway, so plain comparison is a
/// reasonable fallback.
fn paths_refer_to_same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Deletes the given source files as part of --delete-source, skipping
/// (with a warning) any file that is also the resolved output path -
/// that would otherwise delete the just-written result instead of a
/// source file. `output_path` is `None` for extract mode, where the
/// output is a directory and can never collide with a source file path.
///
/// A failed deletion (e.g. permission denied) only prints a warning and
/// does not turn the overall command into a failure: the actual
/// conversion/merge/extraction the user asked for already succeeded by
/// the time this runs, so a cleanup problem afterward shouldn't make the
/// whole command look like it failed.
pub fn delete_source_files(paths: &[PathBuf], output_path: Option<&Path>) {
    for path in paths {
        if let Some(output_path) = output_path {
            if paths_refer_to_same_file(path, output_path) {
                eprintln!(
                    "Not deleting '{}': it's also the output path.",
                    path.display()
                );
                continue;
            }
        }
        match std::fs::remove_file(path) {
            Ok(()) => println!("Deleted source file '{}'.", path.display()),
            Err(e) => eprintln!("Warning: could not delete '{}': {e}", path.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// parse_seed must return a normal Ok/Err for absolutely any
        /// string - never panic - for the same reason as
        /// chroma_key::parse_hex_color: it takes raw command-line text
        /// (--seed) with no restrictions on what it can contain.
        #[test]
        fn parse_seed_never_panics(input in ".*") {
            let _ = parse_seed(&input);
        }

        /// Round-trip property: any coordinate pair, formatted as "x,y"
        /// the way --seed expects it, must parse back to exactly that
        /// same pair.
        #[test]
        fn parse_seed_roundtrip(x: u32, y: u32) {
            let input = format!("{x},{y}");
            prop_assert_eq!(parse_seed(&input), Ok((x, y)));
        }
    }
}
