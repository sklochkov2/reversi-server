# reversi-server
A simple server which can host a reversi game

## Build

```bash
cargo build --release
```

## API

The basic response structure looks like the following:
```json
{"status": "ok", error: {}, 
  result: {
    
  }
}

```

|Request URI             |Method|Request JSON                 |Result JSON                    |
|------------------------|------|-----------------------------|-------------------------------|
|/reversi/v1/create_game |POST  |`{"player_id": "<uuid>"}`    |`{"game_id": "<uuid>", "color": "white|black"}`|
|/reversi/v1/game_list|POST|`{"player_id": "<uuid>"}`|`{"game_list": [{"game_id": "<uuid>","first_player": "<uuid>"}]`|
|/reversi/v1/game_status|POST|`{"player_id": "<uuid>", "game_id": "<uuid>"}`|`{"Status": enum("pending","white", "black", "white_won", "black_won")}`|
|/reversi/v1/move|POST|`{"player_id": "<uuid>", "game_id": "<uuid>", "move": "<letter><number>"}` or `{"player_id": "<uuid>", "game_id": "<uuid>", "move": "resign"}`|`{"ok": "true","continue": "true","winner": ""}`|
|/reversi/v1/join|POST|`{"player_id": "<uuid>", "game_id": "<uuid>"}`|`{"ok": bool, "color": "white|black"}`|

## TODO

* Implement tokio HTTP configuration loading from environment variables.
* Implement request parameter validation.
* Implement random color selection.
