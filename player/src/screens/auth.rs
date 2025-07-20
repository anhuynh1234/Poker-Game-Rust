use crate::egui::ScrollArea;
use crate::ui::auth::color::*;
use crate::ui::auth::images::*;
use crate::ui::cards::load_card_texture;
use crate::AppState;
use crate::Mode;
use crate::PlayerApp;
use crate::MSG_SIZE;
use eframe::egui;
use eframe::egui::Frame;
use eframe::egui::RichText;
use serde_json::json;
use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Draws the authentication screen with login/register options.
///
/// - Allows players to enter username, password, and server IP.
/// - Spawns an authentication network thread upon submission.
pub fn draw_auth_screen(app: &mut PlayerApp, ctx: &egui::Context) {
    if app.logo_texture.is_none() {
        app.logo_texture = load_poker_logo(ctx);
    }

    egui::CentralPanel::default()
        .frame(Frame::default().fill(BACKGROUND_COLOR))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                // Show logo
                if let Some(texture) = &app.logo_texture {
                    // Get available width of the panel
                    let available_width = ui.available_width();

                    // Desired width: e.g. 50% of available width
                    let desired_width = available_width * 0.3;

                    // Keep aspect ratio
                    let aspect_ratio = texture.size()[0] as f32 / texture.size()[1] as f32;
                    let desired_height = desired_width / aspect_ratio;

                    // Center horizontally
                    ui.vertical_centered(|ui| {
                        ui.image((texture.id(), egui::vec2(desired_width, desired_height)));
                    });

                    ui.add_space(20.0); // Optional vertical spacing
                }

                ui.vertical_centered(|ui| {
                    ui.colored_label(
                        HEADING_COLOR,
                        RichText::new("Poker Player - Login / Register")
                            .heading()
                            .strong(),
                    );
                    ui.add_space(10.0);
                });

                ui.separator();

                ui.vertical_centered(|ui| {
                    ui.radio_value(
                        &mut app.mode,
                        Mode::Register,
                        RichText::new("Register").color(TEXT_COLOR),
                    );
                    ui.radio_value(
                        &mut app.mode,
                        Mode::Login,
                        RichText::new("Login").color(TEXT_COLOR),
                    );
                    ui.add_space(10.0);
                });

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Username:").italics());
                    ui.text_edit_singleline(&mut app.username);
                });

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Password:").italics());
                    ui.add(egui::TextEdit::singleline(&mut app.password).password(true));
                });

                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("Dealer IP:").italics());
                    ui.text_edit_singleline(&mut app.dealer_ip);
                    ui.add_space(20.0);
                });

                ui.vertical_centered(|ui| {
                    if ui.button("Submit").clicked() {
                        let username = app.username.clone();
                        let password = app.password.clone();
                        let mode = app.mode.clone();
                        let dealer_ip = app.dealer_ip.clone();
                        let output_ref = Arc::clone(&app.output);

                        // Spawn a thread to send the one-shot auth command.
                        thread::spawn(move || {
                            match TcpStream::connect(format!("{}:8080", dealer_ip)) {
                                Ok(mut stream) => {
                                    let command = match mode {
                                        Mode::Register => "register",
                                        Mode::Login => "login",
                                    };

                                    println!("[Client] Selected command: {}", command);
                                    println!(
                                        "[Client] Sending credentials - username: {}, password: {}",
                                        username, password
                                    );

                                    let message = json!({
                                        "command": command,
                                        "username": username,
                                        "password": password,
                                    })
                                    .to_string();

                                    println!("[Client] Sending JSON: {}", message);

                                    if stream.write_all(message.as_bytes()).is_ok() {
                                        println!("[Client] Message sent successfully.");

                                        let mut buffer = [0; 512];
                                        match stream.read(&mut buffer) {
                                            Ok(size) => {
                                                let reply =
                                                    String::from_utf8_lossy(&buffer[..size])
                                                        .to_string();
                                                println!(
                                                    "[Client] Received response ({} bytes): {}",
                                                    size, reply
                                                );
                                                *output_ref.lock().unwrap() = reply;
                                            }
                                            Err(e) => {
                                                println!(
                                                    "[Client] Error reading from server: {}",
                                                    e
                                                );
                                                *output_ref.lock().unwrap() =
                                                    format!("Error reading from server: {}", e);
                                            }
                                        }
                                    } else {
                                        println!("[Client] Failed to send message to server.");
                                        *output_ref.lock().unwrap() =
                                            "Failed to send message.".to_string();
                                    }
                                }
                                Err(e) => {
                                    *output_ref.lock().unwrap() =
                                        format!("Connection failed: {}", e);
                                }
                            }
                        });
                    }
                });

                ui.separator();

                let reply = app.output.lock().unwrap().clone();

                ui.vertical_centered(|ui| {
                    ui.label(format!("Server response: {}", reply));
                });

                ui.horizontal(|ui| {
                    if let Some(texture) = load_card_texture(ctx, "5 of Spades") {
                        ui.image((texture.id(), egui::vec2(60.0, 100.0)));
                    }
                    if let Some(texture) = load_card_texture(ctx, "Q of Spades") {
                        ui.image((texture.id(), egui::vec2(100.0, 200.0)));
                    }
                });

                // If the server response indicates a successful login, spawn the persistent thread.
                if reply.contains("game") || reply.contains("Welcome") {
                    // Create channels for communication.
                    let (ui_to_net_tx, ui_to_net_rx) = mpsc::channel::<String>();
                    let (net_to_ui_tx, net_to_ui_rx) = mpsc::channel::<String>();
                    let dealer_ip = app.dealer_ip.clone();
                    let _username = app.username.clone();
                    // Spawn one background thread for persistent communication.
                    thread::spawn(move || {
                        // Connect persistently.
                        let mut client = TcpStream::connect(format!("{}:8080", dealer_ip))
                            .expect("Failed to connect persistently");
                        client
                            .set_nonblocking(true)
                            .expect("Failed to set non-blocking");
                        loop {
                            // Read from the server.
                            let mut buff = vec![0; MSG_SIZE];
                            match client.read(&mut buff) {
                                Ok(_) => {
                                    let msg_bytes = buff
                                        .into_iter()
                                        .take_while(|&x| x != 0)
                                        .collect::<Vec<u8>>();
                                    if let Ok(msg_str) = String::from_utf8(msg_bytes) {
                                        if net_to_ui_tx.send(msg_str).is_err() {
                                            break;
                                        }
                                    }
                                }
                                Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
                                Err(_) => {
                                    println!("Persistent connection severed");
                                    break;
                                }
                            }
                            // Check if UI sent a message.
                            match ui_to_net_rx.try_recv() {
                                Ok(msg) => {
                                    let mut buff = msg.into_bytes();
                                    buff.resize(MSG_SIZE, 0);
                                    if let Err(e) = client.write_all(&buff) {
                                        println!("Failed to send message: {}", e);
                                    }
                                }
                                Err(TryRecvError::Empty) => (),
                                Err(TryRecvError::Disconnected) => break,
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                    });

                    // Save the channel handles in the app.
                    app.ui_to_net_tx = Some(ui_to_net_tx);
                    app.net_to_ui_rx = Some(net_to_ui_rx);
                    // Transition state.
                    app.state = AppState::Ready;
                    *app.output.lock().unwrap() = String::new();
                }
            });
        });
}
