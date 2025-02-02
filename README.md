# reversi-server

A simple Rust server that hosts and manages Reversi (Othello) games for multiple players.

## Overview

This server allows players to create or join Reversi games, make moves, and retrieve the current game status. It uses an asynchronous Rust framework and maintains game data (e.g., board state, moves, player information) in a database. Each request is handled independently and updates the underlying database accordingly, ensuring consistency and concurrency across multiple games.

## Build

To build the server in release mode, run:
```bash
cargo build --release
```

Optionally, you can build in debug mode by omitting `--release`:
```bash
cargo build
```

## Server Internals

- **Language and Framework**: The server is written in Rust, using an asynchronous runtime (Tokio). It uses a MySQL (or MariaDB) database for persistent storage of games, moves, and player IDs.
- **Database Schema**: It stores each game’s unique UUID, the participating players, moves, and the current state (whose turn it is, whether the game is ongoing, or if it’s finished).
- **Concurrency**: Each incoming request is processed asynchronously, allowing multiple games and moves to be handled in parallel without blocking.
- **Game Logic**: The server enforces Reversi-specific rules like valid moves, capturing discs, and game completion. Moves submitted by the client are validated both for proper formatting (e.g., `<letter><number>`) and rule compliance (capturing opponent discs).

## API

All endpoints are accessed via `POST` (except where noted) and expect/return JSON. Every response follows this structure:
```json
{
  "status": "ok", 
  "error": {},
  "result": {}
}
```
- `status`: Usually `"ok"` if there were no errors; otherwise, it might indicate `"error"`.
- `error`: An object containing error details if `status` is not `"ok"`.
- `result`: Contains the actual data returned by the endpoint.

> **Note**: Some endpoints return a simplified JSON structure (e.g., `{"ok": "true"}`). These still follow the general success/error pattern but omit unused fields for brevity.

| **Request URI**          | **Method** | **Request JSON**                                                        | **Result JSON**                                                                      |
|--------------------------|------------|--------------------------------------------------------------------------|---------------------------------------------------------------------------------------|
| `/reversi/v1/create_game`| POST       | `{"player_id": "<uuid>"}`                                               | `{"game_id": "<uuid>", "color": "white"/"black"}`<br/>Creates a new game and assigns the requesting player to either white or black. |
| `/reversi/v1/game_list`  | POST       | `{"player_id": "<uuid>"}`                                               | `[{"game_id": "<uuid>", "first_player": "<uuid>"}]`<br/>Returns a list of available or ongoing games, including the ID of the first player. |
| `/reversi/v1/game_status`| POST       | `{"player_id": "<uuid>", "game_id": "<uuid>"}`                          | `{"status": "pending"/"white"/"black"/"white_won"/"black_won", "last_move": "<move or empty string>"}`<br/>Provides the current state of the game, whose turn it is, and the last move made (if any). |
| `/reversi/v1/move`       | POST       | `{"player_id": "<uuid>", "game_id": "<uuid>", "move": "<letter><number>"}`<br/>or<br/>`{"player_id": "<uuid>", "game_id": "<uuid>", "move": "resign/pass"}` | `{"ok": "true","continue": "true","winner": ""}`<br/>Executes a move, which may be a board move, a resignation, or a pass. Indicates if the move is valid, if the game continues, and if there is a winner. |
| `/reversi/v1/join`       | POST       | `{"player_id": "<uuid>", "game_id": "<uuid>"}`                          | `{"result": <bool>, "color": "white"/"black"}`<br/>Attempts to join an existing game. `result` is `true` if join was successful, and `color` indicates the side assigned. |

### Endpoint Details

#### 1. **Create Game**
- **Purpose**: Creates a new Reversi game session for the requesting player.
- **Request**:
  ```json
  {"player_id": "<uuid>"}
  ```
- **Response**:
  ```json
  {
    "game_id": "<uuid>",
    "color": "white"|"black"
  }
  ```

#### 2. **Game List**
- **Purpose**: Fetch a list of available or ongoing Reversi games.
- **Request**:
  ```json
  {"player_id": "<uuid>"}
  ```
- **Response**:
  ```json
  [
    {"game_id": "<uuid>", "first_player": "<uuid>"}
  ]
  ```

#### 3. **Game Status**
- **Purpose**: Retrieve the current state and the last move made in a particular game.
- **Request**:
  ```json
  {
    "player_id": "<uuid>",
    "game_id": "<uuid>"
  }
  ```
- **Response**:
  ```json
  {
    "status": "pending"|"white"|"black"|"white_won"|"black_won",
    "last_move": "<empty string or last algebraic move>"
  }
  ```

#### 4. **Move**
- **Purpose**: Make a move in an existing game or take a special action (`resign` or `pass`).
- **Request**:
  ```json
  {
    "player_id": "<uuid>",
    "game_id": "<uuid>",
    "move": "<letter><number>"|"resign"|"pass"
  }
  ```
- **Response**:
  ```json
  {
    "ok": "true",
    "continue": "true",
    "winner": "<uuid or empty string>"
  }
  ```
  - `"continue"` indicates if the game should continue or has ended.
  - `"winner"` is set if the game ends immediately after the move.

#### 5. **Join**
- **Purpose**: Join an existing game if it is awaiting a second player.
- **Request**:
  ```json
  {
    "player_id": "<uuid>",
    "game_id": "<uuid>"
  }
  ```
- **Response**:
  ```json
  {
    "result": <bool>,
    "color": "white"|"black"
  }
  ```
  - `"result"` is `true` if the join was successful and `false` otherwise.
  - `"color"` indicates the side assigned to the player if successful.

---

## Running the Server

After building, run the server binary (for example, `./target/release/reversi-server`). Make sure the required environment variables (like `DATABASE_URL`) are set if your server relies on them. Once running, you can send requests to the server’s endpoints (e.g., `http://localhost:8000/reversi/v1/...`).
