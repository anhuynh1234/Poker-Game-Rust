# Rust Multiplayer Poker Project 3 - GUI Client + Server

## Overview

This project implements a **multiplayer poker game system** written in Rust. It consists of:

- A **server crate** that handles game logic, player management, lobby coordination, and communication over TCP.
- A **player crate** (**GUI client crate**; built with `egui`) that lets players register, log in, select poker variants, join lobbies, ready up, and play a game.

All player interaction happens through the GUI. The server is headless and handles backend logic and state.

---

## Features

- GUI-based player gameplay
- Realtime multiplayer lobbies  
- Server-controlled game start via TCP  
- Custom deck, shuffle, and hand evaluation logic  
- MongoDB integration for player stats and game history data
- Viewable stats page for any and all users
- Five Card Draw, Seven card stud, texas hold 'em poker support  

---

## Requirements

Before running the server and client, make sure you have:

- **Rust & Cargo** ([Install here](https://rustup.rs))
- **MongoDB** ([Install here](https://www.mongodb.com/try/download/community))
- **Dependencies** (see below)

---

## Setup Instructions

### 1. Install Dependencies
From both the server and client directories:

### 2. Start MongoDB
Make sure MongoDB is running:
```mongod```

---

## Running the Project

### 1. Start the Server
In the server crate directory:
```cargo run```

The server will:
- Start listening on `0.0.0.0:8080`
- Handle incoming client connections
- Manage game logic and state

Output:
```Enter number of players [default = 2]:```
```
Select game variant:
1 - 5 Card Draw (default)
2 - 7 Card Stud
3 - Texas Hold'em
Enter choice [1-3]:
```
```
[Server] Connected to MongoDB and initialized collections.
Server listening on 172.28.225.7:8080
```
---

### 2. Start the GUI Client (Same or Different Device)
In the client crate directory:
```cargo run```

From here, the GUI client handles everything:
- Register a new account
- Log in
- Select a game type (one of the 3 we want)
- Enter a lobby and wait for others
- Click "Ready"
- Play the selected game once all players are ready

---

## Architecture

### Server Crate

- Runs a TCP server using `tokio`
- Tracks players and lobbies in shared memory (`Arc<Mutex<HashMap<...>>>`)
- Uses MongoDB for:
  - Player authentication and stats
  - Lobby management
  - Saving completed game history
- Sends JSON-formatted updates to clients via persistent TCP streams *c

### Client Crate (GUI)

- Built using `egui` and `eframe`
- Displays login/register screen, game selection UI, lobby status, and game view
- Maintains a TCP connection to the server
- Background threads handle:
  - Lobby polling
  - Receiving real-time broadcasts (e.g., `"game_start"`)
- Automatically changes UI based on server messages

---

## Game Flow
1. CLI to start a server with a specific amount of players, and to select a poker variant. 
1. Player opens the GUI and connects to the server.
2. Registers/logs in → Join the waiting room.
3. Joins a shared waiting room lobby with up to 6 players.
4. Users can also go the stats page here to view any user's stats, given their username is entered in the search bar.
5. Once the specific amount of players have joined. Game will start to the selected poker variant. 
6. Server deals cards and notifies all players via persistent TCP.
7. (*c) include full turn-based gameplay and betting.

---

## Poker Variant: Five Card Draw

- Each player receives 5 cards
- Evaluation uses custom ranking system
- Winner is determined and recorded in MongoDB
- Game state is reset after completion

## Poker Variant: Seven Card Stud
- Each player is dealt a total of 7 cards with a mix of face-down and face-up cards.
- Betting rounds occur as cards are progressively dealt.
- Each player forms the best 5-card hand from their 7 cards.
- Winner is determined at showdown and recorded in MongoDB.
- Game state is reset after completion.
  
## Poker Variant: Texas Hold `Em
- Each player receives 2 private (hole) cards.
- Five community cards are dealt in rounds (flop, turn, and river).
- The game begins with two forced bets: the small blind and the big blind.
  - Small Blind: A smaller forced bet that gets money into the pot right from the start.
  - Big Blind: A larger forced bet, further builds the initial pot.
- Best 5-card hand is formed using any combination of hole and community cards.
- Multiple betting rounds occur throughout the deal.
- Winner is determined at showdown and recorded in MongoDB.
- Game state is reset after completion.

## Betting
- **Check**: If no bet has been made, a player may choose to check, meaning they do not wager any additional chips but remain in the hand.
- **Bet/Call**: When a bet is placed, a player can call by matching the current bet to stay in the round.
- **Raise**: A player can increase the current bet, which forces other players to either match the new amount or fold.
- **Fold**: A player ca choose to fold, withdrawing from the current hand and forfeiting any chips already bet.


## References
put used crates or other resources here
---

