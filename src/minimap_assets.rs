#[derive(Clone, Copy, Debug)]
pub struct MinimapAssets {
    pub tiles_ktx2: &'static [u8],
    pub tiles_width: u32,
    pub tiles_height: u32,
    pub marker_icon_ktx2: &'static [u8],
    pub marker_icon_width: u32,
    pub marker_icon_height: u32,
}

pub const MINIMAP_TILES: (&[u8], u32, u32) =
    formats_macros::include_png_ktx2!("src/minimap_tiles_1x.png");
pub const MINIMAP_ICON_BASE: (&[u8], u32, u32) =
    formats_macros::include_png_ktx2!("src/minimap_icon_base_1x.png");

pub const FULLSCREEN_MINIMAP_ASSETS: MinimapAssets = MinimapAssets {
    tiles_ktx2: MINIMAP_TILES.0,
    tiles_width: MINIMAP_TILES.1,
    tiles_height: MINIMAP_TILES.2,
    marker_icon_ktx2: MINIMAP_ICON_BASE.0,
    marker_icon_width: MINIMAP_ICON_BASE.1,
    marker_icon_height: MINIMAP_ICON_BASE.2,
};

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{MINIMAP_ICON_BASE, MINIMAP_TILES};

    #[test]
    fn minimap_tiles_constant_matches_source_png_dimensions() {
        assert_dimensions_match(MINIMAP_TILES.0, include_bytes!("minimap_tiles_1x.png"));
    }

    #[test]
    fn minimap_icon_constant_matches_source_png_dimensions() {
        assert_dimensions_match(
            MINIMAP_ICON_BASE.0,
            include_bytes!("minimap_icon_base_1x.png"),
        );
    }

    fn assert_dimensions_match(ktx2_bytes: &[u8], png_bytes: &[u8]) {
        let (ktx_width, ktx_height, ktx_data) =
            rendering::texture::Texture::load_ktx2(ktx2_bytes).unwrap();
        let (png_width, png_height) = source_png_dimensions(png_bytes);

        assert_eq!(ktx_width, png_width);
        assert_eq!(ktx_height, png_height);
        assert_eq!(ktx_data.len(), png_width as usize * png_height as usize * 4);
    }

    fn source_png_dimensions(png_bytes: &[u8]) -> (u32, u32) {
        let mut decoder = png::Decoder::new(Cursor::new(png_bytes));
        decoder.set_transformations(png::Transformations::normalize_to_color8());

        let mut reader = decoder.read_info().unwrap();
        let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut decoded).unwrap();

        (info.width, info.height)
    }
}
