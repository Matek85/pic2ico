// Everything related to writing the macOS .icns icon format: the size-to-
// OSType mapping Apple's own tools use, and the container writer itself.

use crate::resize::make_square_icon;
use image::{ImageEncoder, RgbaImage};
use std::path::Path;

/// Standard Apple icon sizes (in pixels) mapped to their corresponding
/// ICNS "OSType" codes, matching what Apple's own `iconutil` produces from
/// a .iconset folder of PNGs. Some pixel sizes map to more than one
/// OSType, because the ICNS format distinguishes a size's "native"
/// resolution from its use as the 2x ("Retina") asset of a smaller
/// nominal size - e.g. a 64x64 pixel image is both the native "64x64"
/// icon (icp6) AND the Retina asset for "32x32" (ic12). In a real design
/// workflow those could be different artwork; here they're pixel-identical
/// since we generate every size from the same source image, so we simply
/// reuse the same encoded PNG bytes for both OSTypes.
///
/// Also used by ico_ops::inspect_source_image, to report which of these
/// sizes a source image can cover natively vs. would need upscaling.
pub const ICNS_SIZES: &[(u32, &[[u8; 4]])] = &[
    (16, &[*b"icp4"]),
    (32, &[*b"icp5", *b"ic11"]),
    (64, &[*b"icp6", *b"ic12"]),
    (128, &[*b"ic07"]),
    (256, &[*b"ic08", *b"ic13"]),
    (512, &[*b"ic09", *b"ic14"]),
    (1024, &[*b"ic10"]),
];

/// Writes an .icns (macOS icon) file.
///
/// Container format (verified against the public ICNS specification):
///   - 8-byte file header: 4-byte magic "icns", then a 4-byte big-endian
///     total file length.
///   - Followed directly by any number of entries, back to back. Each
///     entry is: a 4-byte OSType code (e.g. "ic07"), a 4-byte big-endian
///     length (INCLUDING this 8-byte entry header, not just the payload),
///     and then the payload itself.
///   - Since macOS 10.7, the payload for all the OSTypes we use here is
///     simply a complete, standalone PNG file - nothing more exotic than
///     that.
///
/// This function's container-writing logic was checked byte-by-byte
/// against the public ICNS format documentation and the resulting file
/// was independently verified to parse back correctly with a separate
/// ICNS implementation (see this project's history for the exact checks
/// performed) - built and verified without access to an actual Mac, since
/// the environment this was written in only has Linux available. A
/// real-world test on macOS is still worth doing before relying on this
/// for anything important.
pub fn write_icns(
    rgba_source: &RgbaImage,
    padding: u8,
    has_alpha: bool,
    output_path: &Path,
) -> Result<(), String> {
    let mut body: Vec<u8> = Vec::new();

    for &(size, type_codes) in ICNS_SIZES {
        let square = make_square_icon(rgba_source, size, padding, has_alpha);

        // Encode this size as a standalone PNG in memory (not a file on
        // disk) - image::codecs::png::PngEncoder can write directly into
        // any std::io::Write, and a Vec<u8> implements that.
        let mut png_bytes: Vec<u8> = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png_bytes)
            .write_image(square.as_raw(), size, size, image::ExtendedColorType::Rgba8)
            .map_err(|e| format!("Could not encode the {size}x{size} icon as PNG: {e}"))?;

        for type_code in type_codes {
            body.extend_from_slice(type_code);
            let entry_len: u32 = 8 + png_bytes.len() as u32;
            body.extend_from_slice(&entry_len.to_be_bytes());
            body.extend_from_slice(&png_bytes);
        }
    }

    let total_len: u32 = 8 + body.len() as u32;
    let mut file_bytes: Vec<u8> = Vec::with_capacity(total_len as usize);
    file_bytes.extend_from_slice(b"icns");
    file_bytes.extend_from_slice(&total_len.to_be_bytes());
    file_bytes.extend_from_slice(&body);

    std::fs::write(output_path, &file_bytes).map_err(|e| format!("Could not write ICNS file: {e}"))?;

    println!(
        "Done: '{}' created with {} icon size(s) (icns format).",
        output_path.display(),
        ICNS_SIZES.len()
    );

    Ok(())
}
