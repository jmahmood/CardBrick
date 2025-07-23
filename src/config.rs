use std::env;
use std::path::{Path, PathBuf};
use macroquad::prelude::*;

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

/// UI assets for the deck selection screen
pub struct UiAssets {
    pub deck_selection_bg: Texture2D,
    pub character_sprite: Texture2D,
}

impl UiAssets {
    /// Load UI assets from embedded bytes or files
    pub fn load() -> Result<Self, String> {
        // Create efficient placeholder textures with less extreme scaling
        
        // Create small background (64x48 - 16:12 aspect ratio, easy to scale)
        let mut bg_pixels = vec![0u8; 64 * 48 * 4];
        for i in 0..(64 * 48) {
            let idx = i * 4;
            bg_pixels[idx] = 45;      // R
            bg_pixels[idx + 1] = 50;  // G  
            bg_pixels[idx + 2] = 65;  // B
            bg_pixels[idx + 3] = 255; // A
        }
        let deck_selection_bg = Texture2D::from_rgba8(64, 48, &bg_pixels);
        
        // Create small character sprite (16x4 for 4 frames of 4x4 each)
        let mut sprite_pixels = vec![0u8; 16 * 4 * 4];
        for frame in 0..4 {
            let colors = [
                [200u8, 100u8, 100u8, 255u8], // Red
                [255u8, 150u8, 100u8, 255u8], // Orange  
                [255u8, 200u8, 100u8, 255u8], // Yellow
                [150u8, 255u8, 100u8, 255u8], // Green
            ];
            let color = colors[frame];
            
            for y in 0..4 {
                for x in 0..4 {
                    let sprite_x = frame * 4 + x;
                    let idx = (y * 16 + sprite_x) * 4;
                    sprite_pixels[idx] = color[0];
                    sprite_pixels[idx + 1] = color[1];
                    sprite_pixels[idx + 2] = color[2];
                    sprite_pixels[idx + 3] = color[3];
                }
            }
        }
        let character_sprite = Texture2D::from_rgba8(16, 4, &sprite_pixels);
        
        Ok(Self {
            deck_selection_bg,
            character_sprite,
        })
    }
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
