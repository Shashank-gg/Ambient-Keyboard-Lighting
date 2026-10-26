use std::{collections::HashSet, process, thread};

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(debug_assertions)]
use eframe::egui::style::DebugOptions;
use eframe::{
    egui::{CentralPanel, Context, CornerRadius, Frame, Layout, ScrollArea, Style, TopBottomPanel, ViewportCommand},
    emath::Align,
    epaint::{Color32, Vec2},
    CreationContext,
};

use egui_notify::Toasts;
use legion_input::{EventKind, Key};
use strum::IntoEnumIterator;
use tray_icon::menu::MenuEvent;

use crate::{
    cli::OutputType,
    enums::Effects,
    manager::{self, custom_effect::CustomEffect, profile::Profile, show_effect_ui, EffectManager, ManagerCreationError},
    persist::Settings,
    tray::{QUIT_ID, SHOW_ID},
    DENY_HIDING,
};

use self::{menu_bar::MenuBarState, saved_items::SavedItems, style::Theme};

mod menu_bar;
mod modals;
mod saved_items;
pub mod style;

pub struct App {
    instance_not_unique: bool,
    gui_tx: crossbeam_channel::Sender<GuiMessage>,
    gui_rx: crossbeam_channel::Receiver<GuiMessage>,

    has_tray: Arc<AtomicBool>,
    visible: Arc<AtomicBool>,

    manager: Option<EffectManager>,
    state_changed: bool,
    loaded_effect: LoadedEffect,
    current_profile: Profile,

    menu_bar: MenuBarState,
    saved_items: SavedItems,
    global_rgb: [u8; 3],
    theme: Theme,
    toasts: Toasts,
}

pub enum GuiMessage {
    CycleProfiles,
    Quit,
}

pub struct LoadedEffect {
    state: State,
    effect: CustomEffect,
}

impl LoadedEffect {
    pub fn default() -> Self {
        Self::none()
    }

    pub fn none() -> Self {
        Self {
            state: State::None,
            effect: CustomEffect::default(),
        }
    }

    pub fn queued(effect: CustomEffect) -> Self {
        Self { state: State::Queued, effect }
    }

    pub fn is_none(&self) -> bool {
        matches!(self.state, State::None)
    }

    pub fn is_queued(&self) -> bool {
        matches!(self.state, State::Queued)
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.state, State::Playing)
    }
}

#[derive(Default)]
pub enum State {
    #[default]
    None,
    Queued,
    Playing,
}

impl App {
    pub fn new(output: OutputType, has_tray: Arc<AtomicBool>, visible: Arc<AtomicBool>) -> Self {
        let (gui_tx, gui_rx) = crossbeam_channel::unbounded::<GuiMessage>();

        let manager_result = EffectManager::new(manager::OperationMode::Gui);

        let instance_not_unique = if let Err(err) = &manager_result {
            &ManagerCreationError::InstanceAlreadyRunning == err.current_context()
        } else {
            false
        };

        let manager = manager_result.ok();

        let settings: Settings = Settings::load();
        let Settings { mut current_profile, profiles, effects } = settings;
        // This dedicated build always runs AmbientLight, even if an older settings.json selected another effect.
        let saturation = match current_profile.effect {
            Effects::AmbientLight { saturation_boost, .. } => saturation_boost,
            _ => 0.5,
        };
        current_profile.effect = Effects::AmbientLight { fps: 100, saturation_boost: saturation.clamp(0.0, 1.0) };

        let gui_tx_c = gui_tx.clone();
        // Default app state
        let mut app = Self {
            instance_not_unique,
            gui_tx,
            gui_rx,

            has_tray,
            visible,

            manager,
            // Default to true for an instant update on launch
            state_changed: true,
            loaded_effect: LoadedEffect::default(),
            current_profile,

            menu_bar: MenuBarState::new(gui_tx_c),
            saved_items: SavedItems::new(profiles, effects),
            global_rgb: [0; 3],
            theme: Theme::default(),
            toasts: Toasts::default(),
        };

        // Update the state according to the option chosen by the user
        match output {
            OutputType::Profile(profile) => app.current_profile = profile,
            OutputType::Custom(effect) => app.loaded_effect = LoadedEffect::queued(effect),
            OutputType::NoArgs => {}
            OutputType::Exit => unreachable!("Exiting the app supersedes starting the GUI"),
        }

        app
    }

