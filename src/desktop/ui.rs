use crate::desktop::browser::{default_workspace_dir, scan_workspace_for_decks, DeckInfo};
use crate::desktop::session::{CardFace, Session};
use crate::desktop::KartaDeck;
use crate::scheduler::sm2::Rating;
use egui::{Align, Color32, Frame, Key, Layout, Margin, RichText, Rounding, Stroke, Vec2};
use std::path::PathBuf;

/// Application state
enum AppState {
    DeckSelection {
        decks: Vec<DeckInfo>,
        selected_index: Option<usize>,
        workspace_path: PathBuf,
    },
    Studying(Session),
    Error(String),
}

/// Actions from the session complete screen
enum SessionCompleteAction {
    None,
    Restart,
    Continue,
    Exit,
}

/// Main application struct for egui
pub struct CardBrickApp {
    state: AppState,
    error_message: Option<String>,
}

impl CardBrickApp {
    pub fn new(session: Session) -> Self {
        Self {
            state: AppState::Studying(session),
            error_message: None,
        }
    }

    pub fn new_with_error(error: String) -> Self {
        Self {
            state: AppState::Error(error.clone()),
            error_message: Some(error),
        }
    }

    /// Create app with optional deck path
    pub fn new_with_deck_path(deck_path: Option<PathBuf>) -> Self {
        if let Some(path) = deck_path {
            // Load the specified deck
            match KartaDeck::from_file(&path) {
                Ok(deck) => {
                    println!("Loaded deck: {} ({} cards)", deck.name, deck.cards.len());
                    match Session::new(deck) {
                        Ok(session) => Self::new(session),
                        Err(e) => Self::new_with_error(format!("Failed to create session: {}", e)),
                    }
                }
                Err(e) => Self::new_with_error(format!("Failed to load deck: {}", e)),
            }
        } else {
            // Show deck selection
            let workspace_path = default_workspace_dir();
            let decks = scan_workspace_for_decks(&workspace_path);

            println!(
                "Found {} decks in {}",
                decks.len(),
                workspace_path.display()
            );

            Self {
                state: AppState::DeckSelection {
                    decks,
                    selected_index: None,
                    workspace_path,
                },
                error_message: None,
            }
        }
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        let session = match &mut self.state {
            AppState::Studying(s) => s,
            _ => return,
        };

        // Space/Enter: flip or grade as Good
        if ctx.input(|i| i.key_pressed(Key::Space) || i.key_pressed(Key::Enter)) {
            match session.current_face() {
                CardFace::Front => session.flip_to_back(),
                CardFace::Back => {
                    if let Err(e) = session.grade_and_next(Rating::Good) {
                        self.error_message = Some(format!("Error grading card: {}", e));
                    }
                }
            }
        }

        // Number keys for grading (only on Back)
        if session.current_face() == CardFace::Back {
            let rating = if ctx.input(|i| i.key_pressed(Key::Num1)) {
                Some(Rating::Again)
            } else if ctx.input(|i| i.key_pressed(Key::Num2)) {
                Some(Rating::Hard)
            } else if ctx.input(|i| i.key_pressed(Key::Num3)) {
                Some(Rating::Good)
            } else if ctx.input(|i| i.key_pressed(Key::Num4)) {
                Some(Rating::Easy)
            } else {
                None
            };

            if let Some(rating) = rating {
                if let Err(e) = session.grade_and_next(rating) {
                    self.error_message = Some(format!("Error grading card: {}", e));
                }
            }
        }

        // F: flip back to front without grading
        if ctx.input(|i| i.key_pressed(Key::F)) {
            session.flip_to_front();
        }

        // Left/Right arrows: manual navigation
        if ctx.input(|i| i.key_pressed(Key::ArrowLeft)) {
            session.prev_card();
        }
        if ctx.input(|i| i.key_pressed(Key::ArrowRight)) {
            session.next_card();
        }

        // Backspace: undo
        if ctx.input(|i| i.key_pressed(Key::Backspace)) {
            if let Err(e) = session.undo_last_grade() {
                self.error_message = Some(format!("Error undoing: {}", e));
            }
        }

        // Mouse click anywhere in window: flip (like Space)
        if ctx.input(|i| i.pointer.primary_clicked()) {
            match session.current_face() {
                CardFace::Front => session.flip_to_back(),
                CardFace::Back => {} // Don't auto-grade on click when on back
            }
        }
    }

