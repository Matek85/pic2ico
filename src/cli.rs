// Everything clap needs to build the command-line interface: the two
// selectable-value enums (SizePreset, OutputFormat) and the top-level Args
// struct they're used from. Kept together in one module since they're
// really one unit - clap reads all of it together to build a single
// coherent --help output.

use clap::Parser;
use std::path::PathBuf;

/// Sizes Microsoft recommends including so Windows always has an exact
/// match for every common DPI scaling level, instead of having to stretch
/// a nearby size and lose sharpness. Used both by SizePreset::Windows (to
/// generate exactly this set) and by ico_ops::inspect_icons (to warn about
/// missing sizes in an existing .ico).
pub const RECOMMENDED_WINDOWS_SIZES: [u32; 10] = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];

// clap automatically generates the following from this struct:
// - command-line argument parsing
// - a --help output
// - error messages for incorrect usage
//
// NOTE: "///" (three slashes) are Rust "doc comments". clap reads exactly
// these automatically and shows them in "--help". Regular comments with
// "//" (two slashes, like this one) do NOT end up in the help text - those
// are only meant as developer documentation for us.
/// A predefined set of icon sizes for a common use case, selectable via
/// --preset instead of listing sizes manually with --sizes.
///
/// clap's `ValueEnum` derive does the same job here that `Parser` does for
/// the whole `Args` struct: it generates the parsing/validation code (so
/// an invalid value like `--preset foo` gets a clear clap error listing
/// the valid options) and feeds `--help` from this enum's variants and
/// their doc comments, without us writing any of that by hand.
///
/// Adding a new preset later is just: add a variant here, add its doc
/// comment, and add one line to `SizePreset::sizes()` below - the CLI
/// parsing and --help text update themselves automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum SizePreset {
    /// Microsoft's recommended set for full DPI-scaling coverage (16, 20,
    /// 24, 32, 40, 48, 64, 96, 128, 256) - Windows never has to stretch a
    /// nearby size at any scaling level.
    Windows,
    /// The classic small multi-resolution favicon.ico set used on
    /// websites (16, 32, 48).
    Favicon,
    /// Just two sizes (16, 32) for a small file / quick test.
    Minimal,
}

impl SizePreset {
    /// Returns the concrete list of sizes this preset expands to.
    pub fn sizes(self) -> &'static [u32] {
        match self {
            SizePreset::Windows => &RECOMMENDED_WINDOWS_SIZES,
            SizePreset::Favicon => &[16, 32, 48],
            SizePreset::Minimal => &[16, 32],
        }
    }
}

/// Which icon container format to write, selectable via --output-format.
/// If --output-format isn't given at all (the field stays `None` in
/// `Args`), img2ico falls back to a platform-based default instead - see
/// where `use_icns` is computed in `run()` (main.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// The Windows icon format.
    Ico,
    /// The macOS icon format.
    Icns,
}

#[derive(Parser, Debug)]
#[command(
    name = "img2ico",
    about = "Converts any image into an ICO file (with transparency)"
)]
pub struct Args {
    /// Input file(s).
    /// - Normal mode: exactly one image file (PNG, JPG, BMP, GIF) to convert.
    /// - With --merge: two or more existing .ico files whose icons should
    ///   be combined into one output file.
    #[arg(required = true)]
    pub input: Vec<PathBuf>,

    /// Path to the output file. If not given, the input file's name is
    /// used, just with the ".ico" extension instead.
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Comma-separated list of icon sizes to embed into the .ico file.
    /// Windows ICOs can contain several resolutions at once (the operating
    /// system then picks the appropriate one depending on context, e.g.
    /// small for the taskbar, large for the desktop view).
    #[arg(short, long, default_value = "16,32,48,64,128,256")]
    pub sizes: String,

