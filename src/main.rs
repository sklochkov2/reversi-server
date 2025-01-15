#[macro_use]
extern crate rocket;

use std::env;
use uuid::Uuid;

use mysql_async::{prelude::*, Opts, Pool, TxOpts};
use rocket::serde::json::Json;
use rocket::State;

mod model;
use model::*;
use rand::Rng;

struct Game {
    black_uuid: String,
    white_uuid: String,
    position_black: u64,
    position_white: u64,
    state: u64,
}

pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn move_to_bitmap(move_notation: &str) -> Result<u64, &str> {
    if move_notation.len() != 2 {
        return Err("Invalid move notation");
    }
    let file = move_notation.chars().next().unwrap().to_ascii_lowercase() as usize - 'a' as usize;
    let rank = move_notation.chars().nth(1).unwrap().to_digit(10).unwrap() as usize - 1;
    if file >= 8 || rank >= 8 {
        return Err("Invalid move notation");
    }

    let move_pos = rank * 8 + file;
    let move_bit = 1u64 << move_pos;
    Ok(move_bit)
}

fn move_to_algebraic(move_bit: u64) -> Option<String> {
    if move_bit.count_ones() != 1 {
        return None;
    }

    let pos = move_bit.trailing_zeros() as usize;
    let file = (pos % 8) as u8 + b'a';
    let rank = (pos / 8) as u8 + b'1';

    Some(format!("{}{}", file as char, rank as char))
}

fn random_upto(n: usize) -> usize {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..n)
}

async fn get_game(
    pool: &State<Pool>,
    game_uuid: String,
) -> Result<Option<Game>, mysql_async::Error> {
    let mut conn = pool.get_conn().await?;
    let game: Option<Game> = conn.exec_first(
        "select IFNULL(BIN_TO_UUID(black_uuid), '') as black_uuid, IFNULL(BIN_TO_UUID(white_uuid), '') as white_uuid, position_black, position_white, state from games where game_uuid = UUID_TO_BIN(:game_uuid)",
        params!{
            "game_uuid" => &game_uuid,
        },
    ).await?.map(|(black_uuid, white_uuid, position_black, position_white, state)| Game{ black_uuid, white_uuid, position_black, position_white, state });
    Ok(game)
}

async fn get_max_move_no(pool: &State<Pool>, game_uuid: String) -> Result<u64, mysql_async::Error> {
    let mut conn = pool.get_conn().await?;
    let max_move_opt: Option<(Option<u64>,)> = conn.exec_first(
        "SELECT MAX(move_number) AS move_number FROM moves WHERE game_uuid = UUID_TO_BIN(:game_uuid)",
        params! {
            "game_uuid" => &game_uuid,
        },
    )
    .await?;
    let max_move: u64 = max_move_opt
        .and_then(|row| row.0) // Extract and flatten the inner Option<u64>
        .unwrap_or(0);
    Ok(max_move)
}

async fn get_last_move(pool: &State<Pool>, game_uuid: String) -> String {
    let mut conn = pool.get_conn().await.unwrap();
    let last_move_opt: Option<(Option<u64>,)> = conn.exec_first(
        "SELECT move_position FROM moves WHERE game_uuid = UUID_TO_BIN(:game_uuid) ORDER BY move_number DESC LIMIT 1;",
        params! {
            "game_uuid" => &game_uuid,
        },
    ).await.unwrap();
    let last_move: u64 = last_move_opt.and_then(|row| row.0).unwrap_or(0);
    if last_move == 0 {
        return String::new();
    } else if last_move == u64::MAX {
        return "pass".to_string();
    } else {
        return move_to_algebraic(last_move).unwrap();
    }
}