    fn render_top_strip(ui: &mut egui::Ui, session: &Session) {
        Frame::none()
            .fill(Color32::from_gray(240))
            .inner_margin(Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Deck name
                    ui.label(RichText::new(session.deck_name()).size(16.0).strong());

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Progress
                        let (current, total) = session.progress();
                        ui.label(
                            RichText::new(format!("{} / {}", current, total))
                                .size(14.0)
                                .color(Color32::from_gray(100)),
                        );
                    });
                });
            });
    }

    fn render_command_strip(ui: &mut egui::Ui, session: &Session) {
        Frame::none()
            .fill(Color32::from_gray(240))
            .inner_margin(Margin::symmetric(16.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Left: hints
                    ui.label(
                        RichText::new("←/→: navigate  F: flip  Backspace: undo")
                            .size(12.0)
                            .color(Color32::from_gray(120)),
                    );

                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        // Center/Right: status
                        let status = match session.current_face() {
                            CardFace::Front => "Press SPACE to show answer",
                            CardFace::Back => "1–4: grade recall, SPACE: OK",
                        };
                        ui.label(
                            RichText::new(status)
                                .size(12.0)
                                .color(Color32::from_gray(100)),
                        );
                    });
                });
            });
    }

    fn render_front_card(ui: &mut egui::Ui, session: &Session) {
        if let Some(card) = session.current_card() {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    // Small label
                    ui.label(
                        RichText::new("Word")
                            .size(14.0)
                            .color(Color32::from_gray(120)),
                    );

                    ui.add_space(20.0);

                    // Large word
                    if let Some(front) = card.front() {
                        ui.label(
                            RichText::new(front)
                                .size(36.0)
                                .strong()
                                .color(Color32::from_gray(20)),
                        );
                    }
                });
            });
        }
    }

    fn render_back_card(ui: &mut egui::Ui, session: &Session) {
        if let Some(card) = session.current_card() {
            ui.vertical(|ui| {
                ui.add_space(20.0);

                // Prompt recap (small)
                if let Some(front) = card.front() {
                    ui.label(
                        RichText::new(format!("Word: {}", front))
                            .size(14.0)
                            .color(Color32::from_gray(120)),
                    );
                }

                ui.add_space(20.0);

                // Answer block (dominant)
                let (definition, example) = card.parse_back();

                Frame::none()
                    .inner_margin(Margin::same(16.0))
                    .rounding(Rounding::same(4.0))
                    .stroke(Stroke::new(1.0, Color32::from_gray(200)))
                    .fill(Color32::from_gray(250))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            // Definition
                            ui.label(
                                RichText::new(&definition)
                                    .size(24.0)
                                    .color(Color32::from_gray(20)),
                            );

                            // Example (if available)
                            if let Some(ex) = example {
                                ui.add_space(12.0);
                                ui.label(
                                    RichText::new(ex).size(16.0).color(Color32::from_gray(80)),
                                );
                            }
                        });
                    });

                ui.add_space(20.0);

                // Rating strip at bottom of card
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("1: I forgot")
                            .size(14.0)
                            .color(Color32::from_gray(100)),
                    );
                    ui.label(RichText::new("  •  ").color(Color32::from_gray(180)));
                    ui.label(
                        RichText::new("2: Hard")
                            .size(14.0)
                            .color(Color32::from_gray(100)),
                    );
                    ui.label(RichText::new("  •  ").color(Color32::from_gray(180)));
                    ui.label(
                        RichText::new("3: OK")
                            .size(14.0)
                            .color(Color32::from_gray(100)),
                    );
                    ui.label(RichText::new("  •  ").color(Color32::from_gray(180)));
                    ui.label(
                        RichText::new("4: Easy")
                            .size(14.0)
                            .color(Color32::from_gray(100)),
                    );
                });
            });
        }
    }

    fn render_card_region(ui: &mut egui::Ui, session: &Session) {
        // Card region with white background
        Frame::none()
            .fill(Color32::WHITE)
            .inner_margin(Margin::same(40.0))
            .show(ui, |ui| match session.current_face() {
                CardFace::Front => Self::render_front_card(ui, session),
                CardFace::Back => Self::render_back_card(ui, session),
            });
    }

    fn render_session_complete(ui: &mut egui::Ui, session: &mut Session) -> SessionCompleteAction {
        let mut action = SessionCompleteAction::None;

        ui.centered_and_justified(|ui| {
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("🎉 Session Complete!")
                        .size(32.0)
                        .strong()
                        .color(Color32::from_rgb(76, 175, 80)),
                );
                ui.add_space(20.0);

                let (remaining, total) = session.remaining_cards_info();
                ui.label(
                    RichText::new(format!(
                        "You've completed all {} due cards.",
                        session.progress().1
                    ))
                    .size(18.0)
                    .color(Color32::from_gray(100)),
                );

                if remaining > 0 {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!(
                            "{} more cards available in this deck ({}% complete)",
                            remaining,
                            ((total - remaining) * 100 / total)
                        ))
                        .size(14.0)
                        .color(Color32::from_gray(120)),
                    );
                }

                ui.add_space(40.0);

                // Action buttons
                if ui
                    .add_sized(
                        [300.0, 50.0],
                        egui::Button::new(RichText::new("↻ Review These Cards Again").size(16.0)),
                    )
                    .clicked()
                {
                    action = SessionCompleteAction::Restart;
                }

                ui.add_space(15.0);

                if remaining > 0 {
                    if ui
                        .add_sized(
                            [300.0, 50.0],
                            egui::Button::new(
                                RichText::new("+ Continue Studying (10 more cards)").size(16.0),
                            ),
                        )
                        .clicked()
                    {
                        action = SessionCompleteAction::Continue;
                    }

                    ui.add_space(15.0);
                }

                if ui
                    .add_sized(
                        [300.0, 50.0],
                        egui::Button::new(RichText::new("✓ Done for Today").size(16.0))
                            .fill(Color32::from_gray(220)),
                    )
                    .clicked()
                {
                    action = SessionCompleteAction::Exit;
                }

                ui.add_space(30.0);
                ui.label(
                    RichText::new("Keyboard: R (review) • C (continue) • Esc (done)")
                        .size(12.0)
                        .color(Color32::from_gray(140)),
                );
            });
        });

        action
    }

    fn render_studying_state(&mut self, ctx: &egui::Context) {
        // Handle keyboard shortcuts for completion screen
        let mut complete_action = SessionCompleteAction::None;
        if let AppState::Studying(session) = &self.state {
            if session.is_complete() {
                if ctx.input(|i| i.key_pressed(Key::R)) {
                    complete_action = SessionCompleteAction::Restart;
                } else if ctx.input(|i| i.key_pressed(Key::C)) {
                    complete_action = SessionCompleteAction::Continue;
                } else if ctx.input(|i| i.key_pressed(Key::Escape)) {
                    complete_action = SessionCompleteAction::Exit;
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if let AppState::Studying(session) = &mut self.state {
                // Layout: top strip, card region, command strip
                ui.vertical(|ui| {
                    // Top strip - inline to avoid borrowing issues
                    Frame::none()
                        .fill(Color32::from_gray(240))
                        .inner_margin(Margin::symmetric(16.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(session.deck_name()).size(16.0).strong());

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let (current, total) = session.progress();
                                    ui.label(
                                        RichText::new(format!("{} / {}", current, total))
                                            .size(14.0)
                                            .color(Color32::from_gray(100)),
                                    );
                                });
                            });
                        });

                    ui.separator();

                    // Card region (expandable)
                    let available_height = ui.available_height() - 60.0;
                    let is_complete = session.is_complete();
                    let current_face = session.current_face();

                    ui.allocate_ui(Vec2::new(ui.available_width(), available_height), |ui| {
                        if is_complete {
                            let action = Self::render_session_complete(ui, session);
                            if !matches!(action, SessionCompleteAction::None) {
                                complete_action = action;
                            }
                        } else {
                            Self::render_card_region(ui, session);
                        }
                    });

                    ui.separator();

                    // Command strip - inline
                    Frame::none()
                        .fill(Color32::from_gray(240))
                        .inner_margin(Margin::symmetric(16.0, 8.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new("←/→: navigate  F: flip  Backspace: undo")
                                        .size(12.0)
                                        .color(Color32::from_gray(120)),
                                );

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    let status = match current_face {
                                        CardFace::Front => "Press SPACE to show answer",
                                        CardFace::Back => "1–4: grade recall, SPACE: OK",
                                    };
                                    ui.label(
                                        RichText::new(status)
                                            .size(12.0)
                                            .color(Color32::from_gray(100)),
                                    );
                                });
                            });
                        });
                });
            }
        });

        // Handle completion actions after rendering
        if let AppState::Studying(session) = &mut self.state {
            match complete_action {
                SessionCompleteAction::Restart => {
                    session.restart_session();
                }
                SessionCompleteAction::Continue => {
                    let added = session.continue_studying();
                    if added > 0 {
                        println!("Added {} more cards to study", added);
                    }
                }
                SessionCompleteAction::Exit => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                SessionCompleteAction::None => {}
            }
        }

        // Request continuous repaint for smooth keyboard input
        ctx.request_repaint();
    }

    fn render_deck_selection(
        ui: &mut egui::Ui,
        decks: &[DeckInfo],
        selected: &mut Option<usize>,
        workspace_path: &PathBuf,
    ) -> Option<PathBuf> {
        let mut deck_to_load = None;
        ui.centered_and_justified(|ui| {
            ui.vertical(|ui| {
                ui.add_space(20.0);

                // Header
                ui.label(RichText::new("Select a Deck").size(32.0).strong());

                ui.add_space(10.0);
                ui.label(
                    RichText::new(format!("Workspace: {}", workspace_path.display()))
                        .size(12.0)
                        .color(Color32::from_gray(120)),
                );

                ui.add_space(20.0);

                if decks.is_empty() {
                    ui.label(
                        RichText::new("No decks found in workspace")
                            .size(18.0)
                            .color(Color32::from_gray(100)),
                    );
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!(
                            "Place deck folders in: {}",
                            workspace_path.display()
                        ))
                        .size(14.0)
                        .color(Color32::from_gray(120)),
                    );
                } else {
                    // Scrollable deck list
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show(ui, |ui| {
                            for (idx, deck_info) in decks.iter().enumerate() {
                                let is_selected = *selected == Some(idx);

                                let response = ui.add(egui::SelectableLabel::new(
                                    is_selected,
                                    RichText::new(&deck_info.name).size(18.0),
                                ));

                                if response.clicked() {
                                    *selected = Some(idx);
                                }

                                // Show card count below
                                ui.label(
                                    RichText::new(format!("{} cards", deck_info.card_count))
                                        .size(12.0)
                                        .color(Color32::from_gray(120)),
                                );

                                ui.add_space(5.0);
                            }
                        });

                    ui.add_space(20.0);

                    // Load button
                    let can_load = selected.is_some();
                    if ui
                        .add_enabled(
                            can_load,
                            egui::Button::new("Load Deck").min_size(Vec2::new(120.0, 40.0)),
                        )
                        .clicked()
                    {
                        if let Some(idx) = *selected {
                            if let Some(deck_info) = decks.get(idx) {
                                deck_to_load = Some(deck_info.path.clone());
                            }
                        }
                    }
                }
            });
        });

        deck_to_load
    }
}

