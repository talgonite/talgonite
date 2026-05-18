#[derive(Clone, Copy, Debug)]
pub struct MinimapAssets {
    pub tiles_ktx2: &'static [u8],
    pub player_icon_ktx2: &'static [u8],
    pub creature_icon_ktx2: &'static [u8],
}

pub const MINIMAP_TILES_KTX2: &[u8] = formats_macros::include_png_ktx2!("src/minimap_tiles_1x.png");
pub const MINIMAP_ICON_PLAYER_KTX2: &[u8] =
    formats_macros::include_png_ktx2!("src/minimap_icon_player_1x.png");
pub const MINIMAP_ICON_CREATURE_KTX2: &[u8] =
    formats_macros::include_png_ktx2!("src/minimap_icon_creature_1x.png");

pub const FULLSCREEN_MINIMAP_ASSETS: MinimapAssets = MinimapAssets {
    tiles_ktx2: MINIMAP_TILES_KTX2,
    player_icon_ktx2: MINIMAP_ICON_PLAYER_KTX2,
    creature_icon_ktx2: MINIMAP_ICON_CREATURE_KTX2,
};

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{MINIMAP_ICON_CREATURE_KTX2, MINIMAP_ICON_PLAYER_KTX2, MINIMAP_TILES_KTX2};

    #[test]
    fn minimap_tiles_constant_matches_source_png_dimensions() {
        assert_dimensions_match(MINIMAP_TILES_KTX2, include_bytes!("minimap_tiles_1x.png"));
    }

    #[test]
    fn minimap_player_icon_constant_matches_source_png_dimensions() {
        assert_dimensions_match(
            MINIMAP_ICON_PLAYER_KTX2,
            include_bytes!("minimap_icon_player_1x.png"),
        );
    }

    #[test]
    fn minimap_creature_icon_constant_matches_source_png_dimensions() {
        assert_dimensions_match(
            MINIMAP_ICON_CREATURE_KTX2,
            include_bytes!("minimap_icon_creature_1x.png"),
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
