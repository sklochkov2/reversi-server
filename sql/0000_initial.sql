CREATE TABLE players (
    player_uuid BINARY(16) NOT NULL PRIMARY KEY,
    comment VARCHAR(1024) CHARACTER SET utf8mb4 COLLATE utf8mb4_general_ci
) ENGINE=InnoDB;

CREATE TABLE games (
    game_uuid BINARY(16) NOT NULL PRIMARY KEY,
    black_uuid BINARY(16) DEFAULT NULL,
    white_uuid BINARY(16) DEFAULT NULL,
    state BIGINT UNSIGNED,
    position_black BIGINT UNSIGNED,
    position_white BIGINT UNSIGNED,
    FOREIGN KEY (black_uuid) REFERENCES players(player_uuid) ON DELETE CASCADE ON UPDATE CASCADE,
    FOREIGN KEY (white_uuid) REFERENCES players(player_uuid) ON DELETE CASCADE ON UPDATE CASCADE
) ENGINE=InnoDB;

CREATE TABLE moves (
    move_id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    game_uuid BINARY(16) NOT NULL,
    move_number BIGINT UNSIGNED NOT NULL,
    move_position BIGINT UNSIGNED NOT NULL,
    position_black BIGINT UNSIGNED NOT NULL,
    position_white BIGINT UNSIGNED NOT NULL,
    FOREIGN KEY (game_uuid) REFERENCES games(game_uuid) ON DELETE CASCADE ON UPDATE CASCADE,
    INDEX (game_uuid)
) ENGINE=InnoDB;
