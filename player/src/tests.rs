#[cfg(test)]
use crate::*;

/// 1) Ensure PlayerApp::default() yields expected default values.
#[test]
fn test_default_player_app() {
    let app = PlayerApp::default();
    assert_eq!(app.username, "");
    assert_eq!(app.password, "");
    assert_eq!(app.dealer_ip, "127.0.0.1");
    assert_eq!(app.mode, Mode::Login);
    assert_eq!(app.state, AppState::Auth);
    assert_eq!(*app.output.lock().unwrap(), "");
}

/// 2) mode switching from Login to Register, etc.
#[test]
fn test_mode_switch() {
    let mut app = PlayerApp::default(); //default is login
    assert_eq!(app.mode, Mode::Login);

    app.mode = Mode::Register; // switching to register
    assert_eq!(app.mode, Mode::Register);

    app.mode = Mode::Login; // switching back to login
    assert_eq!(app.mode, Mode::Login);
}

/// 3) username & password can be input into the text fields and read back
#[test]
fn test_text_field_input() {
    let mut app = PlayerApp::default();
    app.username = "test".to_string();
    app.password = "testpw".to_string();
    assert_eq!(app.username, "test");
    assert_eq!(app.password, "testpw");
}

/// 4) transitioning to ready state
#[test]
fn test_auth_success_transition() {
    let mut app = PlayerApp::default();
    let (tx_ui, _rx_net) = mpsc::channel();
    let (_tx_net, rx_ui) = mpsc::channel();

    app.ui_to_net_tx = Some(tx_ui);
    app.net_to_ui_rx = Some(rx_ui);
    *app.output.lock().unwrap() = "Welcome test_user, you are now in the game.".to_string();

    // simulate the auth success block
    if app.output.lock().unwrap().contains("Welcome") {
        app.state = AppState::Ready;
        *app.output.lock().unwrap() = String::new();
    }

    assert_eq!(app.state, AppState::Ready);
    assert_eq!(*app.output.lock().unwrap(), "");
}

/// Tests state transition when exiting the game.
#[test]
fn test_exit_game_resets_state() {
    let mut app = PlayerApp::default();
    app.state = AppState::InGame;

    app.state = AppState::Auth; // pressing "exit game" brings back to auth screen
    assert_eq!(app.state, AppState::Auth);
}

#[test]
fn test_state_transitions() {
    let mut app = PlayerApp::default();

    app.state = AppState::Ready;
    assert_eq!(app.state, AppState::Ready);

    app.state = AppState::Stats;
    assert_eq!(app.state, AppState::Stats);

    app.state = AppState::InGame;
    assert_eq!(app.state, AppState::InGame);

    app.state = AppState::Spectator;
    assert_eq!(app.state, AppState::Spectator);

    app.state = AppState::Auth;
    assert_eq!(app.state, AppState::Auth);
}

#[test]
fn test_stats_search_query_input() {
    let mut app = PlayerApp::default();
    app.stats_search_query = "player123".to_string();
    assert_eq!(app.stats_search_query, "player123");
}

#[test]
fn test_user_stats_parsing() {
    let mut app = PlayerApp::default();
    let json_data = r#"{
            "name": "testuser",
            "wins": 5,
            "losses": 3,
            "games_played": 8,
            "money_win": 1200,
            "money_lost": 800
        }"#;

    app.stats_search_query = "testuser".to_string();
    app.user_stats = json_data.to_string();

    let parsed: Value = serde_json::from_str(&app.user_stats).unwrap();
    assert_eq!(parsed["name"], "testuser");
    assert_eq!(parsed["wins"], 5);
    assert_eq!(parsed["games_played"], 8);
}

#[test]
fn test_network_channels_set_after_login() {
    let mut app = PlayerApp::default();
    assert!(app.ui_to_net_tx.is_none());
    assert!(app.net_to_ui_rx.is_none());

    let (tx_ui, _rx_net) = mpsc::channel();
    let (_tx_net, rx_ui) = mpsc::channel();

    app.ui_to_net_tx = Some(tx_ui);
    app.net_to_ui_rx = Some(rx_ui);

    assert!(app.ui_to_net_tx.is_some());
    assert!(app.net_to_ui_rx.is_some());
}

#[test]
fn test_current_bet_and_swap_input() {
    let mut app = PlayerApp::default();
    app.current_bet = "10".to_string();
    app.current_swap = "0,2,4".to_string();

    assert_eq!(app.current_bet, "10");
    assert_eq!(app.current_swap, "0,2,4");
}
