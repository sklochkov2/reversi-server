use crate::model::Game;
use async_trait::async_trait;
use mysql_async::{params, prelude::*, Pool, TxOpts};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("Other error: {0}")]
    Other(String),
}

#[async_trait]
pub trait GameRepository {
    async fn pending_games(&self, player_uuid: String) -> Result<Vec<Game>, RepositoryError>;
    async fn get_game(&self, game_uuid: &str) -> Result<Option<Game>, RepositoryError>;
    async fn get_max_move_no(&self, game_uuid: &str) -> Result<u64, RepositoryError>;
    async fn get_last_move(&self, game_uuid: &str) -> Result<u64, RepositoryError>;
    async fn create_game(&self, game: &Game) -> Result<(), RepositoryError>;
    async fn update_game(&self, game: &Game) -> Result<(), RepositoryError>;
    async fn update_game_with_move(
        &self,
        game: &Game,
        move_bit: u64,
        move_no: u64,
    ) -> Result<(), RepositoryError>;
}

pub struct MySqlGameRepository {
    pool: Pool,
}

#[async_trait]
impl GameRepository for MySqlGameRepository {
    async fn get_game(&self, game_uuid: &str) -> Result<Option<Game>, RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let game: Option<Game> = conn
            .exec_first(
                r#"
            SELECT
                BIN_TO_UUID(game_uuid) AS game_uuid,
                IFNULL(BIN_TO_UUID(black_uuid), '') AS black_uuid,
                IFNULL(BIN_TO_UUID(white_uuid), '') AS white_uuid,
                position_black,
                position_white,
                state
            FROM games
            WHERE game_uuid = UUID_TO_BIN(:game_uuid)
            "#,
                params! {
                    "game_uuid" => game_uuid,
                },
            )
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?
            .map(
                |(game_uuid, black_uuid, white_uuid, position_black, position_white, state)| Game {
                    game_uuid,
                    black_uuid,
                    white_uuid,
                    position_black,
                    position_white,
                    state,
                },
            );

        Ok(game)
    }

    async fn get_max_move_no(&self, game_uuid: &str) -> Result<u64, RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let max_move_opt: Option<(Option<u64>,)> = conn
            .exec_first(
                r#"
                SELECT MAX(move_number) AS move_number
                FROM moves
                WHERE game_uuid = UUID_TO_BIN(:game_uuid)
                "#,
                params! {
                    "game_uuid" => game_uuid,
                },
            )
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        let max_move: u64 = max_move_opt
            .and_then(|(move_num_opt,)| move_num_opt)
            .unwrap_or(0);

        Ok(max_move)
    }

    async fn get_last_move(&self, game_uuid: &str) -> Result<u64, RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let last_move_opt: Option<(Option<u64>,)> = conn.exec_first(
            "SELECT move_position FROM moves WHERE game_uuid = UUID_TO_BIN(:game_uuid) ORDER BY move_number DESC LIMIT 1;",
            params! {
                "game_uuid" => &game_uuid,
            },
        ).await.map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let last_move: Option<u64> = last_move_opt.and_then(|(move_num_opt,)| move_num_opt);
        match last_move {
            Some(m) => {
                return Ok(m);
            }
            None => {
                return Ok(0);
            }
        }
    }

    async fn create_game(&self, game: &Game) -> Result<(), RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        conn.exec_drop(
            r#"
            INSERT INTO games (
                game_uuid,
                black_uuid,
                white_uuid,
                position_black,
                position_white,
                state,
                start_date
            )
            VALUES (
                UUID_TO_BIN(:game_uuid),
                IF(:black_uuid = '', NULL, UUID_TO_BIN(:black_uuid)),
                IF(:white_uuid = '', NULL, UUID_TO_BIN(:white_uuid)),
                :position_black,
                :position_white,
                :state,
                NOW()
            )
            "#,
            params! {
                "game_uuid" => &game.game_uuid,
                "black_uuid" => &game.black_uuid,
                "white_uuid" => &game.white_uuid,
                "position_black" => game.position_black,
                "position_white" => game.position_white,
                "state" => game.state,
            },
        )
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_game(&self, game: &Game) -> Result<(), RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        conn.exec_drop(
            r#"
            UPDATE games
            SET
                black_uuid = UUID_TO_BIN(:black_uuid),
                white_uuid = UUID_TO_BIN(:white_uuid),
                position_black = :position_black,
                position_white = :position_white,
                state = :state,
                end_date = NOW()
            WHERE game_uuid = UUID_TO_BIN(:game_uuid)
            "#,
            params! {
                "game_uuid" => &game.game_uuid,
                "black_uuid" => &game.black_uuid,
                "white_uuid" => &game.white_uuid,
                "position_black" => game.position_black,
                "position_white" => game.position_white,
                "state" => game.state,
            },
        )
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn update_game_with_move(
        &self,
        game: &Game,
        move_bit: u64,
        move_no: u64,
    ) -> Result<(), RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let mut tx = conn
            .start_transaction(TxOpts::default())
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        tx.exec_drop(
            r#"
            UPDATE games
            SET
                black_uuid = UUID_TO_BIN(:black_uuid),
                white_uuid = UUID_TO_BIN(:white_uuid),
                position_black = :position_black,
                position_white = :position_white,
                state = :state,
                end_date = NOW()
            WHERE game_uuid = UUID_TO_BIN(:game_uuid)
            "#,
            params! {
                "game_uuid" => &game.game_uuid,
                "black_uuid" => &game.black_uuid,
                "white_uuid" => &game.white_uuid,
                "position_black" => game.position_black,
                "position_white" => game.position_white,
                "state" => game.state,
            },
        )
        .await
        .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        tx.exec_drop(
            r#"
            INSERT INTO moves (
                game_uuid,
                move_number,
                move_position,
                position_black,
                position_white,
                move_date
            ) VALUES (UUID_TO_BIN(:game_uuid), :move_number, :next_move, :position_black, :position_white, NOW())
            "#,
            params! {
                "game_uuid" => &game.game_uuid,
                "next_move" => move_bit,
                "move_number" => move_no,
                "position_black" => game.position_black,
                "position_white" => game.position_white,
            },
            ).await.map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(())
    }

    async fn pending_games(&self, player_uuid: String) -> Result<Vec<Game>, RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let games: Vec<Game> = conn
            .exec_map(
                "SELECT
                bin_to_uuid(game_uuid) as game_uuid,
                IFNULL(BIN_TO_UUID(black_uuid), '') AS black_uuid,
                IFNULL(BIN_TO_UUID(white_uuid), '') AS white_uuid,
                position_black,
                position_white,
                state
            FROM games
            WHERE state = 0 AND IFNULL(bin_to_uuid(black_uuid), bin_to_uuid(white_uuid)) <> ?
            ORDER BY start_date ASC",
                (player_uuid.clone(),),
                |(game_uuid, black_uuid, white_uuid, position_black, position_white, state)| Game {
                    game_uuid,
                    black_uuid,
                    white_uuid,
                    position_black,
                    position_white,
                    state,
                },
            )
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(games)
    }
}