    /// Overrides --sizes with a predefined set of sizes for a common use
    /// case, instead of listing sizes manually (see the presets listed
    /// below for what each one contains). --sizes is ignored whenever
    /// this is given. This is the Windows-side equivalent of what
    /// producing an .icns file always does unconditionally (uses Apple's
    /// full recommended size set, ignoring --sizes) - so --preset has no
    /// effect whenever the output ends up being .icns, whether that's
    /// because of an explicit --output-format icns or simply because you're
    /// running this on macOS, where icns is the default (see
    /// --output-format).
    #[arg(long = "preset", value_enum)]
    pub preset: Option<SizePreset>,

    /// Optional hex color code (e.g. "#00FF00" or "00ff00") for "chroma
    /// key" mode: starting from the image border, a connected region of
    /// this color is found and made transparent - the classic use case is
    /// removing a solid-colored background. If this option is omitted,
    /// nothing changes compared to the previous behavior.
    #[arg(short = 'c', long = "chroma-key")]
    pub chroma_key: Option<String>,

    /// Tolerance for the chroma-key color comparison (0-100), only
    /// relevant together with --chroma-key.
    /// 0   = only (nearly) exactly the given color is recognized.
    /// 100 = almost any color would count as "background" (not useful in
    ///       practice).
    /// A good starting value to experiment with is 15-30.
    #[arg(short = 't', long = "tolerance", default_value_t = 20)]
    pub tolerance: u8,

    /// Extra starting point for the chroma-key flood fill, as "x,y"
    /// (pixel coordinates IN THE ORIGINAL IMAGE, not in the resulting
    /// icon). Can be given multiple times (e.g. --seed 200,50 --seed
    /// 40,300).
    ///
    /// Normally the flood fill only starts from the image border. If the
    /// desired background color is enclosed by a frame/ring (e.g. an icon
    /// with a dark circular outline, inside which the background color
    /// occurs again), the image border can't reach that area. With --seed
    /// you give a point that lies somewhere INSIDE that enclosed area
    /// (e.g. the pixel coordinate where you'd hover the mouse over the
    /// background color in an image editor) - from there the flood fill
    /// then spreads exactly the same way it does from the border.
    #[arg(long = "seed")]
    pub seeds: Vec<String>,

    /// Merge mode: instead of converting an image, combine the icon
    /// entries of two or more existing .ico files (passed as INPUT) into
    /// a single output .ico file. Every entry is decoded and re-encoded
    /// as PNG, so the merged file has the same consistent 32-bit quality
    /// as a freshly converted one, regardless of how the source files
    /// were created. If two source files contain the same size, the
    /// first occurrence wins and later duplicates are skipped (with a
    /// warning). All image-specific options (--sizes, --chroma-key,
    /// --tolerance, --seed) are ignored in this mode.
    #[arg(long = "merge")]
    pub merge: bool,

    /// Inspect mode: instead of converting anything, print a report about
    /// one or more files (passed as INPUT). For an existing .ico: which
    /// sizes it contains, at what color depth, whether each entry is
    /// stored as PNG or as the legacy BMP format, and a few sanity
    /// warnings (e.g. reduced color depth, or common Windows DPI sizes
    /// that are missing). For a regular source image instead: its
    /// resolution, and which standard icon sizes (Windows and macOS) it
    /// can produce without upscaling versus which would come out
    /// soft/blurry - handy to check before converting. Nothing is written
    /// to disk in this mode.
    #[arg(long = "inspect")]
    pub inspect: bool,

    /// Extract mode: instead of converting an image, pull every icon size
    /// out of an existing .ico file (passed as INPUT, exactly one file)
    /// and save each one as a separate PNG. -o/--output is used as the
    /// target DIRECTORY here (not a file path) and is created if it
    /// doesn't exist yet; if omitted, a directory named after the input
    /// file is created next to it.
    #[arg(long = "extract")]
    pub extract: bool,

