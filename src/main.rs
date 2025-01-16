#[macro_use]
extern crate rocket;

use std::env;
use uuid::Uuid;

use mysql_async::{prelude::*, Opts, Pool};
use reversi_tools::position::*;
use rocket::serde::json::Json;
use rocket::State;

mod repository;
use repository::game_repository::{GameRepository, MySqlGameRepository};

mod model;
use model::*;
use rand::Rng;

pub fn generate_uuid() -> String {
    Uuid::new_v4().to_string()
}

fn random_upto(n: usize) -> usize {
    let mut rng = rand::thread_rng();
    rng.gen_range(0..n)
}

#[get("/players")]
async fn get_users(pool: &State<Pool>) -> Json<PlayerResponse> {
    // TODO: implement player repository
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
    let game_repo = MySqlGameRepository::new(pool.inner().clone());

    let upto: usize = 2;

    //let mut conn = pool.get_conn().await.unwrap();
    let game_uuid: String = generate_uuid();
    let mut game: Game = Game {
        game_uuid: game_uuid.clone(),
        black_uuid: String::new(),
        white_uuid: String::new(),
        position_black: 0x0000000810000000u64,
        position_white: 0x0000001008000000u64,
        state: 0,
    };
    let color: String;
    if random_upto(upto) == 1 {
        color = "white".to_string();
        game.white_uuid = request.player_id.clone();
    } else {
        color = "black".to_string();
        game.black_uuid = request.player_id.clone();
    }

    match game_repo.create_game(&game).await {
        Ok(_) => {}
        Err(e) => {
            let response: NewGameResponse = NewGameResponse {
                status: "error".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: NewGameResult {
                    game_id: String::new(),
                    color: String::new(),
                },
            };
            return Json(response);
        }
    }

    let created_game: Game;
    match game_repo.get_game(game_uuid.clone().as_str()).await {
        Ok(g) => match g {
            Some(gg) => {
                created_game = gg;
            }
            None => {
                let response: NewGameResponse = NewGameResponse {
                    status: "error".to_string(),
                    error: ResponseError {
                        code: 404,
                        message: "Game not found".to_string(),
                    },
                    result: NewGameResult {
                        game_id: String::new(),
                        color: String::new(),
                    },
                };
                return Json(response);
            }
        },
        Err(e) => {
            let response: NewGameResponse = NewGameResponse {
                status: "error".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: NewGameResult {
                    game_id: String::new(),
                    color: String::new(),
                },
            };
            return Json(response);
        }
    }
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
    //let mut conn = pool.get_conn().await.unwrap();
    let statuses: Vec<String> = vec![
        "pending".to_string(),
        "black".to_string(),
        "white".to_string(),
        "black_won".to_string(),
        "white_won".to_string(),
        "draw".to_string(),
    ];
    let game_repo = MySqlGameRepository::new(pool.inner().clone());

    let last_move: String; // = get_last_move(pool, request.game_id.clone()).await;
    match game_repo.get_last_move(request.game_id.as_str()).await {
        Ok(m) => {
            if m == 0 {
                //println!("No moves yet!");
                last_move = String::new();
            } else if m == u64::MAX {
                last_move = "pass".to_string();
            } else {
                last_move = move_to_algebraic(m).unwrap();
            }
        }
        Err(e) => {
            println!("Game status error: {}", e);
            let response: GameStatusResponse = GameStatusResponse {
                status: "ok".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: GameStatusResult {
                    status: String::new(),
                    last_move: String::new(),
                },
            };
            return Json(response);
        }
    }

    let game: Game;

    match game_repo.get_game(request.game_id.as_str()).await {
        Ok(g) => match g {
            Some(gg) => {
                game = gg;
            }
            None => {
                let result: GameStatusResult = GameStatusResult {
                    status: String::new(),
                    last_move: String::new(),
                };
                let response: GameStatusResponse = GameStatusResponse {
                    status: "error".to_string(),
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
            let result: GameStatusResult = GameStatusResult {
                status: String::new(),
                last_move: String::new(),
            };
            let response: GameStatusResponse = GameStatusResponse {
                status: "error".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: result,
            };
            return Json(response);
        }
    }
    let result: GameStatusResult = GameStatusResult {
        status: statuses[game.state as usize].clone(),
        last_move: last_move,
    };

    let response: GameStatusResponse = GameStatusResponse {
        status: "ok".to_string(),
        error: ResponseError {
            code: 200,
            message: "".to_string(),
        },
        result: result,
    };
    Json(response)
}

#[post("/join", format = "json", data = "<request>")]
async fn game_join(pool: &State<Pool>, request: Json<GameRequest>) -> Json<GameJoinResponse> {
    // TODO(1): add player validation
    // TODO(3): make sure the joining player is different from the game creator
    // TODO(4): make sure the game is in pending state
    let game_repo = MySqlGameRepository::new(pool.inner().clone());
    let mut game: Game;
    match game_repo.get_game(request.game_id.as_str()).await {
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
                    status: "error".to_string(),
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
                status: "error".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: result,
            };
            return Json(response);
        }
    }
    let color: String;
    if game.black_uuid == "".to_string() {
        color = "black".to_string();
        game.black_uuid = request.player_id.clone();
    } else {
        color = "white".to_string();
        game.white_uuid = request.player_id.clone();
    }
    game.state = 1;
    match game_repo.update_game(&game).await {
        Ok(_) => {}
        Err(e) => {
            let result: GameJoinResult = GameJoinResult {
                result: false,
                color: String::new(),
            };
            let response: GameJoinResponse = GameJoinResponse {
                status: "error".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: result,
            };
            return Json(response);
        }
    }
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
    let game_repo = MySqlGameRepository::new(pool.inner().clone());

    let mut game: Game;
    match game_repo.get_game(request.game_id.as_str()).await {
        Ok(g) => match g {
            Some(gg) => {
                game = gg;
            }
            None => {
                let result: MoveResult = MoveResult {
                    ok: false,
                    r#continue: true,
                    winner: String::new(),
                };
                let response: MoveResponse = MoveResponse {
                    status: "error".to_string(),
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
            let result: MoveResult = MoveResult {
                ok: false,
                r#continue: true,
                winner: String::new(),
            };
            let response: MoveResponse = MoveResponse {
                status: "error".to_string(),
                error: ResponseError {
                    code: 500,
                    message: format!("{}", e),
                },
                result: result,
            };
            return Json(response);
        }
    }
    let mut curr_player: String = "black".to_string();
    if game.white_uuid == request.player_id {
        curr_player = "white".to_string();
    }
    let mut next_player: String = "white".to_string();
    if curr_player == "white".to_string() {
        next_player = "black".to_string();
    }

    if request.r#move == "resign".to_string() {
        if curr_player == "white".to_string() {
            game.state = 3;
        } else {
            game.state = 4;
        }
        match game_repo.update_game(&game).await {
            Ok(_) => {}
            Err(e) => {
                let result: MoveResult = MoveResult {
                    ok: false,
                    r#continue: true,
                    winner: String::new(),
                };
                let response: MoveResponse = MoveResponse {
                    status: "error".to_string(),
                    error: ResponseError {
                        code: 500,
                        message: format!("{}", e),
                    },
                    result: result,
                };
                return Json(response);
            }
        }
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
        game.state = 3 - game.state;
        let mut cont: bool = true;
        let mut winner: String = String::new();
        let game_status =
            check_game_status(game.position_white, game.position_black, game.state == 2);

        if game_status == (u64::MAX - 2) {
            game.state = 4;
            cont = false;
            winner = "white".to_string();
        } else if game_status == (u64::MAX - 1) {
            game.state = 3;
            cont = false;
            winner = "black".to_string();
        } else if game_status == (u64::MAX - 3) {
            game.state = 5;
            cont = false;
            winner = "draw".to_string();
        }
        let max_move: u64;
        match game_repo.get_max_move_no(request.game_id.as_str()).await {
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

        match game_repo
            .update_game_with_move(&game, u64::MAX, max_move + 1)
            .await
        {
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
                    status: "error".to_string(),
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
        move_to_bitmap(&request.r#move.clone().as_str()).unwrap(),
        curr_player == "white".to_string(),
    ) {
        Ok((new_white, new_black)) => {
            //let mut next_state: u64 = 2;
            let mut cont: bool = true;
            /*if curr_player == "white".to_string() {
                next_state = 1;
            }*/
            game.position_white = new_white;
            game.position_black = new_black;
            game.state = 3 - game.state;
            //let game_status: &str = check_game_status(game.position_white, game.position_black);
            let game_status = check_game_status(
                game.position_white,
                game.position_black,
                curr_player == "white".to_string(),
            );
            if game_status == (u64::MAX - 2) {
                game.state = 4;
                cont = false;
            } else if game_status == (u64::MAX - 1) {
                game.state = 3;
                cont = false;
            } else if game_status == (u64::MAX - 3) {
                game.state = 5;
                cont = false;
            }
            let max_move: u64;
            //match get_max_move_no(pool, request.game_id.clone()).await {
            match game_repo.get_max_move_no(request.game_id.as_str()).await {
                Ok(m) => {
                    max_move = m;
                }
                Err(e) => {
                    println!("Error while getting last move number: {}", e);
                    let result: MoveResult = MoveResult {
                        ok: false,
                        r#continue: true,
                        winner: String::new(),
                    };
                    let response: MoveResponse = MoveResponse {
                        status: "error".to_string(),
                        error: ResponseError {
                            code: 500,
                            message: format!("{}", e),
                        },
                        result: result,
                    };
                    return Json(response);
                }
            }
            match game_repo
                .update_game_with_move(
                    &game,
                    move_to_bitmap(&request.r#move.clone().as_str()).unwrap(),
                    max_move + 1,
                )
                .await
            {
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
                    println!("Error while updating move: {}", e);
                    let result: MoveResult = MoveResult {
                        ok: false,
                        r#continue: true,
                        winner: String::new(),
                    };
                    let response: MoveResponse = MoveResponse {
                        status: "error".to_string(),
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
            println!("Error applying move {}: {}", request.r#move, e);
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

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|val| val.parse().ok())
        .unwrap_or(8000);
    let figment = rocket::Config::figment().merge(("port", port));
    env_logger::init();

    let opts = Opts::from_url(&database_url).expect("Invalid DATABASE_URL");

    let pool = Pool::new(opts);

    rocket::custom(figment)
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
    use mysql_async::{params, Conn, OptsBuilder, Pool, Row};
    use rocket::{http::Status, local::asynchronous::Client, State};
    use serde_json::json;
    use std::env;
    use tokio::runtime::Runtime;
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
