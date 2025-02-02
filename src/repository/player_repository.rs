use crate::model::User;
use async_trait::async_trait;
use mysql_async::{prelude::*, Pool};

use crate::repository::db_errors::*;

#[async_trait]
pub trait PlayerRepository {
    async fn player_list(&self) -> Result<Vec<User>, RepositoryError>;
}

pub struct MySqlPlayerRepository {
    pool: Pool,
}

#[async_trait]
impl PlayerRepository for MySqlPlayerRepository {
    async fn player_list(&self) -> Result<Vec<User>, RepositoryError> {
        let mut conn = self
            .pool
            .get_conn()
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;
        let users: Vec<User> = conn
            .query_map(
                "SELECT bin_to_uuid(player_uuid) as player_uuid, comment FROM players",
                |(player_uuid, comment)| User {
                    player_uuid,
                    comment,
                },
            )
            .await
            .map_err(|e| RepositoryError::DatabaseError(e.to_string()))?;

        Ok(users)
    }
}

impl MySqlPlayerRepository {
    pub fn new(pool: Pool) -> Self {
        MySqlPlayerRepository { pool }
    }
}