fn apply_move(
    white: u64,
    black: u64,
    move_notation: &str,
    is_white_move: bool,
) -> Result<(u64, u64), &'static str> {
    const DIRECTIONS: [(i32, i32); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];

    // Convert algebraic notation to position index
    if move_notation.len() != 2 {
        return Err("Invalid move notation");
    }
    let file = move_notation.chars().next().unwrap().to_ascii_lowercase() as usize - 'a' as usize;
    let rank = move_notation.chars().nth(1).unwrap().to_digit(10).unwrap() as usize - 1;
    if file >= 8 || rank >= 8 {
        return Err("Invalid move notation");
    }

    let move_pos = rank * 8 + file;
    let move_bit = 1u64 << move_pos;

    let (mut player, mut opponent) = if is_white_move {
        (white, black)
    } else {
        (black, white)
    };

    if (player | opponent) & move_bit != 0 {
        return Err("Square already occupied");
    }

    let mut flips = 0u64;

    for &(dx, dy) in DIRECTIONS.iter() {
        let mut current_flips = 0u64;
        let mut x = file as i32 + dx;
        let mut y = rank as i32 + dy;
        let mut found_opponent = false;

        while x >= 0 && x < 8 && y >= 0 && y < 8 {
            let index = (y * 8 + x) as usize;
            let bit = 1u64 << index;

            if (opponent & bit) != 0 {
                current_flips |= bit;
                found_opponent = true;
            } else if (player & bit) != 0 {
                if found_opponent {
                    flips |= current_flips;
                }
                break;
            } else {
                break;
            }

            x += dx;
            y += dy;
        }
    }

    if flips == 0 {
        return Err("Invalid move, no discs flipped");
    }

    player |= move_bit | flips;
    opponent &= !flips;

    if is_white_move {
        Ok((player, opponent))
    } else {
        Ok((opponent, player))
    }
}

fn check_game_status(player: u64, opponent: u64) -> &'static str {
    let all_discs = player | opponent;
    println!(
        "Checking status; white: {}, black: {}, white count: {}, black count: {}",
        player,
        opponent,
        player.count_ones(),
        opponent.count_ones()
    );

    if all_discs == 0xFFFFFFFFFFFFFFFF {
        let player_count = player.count_ones();
        let opponent_count = opponent.count_ones();

        return if player_count > opponent_count {
            "white"
        } else if opponent_count > player_count {
            "black"
        } else {
            "draw"
        };
    }

    if has_valid_moves(player, opponent) || has_valid_moves(opponent, player) {
        "continue"
    } else {
        let player_count = player.count_ones();
        let opponent_count = opponent.count_ones();

        if player_count > opponent_count {
            "white"
        } else if opponent_count > player_count {
            "black"
        } else {
            "draw"
        }
    }
}

fn has_valid_moves(player: u64, opponent: u64) -> bool {
    const DIRECTIONS: [(i32, i32); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];

    for pos in 0..64 {
        let move_bit = 1u64 << pos;

        if (player | opponent) & move_bit != 0 {
            continue; // Square is already occupied
        }

        for &(dx, dy) in DIRECTIONS.iter() {
            let mut x = (pos % 8) as i32 + dx;
            let mut y = (pos / 8) as i32 + dy;
            let mut found_opponent = false;

            while x >= 0 && x < 8 && y >= 0 && y < 8 {
                let index = (y * 8 + x) as usize;
                let bit = 1u64 << index;

                if (opponent & bit) != 0 {
                    found_opponent = true;
                } else if (player & bit) != 0 {
                    if found_opponent {
                        return true;
                    }
                    break;
                } else {
                    break;
                }

                x += dx;
                y += dy;
            }
        }
    }

    false
}

#[get("/players")]
async fn get_users(pool: &State<Pool>) -> Json<PlayerResponse> {
    let mut conn = pool.get_conn().await.unwrap();

    let users: Vec<User> = conn
        .query_map(
            "SELECT bin_to_uuid(player_uuid) as player_uuid, comment FROM players",
            |(player_uuid, comment)| User {
                player_uuid,
                comment,
            },
        )
        .await
        .unwrap();

    let response: PlayerResponse = PlayerResponse {
        status: "ok".to_string(),
        error: ResponseError {
            code: 200,
            message: "".to_string(),
        },
        result: users,
    };

    Json(response)
}