    /// Select mode: instead of converting an image, pull one or more
    /// specific icon(s) out of an existing .ico file (passed as INPUT,
    /// exactly one file) BY INDEX and re-export them as standalone .ico
    /// file(s) - unlike --extract, which always exports every size as
    /// PNG. Use --inspect first to see which index corresponds to which
    /// size (indices are shown there in [brackets]).
    #[arg(long = "select")]
    pub select: bool,

    /// Comma-separated list of zero-based indices to pull out, only
    /// relevant together with --select. If omitted, defaults to just
    /// index 0 (the first icon in the file). With more than one index:
    /// by default each selected icon is saved as its own separate .ico
    /// file into a directory (like --extract does for PNGs) - pass
    /// --combine to bundle them into a single multi-size .ico file
    /// instead.
    #[arg(long = "index")]
    pub index: Option<String>,

    /// Bundles multiple --index selections into a single output .ico file
    /// with all of them, instead of one separate file per selected icon
    /// (the default). Has no effect with only a single index.
    #[arg(long = "combine")]
    pub combine: bool,

    /// Adds transparent padding around the image before it's placed onto
    /// the square icon canvas, as a percentage (0-100) of the icon size.
    /// For example, --padding 10 on a 256x256 icon leaves roughly a 10%
    /// margin on every side, so the actual artwork ends up smaller and
    /// more centered instead of touching the edges. 0 (the default) keeps
    /// the previous behavior (artwork fills the canvas as much as
    /// possible while preserving aspect ratio).
    #[arg(long = "padding", default_value_t = 0)]
    pub padding: u8,

    /// Which icon container format to write: "ico" (the Windows format)
    /// or "icns" (the macOS format, using a fixed, Apple-recommended set
    /// of sizes - 16, 32, 64, 128, 256, 512 and 1024 pixels, each
    /// PNG-encoded - instead of --sizes, which is ignored whenever the
    /// output is icns). --chroma-key, --tolerance, --seed and --padding
    /// all still apply either way, since they're independent of the
    /// container format.
    ///
    /// If this is omitted entirely, img2ico picks automatically based on
    /// the platform it's running on: icns on macOS, ico everywhere else -
    /// since that matches what almost everyone building on a given
    /// platform actually wants. Pass --output-format explicitly to
    /// override that default in either direction (e.g. to produce a
    /// Windows .ico file while working on a Mac).
    ///
    /// Note: the icns container format was verified byte-by-byte against
    /// the public ICNS specification and cross-checked with an
    /// independent parser, but this was built without access to a real
    /// Mac - a real-world check on macOS is still recommended before
    /// relying on it for anything important.
    #[arg(long = "output-format", value_enum)]
    pub output_format: Option<OutputFormat>,

    /// Deletes the original source file(s) after a successful run, but
    /// only after the output has been written completely - if anything
    /// fails along the way, nothing gets deleted. Disabled by default,
    /// since accidentally deleting the only copy of a source image is a
    /// real risk; has to be turned on explicitly. Applies to --merge and
    /// --extract too (deletes the merged .ico files / the extracted .ico,
    /// respectively). Has no effect with --inspect, which never writes
    /// anything and has nothing to delete.
    ///
    /// Safety net: a source file is never deleted if it turns out to also
    /// be the output path (which would delete the just-written result
    /// instead of a source file) - that one file is skipped with a
    /// warning instead.
    #[arg(long = "delete-source")]
    pub delete_source: bool,

    /// Allows overwriting output file(s) that already exist. Disabled by
    /// default: without --force, img2ico refuses to run if the resolved
    /// output already exists on disk, instead of silently overwriting it,
    /// so converting the same file twice by accident doesn't quietly
    /// destroy the previous result. Applies to the normal output file
    /// (regardless of --output-format) and --merge; for --extract it
    /// checks every individual file that would be written before writing
    /// any of them, so a conflict never leaves a half-extracted directory
    /// behind.
    #[arg(short = 'f', long = "force")]
    pub force: bool,
}
