use crate::game_files::GameFiles;
use slint::Image;

pub fn load_item_icon(game_files: &GameFiles, sprite_id: u16) -> Result<Image, String> {
    game_ui::assets::load_item_icon(game_files.inner(), sprite_id)
}

pub fn load_skill_icon(game_files: &GameFiles, sprite_id: u16) -> Result<Image, String> {
    game_ui::assets::load_skill_icon(game_files.inner(), sprite_id)
}

pub fn load_spell_icon(game_files: &GameFiles, sprite_id: u16) -> Result<Image, String> {
    game_ui::assets::load_spell_icon(game_files.inner(), sprite_id)
}

pub fn load_world_map_image(game_files: &GameFiles, field_name: &str) -> Result<Image, String> {
    game_ui::assets::load_world_map_image(game_files.inner(), field_name)
}