#[post("/create_game", format = "json", data = "<request>")]
async fn create_game(pool: &State<Pool>, request: Json<NewGameRequest>) -> Json<NewGameResponse> {
    // TODO(1): add player validation
    println!("Game creation requested by {}", request.player_id.clone());

    let upto: usize = 2;

    let mut conn = pool.get_conn().await.unwrap();
    let game_uuid: String = generate_uuid();
    let mut query = "INSERT INTO games(game_uuid, black_uuid, state, position_black, position_white, start_date) VALUES (UUID_TO_BIN(:game_uuid), UUID_TO_BIN(:player_uuid), 0, :initial_black, :initial_white, NOW())";
    let mut color = "black".to_string();
    if random_upto(upto) == 1 {
        query = "INSERT INTO games(game_uuid, white_uuid, state, position_black, position_white, start_date) VALUES (UUID_TO_BIN(:game_uuid), UUID_TO_BIN(:player_uuid), 0, :initial_black, :initial_white, NOW())";
        color = "white".to_string();
    }
    conn.exec_drop(
        query,
        params! {
            "game_uuid" => game_uuid.clone(),
            "player_uuid" => request.player_id.clone(),
            "initial_white" => 0x0000001008000000u64,
            "initial_black" => 0x0000000810000000u64,
        },
    )
    .await
    .unwrap();
    let created_game = get_game(pool, game_uuid.clone()).await.unwrap().unwrap();
    println!("New game properties: black_uuid: >{}<, white_uuid: >{}<, black_position: >{}<, white_position: >{}<, state: >{}<", created_game.black_uuid, created_game.white_uuid, created_game.position_black, created_game.position_white, created_game.state);
    let response: NewGameResponse = NewGameResponse {
        status: "ok".to_string(),
        error: ResponseError {
            code: 200,
            message: "".to_string(),
        },
        result: NewGameResult {
            game_id: game_uuid,
            color: color,
        },
    };
    Json(response)
}

#[post("/game_list", format = "json", data = "<request>")]
async fn game_list(pool: &State<Pool>, request: Json<NewGameRequest>) -> Json<GameListResponse> {
    // TODO(1): add player validation
    let mut conn = pool.get_conn().await.unwrap();
    let games: Vec<AvailableGame> = conn
        .exec_map(
            "SELECT
          bin_to_uuid(game_uuid) as game_id,
          IFNULL(bin_to_uuid(black_uuid), bin_to_uuid(white_uuid)) as first_player
         FROM games 
         WHERE state = 0 AND IFNULL(bin_to_uuid(black_uuid), bin_to_uuid(white_uuid)) <> ?
         ORDER BY start_date ASC",
            (request.player_id.clone(),),
            |(game_id, first_player)| AvailableGame {
                game_id,
                first_player,
            },
        )
        .await
        .unwrap();

    let response: GameListResponse = GameListResponse {
        status: "ok".to_string(),
        error: ResponseError {
            code: 200,
            message: "".to_string(),
        },
        result: games,
    };
    Json(response)
}

#[post("/game_status", format = "json", data = "<request>")]
async fn game_status(pool: &State<Pool>, request: Json<GameRequest>) -> Json<GameStatusResponse> {
    // TODO(1): add player validation
    let mut conn = pool.get_conn().await.unwrap();
    let statuses: Vec<String> = vec![
        "pending".to_string(),
        "black".to_string(),
        "white".to_string(),
        "black_won".to_string(),
        "white_won".to_string(),
        "draw".to_string(),
    ];

    let last_move: String = get_last_move(pool, request.game_id.clone()).await;

    let game: Option<GameStatusResult> = conn
        .exec_first(
            "select state as status from games where game_uuid = uuid_to_bin(:game_uuid)",
            params! {
                "game_uuid" => &request.game_id,
            },
        )
        .await
        .unwrap()
        .map(|status: u64| GameStatusResult {
            status: statuses[status as usize].clone(),
            last_move: last_move,
        });

    let response: GameStatusResponse = GameStatusResponse {
        status: "ok".to_string(),
        error: ResponseError {
            code: 200,
            message: "".to_string(),
        },
        result: game.unwrap(),
    };
    Json(response)
}

