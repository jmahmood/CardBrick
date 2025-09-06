// src/scenes/main_menu/mod.rs

use macroquad::prelude::*;
use crate::ui::{FontManager, CanvasManager};

// This line was missing. It tells the main_menu module
// that the input.rs file is part of it.
pub mod input;

/// Contains the state specific to the main menu screen.
pub struct MainMenuState {
    pub selected_index: usize,
    // Offscreen cache for static background + static labels
    bg_rt: Option<RenderTarget>,
    bg_dirty: bool,
}

impl MainMenuState {
    /// Creates a new MainMenuState with a default selection.
    pub fn new() -> Self {
        Self { selected_index: 0, bg_rt: None, bg_dirty: true }
    }
}

/// Draws the main menu scene.
/// This function was moved from main.rs.
pub fn draw_main_menu_scene(
    font_manager: &FontManager,
    canvas_manager: &CanvasManager,
    state: &mut MainMenuState,
) -> Result<(), String> {
    let options = ["Study", "Progress", "Send Backup", "Quit"];
    
    // Offscreen cache background + static texts
    let (logical_width, logical_height) = canvas_manager.logical_size();
    let (screen_x, screen_y) = canvas_manager.logical_to_screen(0.0, 0.0);
    let screen_w = logical_width * canvas_manager.get_scale_factor();
    let screen_h = logical_height * canvas_manager.get_scale_factor();

    if state.bg_rt.is_none() || state.bg_dirty {
        let rt = render_target(512, 384);
        rt.texture.set_filter(FilterMode::Nearest);

        set_camera(&Camera2D {
            zoom: vec2(1.0 / 512.0, 1.0 / 384.0),
            target: vec2(256.0, 192.0),
            render_target: Some(rt.clone()),
            ..Default::default()
        });

        clear_background(Color::from_rgba(40, 40, 45, 255));

        // Use unit canvas to draw at logical coordinates without scaling
        let unit_canvas = CanvasManager::new_with_screen_size(512.0, 384.0)
            .map_err(|e| format!("Canvas init failed for RT: {}", e))?;
        font_manager.draw_single_line("CardBrick", 20, 50, &unit_canvas)?;
        font_manager.draw_single_line("Manual sync available", 20, 80, &unit_canvas)?;

        set_default_camera();

        state.bg_rt = Some(rt);
        state.bg_dirty = false;
    }

    if let Some(rt) = &state.bg_rt {
        draw_texture_ex(
            &rt.texture,
            screen_x,
            screen_y,
            WHITE,
            DrawTextureParams { dest_size: Some(Vec2::new(screen_w, screen_h)), ..Default::default() },
        );
    }

    let mut y_pos = 150;
    let font_size = 18.0;
    for (i, option) in options.iter().enumerate() {
        if i == state.selected_index {
            let (text_w, text_h) = font_manager.size_of_text(option)?;
            let (highlight_x, highlight_y) = canvas_manager.logical_to_screen(font_size, y_pos as f32);
            let highlight_w = (text_w + 4) as f32 * canvas_manager.get_scale_factor();
            let highlight_h = text_h as f32 * canvas_manager.get_scale_factor();
            
            draw_rectangle(highlight_x, highlight_y - (text_h as f32), highlight_w, highlight_h, Color::from_rgba(80, 80, 80, 255));
        }
        font_manager.draw_single_line(option, 20, y_pos, canvas_manager)?;
        y_pos += 40;
    }
    Ok(())
}
