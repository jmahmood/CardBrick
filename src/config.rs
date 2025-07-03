use std::env;
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
    pub sfx_directory: PathBuf,
}

impl Config {
    pub fn new() -> Self {
        let is_trimui = Path::new("/mnt/SDCARD").exists(); // basic device check
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
        } else {
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
            window_title: "CardBrick v0.1",
            window_width: 1024,
            window_height: 768,
            logical_window_width: 512,
            logical_window_height: 384,
            font_path: base_assets.join("font/M1MnRegular-M2Gn.ttf"),
            command_font_path: base_assets.join("font/Ac437_Tandy1K-II_200L.ttf"),
            emoji_font_path: base_assets.join("font/M1MnRegular-M2Gn.ttf"),
            font_size_large: 32,
            font_size_medium: 24,
            font_size_small: 10,
            decks_directory: base_decks.to_path_buf(),
            sfx_directory: sfx_directory.to_path_buf(),
        }
    }
}