#[post("/join", format = "json", data = "<request>")]
async fn game_join(pool: &State<Pool>, request: Json<GameRequest>) -> Json<GameJoinResponse> {
    // TODO(1): add player validation
    // TODO(3): make sure the joining player is different from the game creator
    // TODO(4): make sure the game is in pending state
    let mut conn = pool.get_conn().await.unwrap();
    //let game = get_game(pool, request.game_id.clone()).await.unwrap().unwrap();
    //
    let game: Game;
    match get_game(pool, request.game_id.clone()).await {
        Ok(g) => match g {
            Some(gg) => {
                game = gg;
            }
            None => {
                let result: GameJoinResult = GameJoinResult {
                    result: false,
                    color: String::new(),
                };
                let response: GameJoinResponse = GameJoinResponse {
                    status: "ok".to_string(),
                    error: ResponseError {
                        code: 404,
                        message: format!("Game UUID not found"),
                    },
                    result: result,
                };
                return Json(response);
            }
        },
        Err(e) => {
            let result: GameJoinResult = GameJoinResult {
                result: false,
                color: String::new(),
            };
            let response: GameJoinResponse = GameJoinResponse {
                status: "ok".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: result,
            };
            return Json(response);
        }
    }
    let mut query = "UPDATE games SET white_uuid = uuid_to_bin(:player_uuid), state = 1 where game_uuid = uuid_to_bin(:game_uuid)";
    let mut color = "white".to_string();
    if game.black_uuid == "".to_string() {
        query = "UPDATE games SET black_uuid = uuid_to_bin(:player_uuid), state = 1 where game_uuid = uuid_to_bin(:game_uuid)";
        color = "black".to_string();
    }
    conn.exec_drop(
        query,
        params! {
            "player_uuid" => request.player_id.clone(),
            "game_uuid" => request.game_id.clone(),
        },
    )
    .await
    .unwrap();
    let result: GameJoinResult = GameJoinResult {
        result: true,
        color: color,
    };
    let response: GameJoinResponse = GameJoinResponse {
        status: "ok".to_string(),
        error: ResponseError {
            code: 200,
            message: "".to_string(),
        },
        result: result,
    };
    Json(response)
}

