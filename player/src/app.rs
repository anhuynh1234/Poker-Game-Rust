use crate::draw_in_game;
use crate::draw_ready;
use crate::draw_spectator_page;
use crate::draw_stats_page;
use crate::egui::TextureHandle;
use crate::screens::*;
use crate::App;
use crate::AppState;
use crate::Arc;
use crate::Instant;
use crate::Mode;
use crate::Mutex;
use crate::Receiver;
use crate::Sender;
use eframe::egui; // or `use crate::egui;` depending on where egui is defined
use eframe::Frame; // or `use crate::Frame;` or `use crate::egui::Frame;` if Frame is re-exported there

pub struct PlayerApp {
    pub username: String,
    pub password: String,
    pub dealer_ip: String,
    pub mode: Mode,
    pub stats_search_query: String,
    pub user_stats: String,
    pub current_bet: String,
    pub current_swap: String,
    pub state: AppState,
    /// Stores the server response from the one-shot auth command.
    pub output: Arc<Mutex<String>>,
    /// Stores the server response for game state requests for spectator mode.
    pub game_state: Arc<Mutex<String>>,
    pub last_spectate_request_time: Instant,
    /// Channel to send messages from the UI to the network thread.
    pub ui_to_net_tx: Option<Sender<String>>,
    /// Channel to receive messages from the network thread.
    pub net_to_ui_rx: Option<Receiver<String>>,
    pub logo_texture: Option<TextureHandle>,
}

impl Default for PlayerApp {
    fn default() -> Self {
        Self {
            username: String::new(),
            password: String::new(),
            dealer_ip: "127.0.0.1".to_string(),
            mode: Mode::Login,
            stats_search_query: String::new(),
            user_stats: String::new(),
            current_bet: "0".to_string(),
            current_swap: "".to_string(),
            state: AppState::Auth,
            output: Arc::new(Mutex::new(String::new())),
            last_spectate_request_time: Instant::now(),
            game_state: Arc::new(Mutex::new(String::new())),
            ui_to_net_tx: None,
            net_to_ui_rx: None,
        }
    }
}

impl App for PlayerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        match self.state {
            AppState::Auth => draw_auth_screen(self, ctx),
            AppState::InGame => draw_in_game(self, ctx),
            AppState::Ready => draw_ready(self, ctx),
            AppState::Stats => draw_stats_page(self, ctx),
            AppState::Spectator => draw_spectator_page(self, ctx),
        }
        ctx.request_repaint();
    }
}
