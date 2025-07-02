use std::path::PathBuf;
use crate::Path;

pub struct Config {
    pub window_title: &'static str,
    pub window_width: u32,
    pub window_height: u32,
    pub logical_window_width: u32,
    pub logical_window_height: u32,
    pub font_path: PathBuf,
    pub command_font_path: PathBuf,
    pub emoji_font_path: PathBuf,
    pub font_size_large: u32,
    pub font_size_medium: u32,
    pub font_size_small: u32,
    pub decks_directory: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        let is_trimui = Path::new("/mnt/SDCARD").exists(); // basic device check

        let (base_assets, base_decks) = if is_trimui {
            (
                Path::new("/mnt/SDCARD/Tools/tg5040/CardBrick64.pak/assets"),
                Path::new("/mnt/SDCARD/Tools/tg5040/CardBrick64.pak/decks"),
            )
        } else {
            (
                Path::new("./assets"), // desktop/dev build
                Path::new("./decks"),
            )
        };

        Self {
            window_title: "CardBrick v0.1",
            window_width: 1024,
            window_height: 768,
            logical_window_width: 512,
            logical_window_height: 384,
            font_path: base_assets.join("font/NotoSansCJK-Regular.ttc"),
            command_font_path: base_assets.join("font/Ac437_Tandy1K-II_200L.ttf"),
            emoji_font_path: base_assets.join("font/NotoColorEmoji.ttf"),
            font_size_large: 32,
            font_size_medium: 24,
            font_size_small: 10,
            decks_directory: base_decks.to_path_buf(),
        }
    }
}