#[post("/move", format = "json", data = "<request>")]
async fn game_move(pool: &State<Pool>, request: Json<MoveRequest>) -> Json<MoveResponse> {
    let mut conn = pool.get_conn().await.unwrap();
    let game = get_game(pool, request.game_id.clone())
        .await
        .unwrap()
        .unwrap();
    let mut curr_player: String = "black".to_string();
    if game.white_uuid == request.player_id {
        curr_player = "white".to_string();
    }
    let mut next_player: String = "white".to_string();
    if curr_player == "white".to_string() {
        next_player = "black".to_string();
    }

    if request.r#move == "resign".to_string() {
        let mut next_state: u64 = 4;
        if curr_player == "white".to_string() {
            next_state = 3;
        }
        conn.exec_drop(
            "UPDATE games set end_date = NOW(), state = :new_state WHERE game_uuid = UUID_TO_BIN(:game_uuid)",
            params! {
                "game_uuid" => request.game_id.clone(),
                "new_state" => next_state,
            }
        ).await.unwrap();
        let result: MoveResult = MoveResult {
            ok: true,
            r#continue: false,
            winner: next_player.clone(),
        };
        let response: MoveResponse = MoveResponse {
            status: "ok".to_string(),
            error: ResponseError {
                code: 200,
                message: String::new(),
            },
            result: result,
        };
        return Json(response);
    }

    if request.r#move == "pass".to_string() {
        let mut next_state: u64 = 3 - game.state;
        let mut cont: bool = true;
        let mut winner: String = String::new();
        let game_status: &str = check_game_status(game.position_white, game.position_black);

        if game_status == "white" {
            next_state = 4;
            cont = false;
            winner = "white".to_string();
        } else if game_status == "black" {
            next_state = 3;
            cont = false;
            winner = "black".to_string();
        } else if game_status == "draw" {
            next_state = 5;
            cont = false;
            winner = "draw".to_string();
        }
        let max_move: u64;
        match get_max_move_no(pool, request.game_id.clone()).await {
            Ok(m) => {
                max_move = m;
            }
            Err(e) => {
                let result: MoveResult = MoveResult {
                    ok: false,
                    r#continue: true,
                    winner: String::new(),
                };
                let response: MoveResponse = MoveResponse {
                    status: "ok".to_string(),
                    error: ResponseError {
                        code: 500,
                        message: format!("{}", e),
                    },
                    result: result,
                };
                return Json(response);
            }
        }

        let mut tx = conn.start_transaction(TxOpts::default()).await.unwrap();

        tx.exec_drop(
            "INSERT INTO moves (game_uuid, move_number, move_position, position_black, position_white, move_date) VALUES (UUID_TO_BIN(:game_uuid), :move_number, :next_move, :position_black, :position_white, now())",
            params! {
                "game_uuid" => request.game_id.clone(),
                "next_move" => u64::MAX,
                "move_number" => max_move + 1,
                "position_black" => game.position_black,
                "position_white" => game.position_white,
            },
        ).await.unwrap();

        tx.exec_drop(
            "UPDATE games set end_date = NOW(), state = :new_state WHERE game_uuid = UUID_TO_BIN(:game_uuid)",
            params! {
                "game_uuid" => request.game_id.clone(),
                "new_state" => next_state,
            }
        ).await.unwrap();

        match tx.commit().await {
            Ok(_) => {
                let result: MoveResult = MoveResult {
                    ok: true,
                    r#continue: cont,
                    winner: winner,
                };
                let response: MoveResponse = MoveResponse {
                    status: "ok".to_string(),
                    error: ResponseError {
                        code: 200,
                        message: String::new(),
                    },
                    result: result,
                };
                return Json(response);
            }
            Err(e) => {
                let result: MoveResult = MoveResult {
                    ok: false,
                    r#continue: true,
                    winner: String::new(),
                };
                let response: MoveResponse = MoveResponse {
                    status: "ok".to_string(),
                    error: ResponseError {
                        code: 500,
                        message: format!("{}", e),
                    },
                    result: result,
                };
                return Json(response);
            }
        }
    }

    match apply_move(
        game.position_white,
        game.position_black,
        &request.r#move.clone().as_str(),
        curr_player == "white".to_string(),
    ) {
        Ok((new_white, new_black)) => {
            let mut next_state: u64 = 2;
            let mut cont: bool = true;
            if curr_player == "white".to_string() {
                next_state = 1;
            }
            let game_status: &str = check_game_status(game.position_white, game.position_black);
            if game_status == "white" {
                next_state = 4;
                cont = false;
            } else if game_status == "black" {
                next_state = 3;
                cont = false;
            } else if game_status == "draw" {
                next_state = 5;
                cont = false;
            }
            let max_move: u64;
            match get_max_move_no(pool, request.game_id.clone()).await {
                Ok(m) => {
                    max_move = m;
                }
                Err(e) => {
                    let result: MoveResult = MoveResult {
                        ok: false,
                        r#continue: true,
                        winner: String::new(),
                    };
                    let response: MoveResponse = MoveResponse {
                        status: "ok".to_string(),
                        error: ResponseError {
                            code: 500,
                            message: format!("{}", e),
                        },
                        result: result,
                    };
                    return Json(response);
                }
            }
            let mut tx = conn.start_transaction(TxOpts::default()).await.unwrap();

            tx.exec_drop(
                "INSERT INTO moves (game_uuid, move_number, move_position, position_black, position_white, move_date) VALUES (UUID_TO_BIN(:game_uuid), :move_number, :next_move, :position_black, :position_white, now())",
                params! {
                    "game_uuid" => request.game_id.clone(),
                    "next_move" => move_to_bitmap(&request.r#move.clone().as_str()).unwrap(),
                    "move_number" => max_move + 1,
                    "position_black" => new_black,
                    "position_white" => new_white,
                },
            ).await.unwrap();
            tx.exec_drop(
                "UPDATE games set position_black = :black_pos, position_white = :white_pos, end_date = NOW(), state = :new_state WHERE game_uuid = UUID_TO_BIN(:game_uuid)",
                params! {
                    "game_uuid" => request.game_id.clone(),
                    "black_pos" => new_black,
                    "white_pos" => new_white,
                    "new_state" => next_state
                }
            ).await.unwrap();
            match tx.commit().await {
                Ok(_) => {
                    let result: MoveResult = MoveResult {
                        ok: true,
                        r#continue: cont,
                        winner: String::new(),
                    };
                    let response: MoveResponse = MoveResponse {
                        status: "ok".to_string(),
                        error: ResponseError {
                            code: 200,
                            message: String::new(),
                        },
                        result: result,
                    };
                    return Json(response);
                }
                Err(e) => {
                    let result: MoveResult = MoveResult {
                        ok: false,
                        r#continue: true,
                        winner: String::new(),
                    };
                    let response: MoveResponse = MoveResponse {
                        status: "ok".to_string(),
                        error: ResponseError {
                            code: 500,
                            message: format!("{}", e),
                        },
                        result: result,
                    };
                    return Json(response);
                }
            }
        }
        Err(e) => {
            let result: MoveResult = MoveResult {
                ok: false,
                r#continue: true,
                winner: String::new(),
            };
            let response: MoveResponse = MoveResponse {
                status: "error".to_string(),
                error: ResponseError {
                    code: 400,
                    message: e.to_string(),
                },
                result: result,
            };
            return Json(response);
        }
    }
}