impl eframe::App for CardBrickApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle keyboard input for studying state
        self.handle_keyboard_input(ctx);

        // Show error if any
        let error_to_show = self.error_message.clone();
        if let Some(error) = error_to_show {
            egui::Window::new("Error")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(&error);
                    if ui.button("OK").clicked() {
                        self.error_message = None;
                    }
                });
        }

        // Main panel
        let state_clone = match &self.state {
            AppState::DeckSelection {
                decks,
                selected_index,
                workspace_path,
            } => AppState::DeckSelection {
                decks: decks.clone(),
                selected_index: *selected_index,
                workspace_path: workspace_path.clone(),
            },
            AppState::Studying(_) => return self.render_studying_state(ctx),
            AppState::Error(err) => AppState::Error(err.clone()),
        };

        let deck_to_load = egui::CentralPanel::default()
            .show(ctx, |ui| match &mut self.state {
                AppState::DeckSelection { selected_index, .. } => {
                    if let AppState::DeckSelection {
                        decks,
                        workspace_path,
                        ..
                    } = &state_clone
                    {
                        Self::render_deck_selection(ui, decks, selected_index, workspace_path)
                    } else {
                        None
                    }
                }
                AppState::Error(error) => {
                    let error_msg = error.clone();
                    ui.centered_and_justified(|ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(
                                RichText::new("Error")
                                    .size(24.0)
                                    .strong()
                                    .color(Color32::from_rgb(180, 0, 0)),
                            );
                            ui.add_space(20.0);
                            ui.label(RichText::new(&error_msg).size(16.0));
                        });
                    });
                    None
                }
                _ => None,
            })
            .inner;

        // Handle deck loading outside the closure
        if let Some(deck_path) = deck_to_load {
            match KartaDeck::from_file(&deck_path) {
                Ok(deck) => {
                    println!("Loading deck: {} ({} cards)", deck.name, deck.cards.len());
                    match Session::new(deck) {
                        Ok(session) => {
                            self.state = AppState::Studying(session);
                        }
                        Err(e) => {
                            self.error_message = Some(format!("Failed to create session: {}", e));
                        }
                    }
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to load deck: {}", e));
                }
            }
        }

        // Request continuous repaint for smooth keyboard input
        ctx.request_repaint();
    }
}
