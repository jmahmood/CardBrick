use std::env;
use std::path::{Path, PathBuf};

// Constants for Core Learning Loop


#[allow(dead_code)]
pub const BACKLOG_CAP: usize = 200;  // For future use.
#[allow(dead_code)]
pub const PACK_SIZE_DEFAULT: usize = 12;

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

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use std::fs;
    #[allow(unused_imports)]
    use tempfile::TempDir;

    #[test]
    fn test_config_basic_structure() {
        let config = Config::new();
        
        // Test basic config values
        assert_eq!(config.window_title, "CardBrick v0.1");
        assert_eq!(config.window_width, 1024);
        assert_eq!(config.window_height, 768);
        assert_eq!(config.logical_window_width, 512);
        assert_eq!(config.logical_window_height, 384);
        assert_eq!(config.font_size_large, 32);
        assert_eq!(config.font_size_medium, 24);
        assert_eq!(config.font_size_small, 10);
    }

    #[test]
    fn test_config_font_paths() {
        let config = Config::new();
        
        // Test that font paths contain expected components
        assert!(config.font_path.to_string_lossy().contains("M1MnRegular-M2Gn.ttf"));
        assert!(config.command_font_path.to_string_lossy().contains("Ac437_Tandy1K-II_200L.ttf"));
        assert!(config.emoji_font_path.to_string_lossy().contains("M1MnRegular-M2Gn.ttf"));
        
        // Test that paths include font directory
        assert!(config.font_path.to_string_lossy().contains("font"));
        assert!(config.command_font_path.to_string_lossy().contains("font"));
    }

    #[test] 
    fn test_config_directory_paths() {
        let config = Config::new();
        
        // Test that directory paths are set
        assert!(!config.decks_directory.as_os_str().is_empty());
        assert!(!config.sfx_directory.as_os_str().is_empty());
        
        // Test that paths are absolute or relative from exe
        let decks_str = config.decks_directory.to_string_lossy();
        let sfx_str = config.sfx_directory.to_string_lossy();
        
        // Should contain either assets, decks, or absolute paths
        assert!(decks_str.contains("decks") || decks_str.starts_with("/"));
        assert!(sfx_str.contains("sfx") || sfx_str.starts_with("/"));
    }

    #[test]
    fn test_config_path_consistency() {
        let config = Config::new();
        
        // All asset paths should share a common base
        let font_base = config.font_path.parent().and_then(|p| p.parent());
        let command_font_base = config.command_font_path.parent().and_then(|p| p.parent());
        let emoji_font_base = config.emoji_font_path.parent().and_then(|p| p.parent());
        
        assert_eq!(font_base, command_font_base);
        assert_eq!(font_base, emoji_font_base);
    }

    #[test]
    fn test_device_detection_logic_paths() {
        let config = Config::new();
        
        // Test that we get sensible paths regardless of device detection
        // The logic should always produce valid PathBuf objects
        assert!(config.font_path.is_absolute() || config.font_path.has_root() || !config.font_path.as_os_str().is_empty());
        assert!(config.decks_directory.is_absolute() || config.decks_directory.has_root() || !config.decks_directory.as_os_str().is_empty());
        assert!(config.sfx_directory.is_absolute() || config.sfx_directory.has_root() || !config.sfx_directory.as_os_str().is_empty());
    }

    #[test]
    fn test_window_dimensions_reasonable() {
        let config = Config::new();
        
        // Test that window dimensions are reasonable
        assert!(config.window_width > 0);
        assert!(config.window_height > 0);
        assert!(config.logical_window_width > 0);
        assert!(config.logical_window_height > 0);
        
        // Test that logical dimensions are smaller than or equal to actual
        assert!(config.logical_window_width <= config.window_width);
        assert!(config.logical_window_height <= config.window_height);
    }

    #[test]
    fn test_font_sizes_logical() {
        let config = Config::new();
        
        // Test that font sizes are in logical order
        assert!(config.font_size_small < config.font_size_medium);
        assert!(config.font_size_medium < config.font_size_large);
        
        // Test that font sizes are reasonable
        assert!(config.font_size_small > 0);
        assert!(config.font_size_large < 100); // Reasonable upper bound
    }

    // Note: Testing actual device detection would require mocking the filesystem
    // which is complex for this static function. The current implementation
    // would benefit from dependency injection to make it more testable.
}