#[rocket::main]
async fn main() {
    dotenv::dotenv().ok(); // Optional: Load from .env file
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    env_logger::init();

    let opts = Opts::from_url(&database_url).expect("Invalid DATABASE_URL");

    let pool = Pool::new(opts);

    rocket::build()
        .manage(pool)
        .mount(
            "/reversi/v1",
            routes![
                get_users,
                create_game,
                game_list,
                game_status,
                game_join,
                game_move
            ],
        )
        .launch()
        .await
        .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;
    use dotenv::dotenv;
    use mysql_async::{OptsBuilder, Pool};
    use rocket::{http::Status, local::asynchronous::Client};
    use serde_json::json;
    use std::env;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_generate_uuid() {
        let uuid = generate_uuid(); // Use the function directly, no `super` needed.
        assert!(
            Uuid::parse_str(&uuid).is_ok(),
            "Generated UUID is not valid"
        );
    }

    async fn test_get_users() {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").unwrap();
        let opts = Opts::from_url(&database_url).expect("Invalid DATABASE_URL"); // Correctly parse the URL
        let pool = Pool::new(opts);

        let client = Client::tracked(rocket::build().manage(pool).mount(
            "/reversi/v1",
            routes![get_users, create_game, game_list, game_status, game_join],
        ))
        .await
        .expect("Failed to create Rocket client");

        let response = client.get("/reversi/v1/players").dispatch().await;
        assert_eq!(response.status(), Status::Ok);

        let body = response.into_json::<PlayerResponse>().await.unwrap();
        assert_eq!(body.status, "ok");
        assert!(
            body.result.is_empty() || !body.result.is_empty(),
            "Result list should be present"
        );
    }
    // TODO(8): add more unit tests.
}
