pub struct Config {
    pub window_title: &'static str,
    pub window_width: u32,
    pub window_height: u32,
    pub logical_window_width: u32,
    pub logical_window_height: u32,
    pub font_path: &'static str,
    pub font_size_large: u32,
    pub font_size_medium: u32,
    pub font_size_small: u32,
    pub decks_directory: &'static str,
}

impl Config {
    pub fn new() -> Self {
        Self {
            window_title: "CardBrick v0.1",
            window_width: 1024,
            window_height: 768,
            logical_window_width: 512,
            logical_window_height: 384,
            font_path: "/mnt/SDCARD/Tools/tg5040/CardBrick64.pak/fonts/NotoSansCJK-Regular.ttc",
            font_size_large: 32,
            font_size_medium: 24,
            font_size_small: 20,
            decks_directory: "/mnt/SDCARD/Tools/tg5040/CardBrick64.pak/decks",
        }
    }
}