use std::env;
use std::path::{Path, PathBuf};

// Constants for Core Learning Loop


#[allow(dead_code)]
pub const BACKLOG_CAP: usize = 200;  // For future use.
pub const PACK_SIZE_DEFAULT: usize = 12;
pub const SMALL_FONT: &str = "PixelMplus10-Regular.ttf";
pub const EMOJI_FONT: &str = "PixelMplus10-Regular.ttf";


pub mod assets {
    // Small menu font for fast startup
    pub const MENU_FONT: &[u8] = include_bytes!("../assets/font/PixelMplus10-Regular.ttf");
    pub const JAPANESE_FONT: &[u8] = include_bytes!("../assets/font/NotoSansJP-Regular.otf");
    pub const CLICK_SOUND: &[u8] = include_bytes!("../assets/sfx/click.wav");
    pub const OPEN_SOUND: &[u8] = include_bytes!("../assets/sfx/open.wav");
}


pub struct Config {
    pub window_width: u32,
    pub font_size_large: u32,
    pub font_size_medium: u32,
    pub font_size_small: u32,
}

impl Config {
    pub fn new() -> Self {
        let is_trimui = Path::new("/mnt/SDCARD").exists(); // basic device check
        let is_rg35xx = Path::new("/storage/.config/").exists();

        let exe_path = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let exe_dir = exe_path.parent().unwrap_or_else(|| Path::new("."));

        let base_assets_dir = exe_dir.join("assets");
        let base_decks_dir = exe_dir.join("decks");
        let sfx_dir = exe_dir.join("sfx");


        let (base_assets, base_decks, sfx_directory) = if is_trimui {
            (
                Path::new(&base_assets_dir),
                Path::new(&base_decks_dir),
                Path::new(&sfx_dir),
            )
        } else if is_rg35xx {
                (Path::new(&base_assets_dir),
                                Path::new(&base_decks_dir),
                                Path::new(&sfx_dir),)
        }else {
            (
                Path::new("/home/jawaad/CardBrick/assets"),
                Path::new("/home/jawaad/CardBrick/assets/decks"),
                Path::new("/home/jawaad/CardBrick/assets/sfx"),
            )
        };

        println!("{:?}", base_assets);
        println!("{:?}", base_decks);
        println!("{:?}", sfx_directory);

        Self {
            window_width: 1024,
            font_size_large: 32,
            font_size_medium: 24,
            font_size_small: 10,
        }
    }
}
