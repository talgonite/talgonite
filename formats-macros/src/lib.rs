use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use proc_macro::TokenStream;
use proc_macro2::{Literal, TokenStream as TokenStream2};
use quote::quote;
use syn::{LitStr, parse_macro_input};

#[proc_macro]
pub fn include_png_ktx2(input: TokenStream) -> TokenStream {
    let path_lit = parse_macro_input!(input as LitStr);

    match expand_include_png_ktx2(&path_lit) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

struct EncodedTexture {
    source_path: PathBuf,
    width: u32,
    height: u32,
    bytes: Vec<u8>,
}

fn expand_include_png_ktx2(path_lit: &LitStr) -> Result<TokenStream2, syn::Error> {
    let encoded = encode_png_path(&path_lit.value())
        .map_err(|error| syn::Error::new(path_lit.span(), error))?;
    let source_path = normalized_path_literal(&encoded.source_path);
    let source_path_lit = LitStr::new(&source_path, path_lit.span());
    let bytes = Literal::byte_string(&encoded.bytes);
    let width = encoded.width;
    let height = encoded.height;

    Ok(quote! {{
        const _: &[u8] = include_bytes!(#source_path_lit);
        const _: (u32, u32) = (#width, #height);
        #bytes
    }})
}

fn encode_png_path(path: &str) -> Result<EncodedTexture, String> {
    let source_path = resolve_source_path(path)?;
    let (width, height, rgba) = decode_png_rgba8(&source_path)?;
    let bytes = formats::ktx2::encode_ktx2(
        width,
        height,
        formats::ktx2::VK_FORMAT_R8G8B8A8_UNORM,
        &rgba,
    )
    .map_err(|error| {
        format!(
            "failed to build KTX2 bytes for {}: {error}",
            source_path.display()
        )
    })?;

    Ok(EncodedTexture {
        source_path,
        width,
        height,
        bytes,
    })
}

fn resolve_source_path(path: &str) -> Result<PathBuf, String> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .map_err(|error| format!("CARGO_MANIFEST_DIR is unavailable: {error}"))?;
    let candidate = Path::new(path);
    let resolved = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        manifest_dir.join(candidate)
    };

    if resolved.is_file() {
        Ok(resolved)
    } else {
        Err(format!("PNG file not found: {}", resolved.display()))
    }
}

fn decode_png_rgba8(source_path: &Path) -> Result<(u32, u32, Vec<u8>), String> {
    let file = File::open(source_path)
        .map_err(|error| format!("failed to open {}: {error}", source_path.display()))?;

    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(
        png::Transformations::normalize_to_color8() | png::Transformations::ALPHA,
    );

    let mut reader = decoder.read_info().map_err(|error| {
        format!(
            "failed to decode PNG header for {}: {error}",
            source_path.display()
        )
    })?;
    let mut decoded = vec![
        0;
        reader.output_buffer_size().ok_or_else(|| {
            format!(
                "decoded PNG buffer size overflow for {}",
                source_path.display()
            )
        })?
    ];
    let info = reader.next_frame(&mut decoded).map_err(|error| {
        format!(
            "failed to decode PNG data for {}: {error}",
            source_path.display()
        )
    })?;
    let decoded = &decoded[..info.buffer_size()];

    if info.bit_depth != png::BitDepth::Eight {
        return Err(format!(
            "{} decoded to unsupported bit depth {:?}; expected 8-bit output",
            source_path.display(),
            info.bit_depth
        ));
    }

    let rgba = convert_to_rgba8(info.color_type, decoded)?;
    let expected_len = pixel_count(info.width, info.height)?
        .checked_mul(4)
        .ok_or_else(|| format!("image byte length overflow for {}", source_path.display()))?;

    if rgba.len() != expected_len {
        return Err(format!(
            "decoded RGBA byte length mismatch for {}: expected {}, got {}",
            source_path.display(),
            expected_len,
            rgba.len()
        ));
    }

    Ok((info.width, info.height, rgba))
}

fn convert_to_rgba8(color_type: png::ColorType, decoded: &[u8]) -> Result<Vec<u8>, String> {
    match color_type {
        png::ColorType::Rgba => Ok(decoded.to_vec()),
        png::ColorType::Rgb => Ok(decoded
            .chunks_exact(3)
            .flat_map(|pixel| [pixel[0], pixel[1], pixel[2], 255])
            .collect()),
        png::ColorType::Grayscale => Ok(decoded
            .iter()
            .flat_map(|value| [*value, *value, *value, 255])
            .collect()),
        png::ColorType::GrayscaleAlpha => Ok(decoded
            .chunks_exact(2)
            .flat_map(|pixel| [pixel[0], pixel[0], pixel[0], pixel[1]])
            .collect()),
        png::ColorType::Indexed => {
            Err("indexed PNG remained indexed after normalization; RGBA expansion failed".into())
        }
    }
}

fn pixel_count(width: u32, height: u32) -> Result<usize, String> {
    let width = usize::try_from(width).map_err(|_| "image width does not fit usize".to_string())?;
    let height =
        usize::try_from(height).map_err(|_| "image height does not fit usize".to_string())?;
    width
        .checked_mul(height)
        .ok_or_else(|| "image dimensions overflow usize".to_string())
}

fn normalized_path_literal(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::encode_png_path;

    #[test]
    fn encodes_minimap_png_to_rgba8_ktx2() {
        let encoded = encode_png_path("../src/minimap_tiles_2x.png").unwrap();
        let reader = ktx2::Reader::new(&encoded.bytes).unwrap();
        let header = reader.header();
        let level = reader.levels().next().unwrap();

        assert_eq!(header.pixel_width, encoded.width);
        assert_eq!(header.pixel_height, encoded.height);
        assert_eq!(
            level.data.len(),
            encoded.width as usize * encoded.height as usize * 4
        );
    }
}
