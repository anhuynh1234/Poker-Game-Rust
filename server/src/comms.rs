
use std::{
    collections::HashMap,
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    sync::mpsc::{self, Sender},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use crate::*;


/// Handles communication with a single connected client.
///
/// This function runs in its own thread for each client. It:
/// - Reads messages sent from the client and forwards them to the main server thread.
/// - Listens for messages from the server and sends them back to the client.
/// - Uses non-blocking I/O and a small sleep to avoid busy-waiting.
///
/// # Arguments
/// * `socket` - The TCP stream connected to the client.
/// * `addr` - The network address of the client.
/// * `server_tx` - Sender to forward client messages to the server.
/// * `client_rx` - Receiver to get messages from the server for this client.
///
/// This loop continues until the client disconnects or an error occurs.
pub fn handle_client(
    mut socket: TcpStream,
    addr: std::net::SocketAddr,
    server_tx: Sender<(std::net::SocketAddr, String)>,
    client_rx: mpsc::Receiver<String>,
) {
    socket
        .set_nonblocking(true)
        .expect("Failed to set client socket non-blocking");

    let mut buffer = [0u8; MSG_SIZE];

    loop {
        // Read client message
        match socket.read(&mut buffer) {
            Ok(_) => {
                println!("[Server] Received data from client: {}", addr);
                
                let msg = buffer
                    .iter()
                    .cloned()
                    .take_while(|&x| x != 0)
                    .collect::<Vec<u8>>();
        
                match String::from_utf8(msg) {
                    Ok(text) => {
                        println!("[Server] Parsed UTF-8 message from {}: {}", addr, text);
                        if let Err(e) = server_tx.send((addr, text.clone())) {
                            eprintln!("[Server] Failed to forward message from {}: {}", addr, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("[Server] Invalid UTF-8 from {}: {}", addr, e);
                    }
                }

                buffer.fill(0);
            }
        
            Err(ref err) if err.kind() == ErrorKind::WouldBlock => {
                // No data yet — expected with non-blocking I/O
            }
        
            Err(e) => {
                println!("[Server] Client {} disconnected (read error): {}", addr, e);
                break;
            }
        }
        
        // Check if the server sent a message to this client
        match client_rx.try_recv() {
            Ok(reply) => {
                println!("[Server] Sending reply to {}: {}", addr, reply);
                let mut reply_bytes = reply.into_bytes();
                reply_bytes.resize(MSG_SIZE, 0);
                if let Err(e) = socket.write_all(&reply_bytes) {
                    eprintln!("[Server] Failed to send reply to {}: {}", addr, e);
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No message for client this round
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                println!("[Server] Channel to {} closed", addr);
                break;
            }
        }

        thread::sleep(Duration::from_millis(100));
    }
}

/// Sends a message to a specific connected client.
///
/// # Arguments
/// * `clients` - Shared list of connected clients.
/// * `addr` - The address of the target client.
/// * `message` - The message to send.
///
/// If the client is connected, the message is sent over its channel.
pub fn send_to_client(
    clients: &Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>>,
    addr: &std::net::SocketAddr,
    message: &str,
) {
    if let Some(client) = clients.lock().unwrap().get(addr) {
        let _ = client.sender.send(message.to_string());
    }
}

/// Broadcasts a message to all connected clients.
///
/// # Arguments
/// * `clients` - Shared list of connected clients.
/// * `message` - The message to send.
///
/// The message is cloned and sent to every client.
pub fn broadcast_message(
    clients: &Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>>,
    message: &str,
) {
    for client in clients.lock().unwrap().values() {
        let _ = client.sender.send(message.to_string());
    }
}

/// Broadcasts a message to all players currently in the game.
///
/// # Arguments
/// * `clients` - Shared list of connected clients.
/// * `message` - The message to send.
///
/// Only clients who are registered as active game players will receive the message.
pub fn broadcast_to_game_players(
    clients: &Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>>,
    message: &str,
) {
    if let Some(player_map) = GAME_PLAYERS.get() {
        let player_addrs: Vec<std::net::SocketAddr> = {
            let map = player_map.lock().unwrap();
            map.values().cloned().collect()
        };

        let clients = clients.lock().unwrap();
        for addr in player_addrs {
            if let Some(client) = clients.get(&addr) {
                if let Err(e) = client.sender.send(message.to_string()) {
                    eprintln!("[Broadcast] Error sending to {}: {}", addr, e);
                }
            }
        }
    }
}

/// Sends a message to a specific player by their player ID.
///
/// # Arguments
/// * `clients` - Shared list of connected clients.
/// * `player_id` - The player’s unique id.
/// * `message` - The message to send.
///
/// If the player is found in the active game player list, the message is sent to them.
pub fn send_to_player_by_id(
    clients: &Arc<Mutex<HashMap<std::net::SocketAddr, ClientInfo>>>,
    player_id: &str,
    message: &str,
) {
    if let Some(player_map) = GAME_PLAYERS.get() {
        if let Some(addr) = player_map.lock().unwrap().get(player_id) {
            if let Some(client) = clients.lock().unwrap().get(addr) {
                if let Err(e) = client.sender.send(message.to_string()) {
                    eprintln!("[SendToPlayer] Failed to send message to {}: {}", player_id, e);
                }
            } else {
                eprintln!("[SendToPlayer] No client found for address: {}", addr);
            }
        } else {
            eprintln!("[SendToPlayer] No address found for player: {}", player_id);
        }
    } else {
        eprintln!("[SendToPlayer] GAME_PLAYERS not initialized.");
    }
}