impl MySqlGameRepository {
    pub fn new(pool: Pool) -> Self {
        MySqlGameRepository { pool }
    }
}

pub struct MockGameRepository {
    games: RwLock<HashMap<String, Game>>,
    moves: RwLock<HashMap<String, Vec<u64>>>,
}

impl MockGameRepository {
    pub fn new() -> Self {
        Self {
            games: RwLock::new(HashMap::new()),
            moves: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert_game(&self, game_uuid: &str, game: Game) {
        self.games
            .write()
            .unwrap()
            .insert(game_uuid.to_string(), game);
    }

    pub fn insert_move(&self, game_uuid: &str, move_number: u64) {
        self.moves
            .write()
            .unwrap()
            .entry(game_uuid.to_string())
            .or_default()
            .push(move_number);
    }
}

#[async_trait]
impl GameRepository for MockGameRepository {
    async fn get_game(&self, game_uuid: &str) -> Result<Option<Game>, RepositoryError> {
        let guard = self.games.read().unwrap();
        Ok(guard.get(game_uuid).cloned())
    }

    async fn get_max_move_no(&self, game_uuid: &str) -> Result<u64, RepositoryError> {
        let guard = self.moves.read().unwrap();
        let max_move = guard
            .get(game_uuid)
            .and_then(|m| m.iter().max())
            .cloned()
            .unwrap_or(0);
        Ok(max_move)
    }

    async fn get_last_move(&self, game_uuid: &str) -> Result<u64, RepositoryError> {
        let guard = self.moves.read().unwrap();
        match guard.get(game_uuid) {
            Some(m) => {
                let l = m.len();
                if l == 0 {
                    return Err(RepositoryError::Other(
                        "Failed to get last move".to_string(),
                    ));
                }
                return Ok(m[l - 1]);
            }
            None => {
                return Err(RepositoryError::Other(
                    "Failed to get last move".to_string(),
                ));
            }
        }
    }

    async fn create_game(&self, _game: &Game) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn update_game(&self, _game: &Game) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn update_game_with_move(
        &self,
        _game: &Game,
        _move_bit: u64,
        _move_no: u64,
    ) -> Result<(), RepositoryError> {
        Ok(())
    }

    async fn pending_games(&self, _player_uuid: String) -> Result<Vec<Game>, RepositoryError> {
        let games: Vec<Game> = Vec::new();
        Ok(games)
    }
}