    pub fn init(self, cc: &CreationContext<'_>) -> Self {
        if !*DENY_HIDING {
            cc.egui_ctx.send_viewport_cmd(ViewportCommand::Visible(self.visible.load(Ordering::SeqCst)));
        }

        let egui_ctx = cc.egui_ctx.clone();
        let gui_tx = self.gui_tx.clone();
        let has_tray = self.has_tray.clone();

        std::thread::spawn(move || loop {
            if let Ok(event) = MenuEvent::receiver().recv() {
                if event.id == SHOW_ID {
                    egui_ctx.request_repaint();

                    egui_ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                    egui_ctx.send_viewport_cmd(ViewportCommand::Focus);
                } else if event.id == QUIT_ID {
                    egui_ctx.request_repaint();

                    let _ = gui_tx.send(GuiMessage::Quit);
                    has_tray.store(false, Ordering::SeqCst);
                }
            }
        });

        let ctx = cc.egui_ctx.clone();
        let gui_tx_c = self.gui_tx.clone();
        if let Some(events) = self.manager.as_ref().and_then(EffectManager::input).map(|input| input.subscribe()) {
            thread::spawn(move || {
                let mut held = HashSet::new();
                let mut active = false;

                while let Ok(event) = events.recv() {
                    match event.kind {
                        EventKind::KeyDown(key) => {
                            held.insert(key);
                        }
                        EventKind::KeyUp(key) => {
                            held.remove(&key);
                        }
                        EventKind::Reset => held.clear(),
                    }

                    let chord_pressed = held.contains(&Key::MetaLeft) && held.contains(&Key::AltRight);
                    if chord_pressed && !active {
                        let _ = gui_tx_c.send(GuiMessage::CycleProfiles);
                        ctx.request_repaint();
                    }
                    active = chord_pressed;
                }
            });
        }

        self.configure_style(&cc.egui_ctx);

        self
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        if let Ok(message) = self.gui_rx.try_recv() {
            match message {
                GuiMessage::CycleProfiles => self.cycle_profiles(),
                GuiMessage::Quit => self.exit_app(),
            }
        }

        // Show active toast messages
        self.toasts.show(ctx);

        if *DENY_HIDING && !self.visible.load(Ordering::SeqCst) {
            self.visible.store(true, Ordering::SeqCst);
            self.toasts
                .warning("Window hiding is currently not supported.\nSee https://github.com/4JX/L5P-Keyboard-RGB/issues/181")
                .duration(None);
        }

        if self.instance_not_unique && modals::unique_instance(ctx) {
            self.exit_app();
        }

        if !self.instance_not_unique && self.manager.is_none() && modals::manager_error(ctx) {
            self.exit_app();
        }

        TopBottomPanel::top("top-panel").show(ctx, |ui| {
            self.menu_bar
                .show(ctx, ui, &mut self.current_profile, &mut self.loaded_effect, &mut self.state_changed, &mut self.toasts);
        });

        CentralPanel::default()
            .frame(Frame::new().inner_margin(self.theme.spacing.large).fill(Color32::from_gray(26)))
            .show(ctx, |ui| {
                ui.style_mut().spacing.item_spacing = Vec2::splat(self.theme.spacing.large);
                self.show_ui_elements(ctx, ui);
            });

        if self.state_changed {
            self.update_state();
        }

        self.handle_close_request(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        let SavedItems { profiles, custom_effects, .. } = self.saved_items.clone();

        let mut settings = Settings::new(profiles, custom_effects, self.current_profile.clone());

        settings.save();

        self.visible.store(false, Ordering::SeqCst);

        if let Some(manager) = self.manager.take() {
            manager.shutdown();
        }
    }
}

impl App {
    fn configure_style(&self, ctx: &Context) {
        let style = Style {
            // text_styles: text_utils::default_text_styles(),
            visuals: self.theme.visuals.clone(),
            #[cfg(debug_assertions)]
            debug: DebugOptions {
                debug_on_hover: false,
                debug_on_hover_with_all_modifiers: false,
                hover_shows_next: false,
                show_expand_width: false,
                show_expand_height: false,
                show_resize: false,
                show_interactive_widgets: false,
                show_widget_hits: false,
                show_unaligned: false,
            },
            ..Style::default()
        };

        // ctx.set_fonts(text_utils::get_font_def());
        ctx.set_style(style);
    }

    fn exit_app(&mut self) {
        use eframe::App;

        self.on_exit(None);

        process::exit(0);
    }

    fn cycle_profiles(&mut self) {
        let len = self.saved_items.profiles.len();

        let current_profile_name = &self.current_profile.name;

        if let Some((i, _)) = self.saved_items.profiles.iter().enumerate().find(|(_, profile)| &profile.name == current_profile_name) {
            if i == len - 1 && len > 0 {
                self.current_profile = self.saved_items.profiles[0].clone();
            } else {
                self.current_profile = self.saved_items.profiles[i + 1].clone();
            }

            self.state_changed = true;
        }
    }

    fn show_ui_elements(&mut self, ctx: &Context, ui: &mut eframe::egui::Ui) {
        ui.with_layout(Layout::left_to_right(Align::Center).with_cross_justify(true), |ui| {
            ui.vertical(|ui| {
                ui.set_width(ui.available_width());
                ui.heading("Ambient Lighting");
                ui.label("Keyboard colors follow the colors on your screen.");
                self.show_effect_ui(ui);
            });

            ui.vertical_centered_justified(|ui| {
                Frame {
                    corner_radius: CornerRadius::same(6),
                    fill: Color32::from_gray(20),
                    ..Frame::default()
                }
                .show(ui, |ui| {
                    ui.label("Ambient Lighting is the only available effect.");
                    ui.label("FPS target: up to 100");
                });
            });
        });
    }

    fn show_effect_ui(&mut self, ui: &mut eframe::egui::Ui) {
        ui.add_enabled_ui(self.loaded_effect.is_none(), |ui| {
            show_effect_ui(ui, &mut self.current_profile, &mut self.state_changed, &self.theme);
        });
    }

    fn update_state(&mut self) {
        if let Some(manager) = self.manager.as_mut() {
            if self.loaded_effect.is_none() {
                manager.set_profile(self.current_profile.clone());
            } else if self.loaded_effect.is_queued() {
                self.loaded_effect.state = State::Playing;

                let effect = self.loaded_effect.effect.clone();
                manager.custom_effect(effect);
            }
        }

        self.state_changed = false;
    }

    fn handle_close_request(&mut self, ctx: &Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !*DENY_HIDING {
            if self.has_tray.load(Ordering::Relaxed) {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            } else {
                // Close normally
            }
        }
    }
}
