CREATE TABLE IF NOT EXISTS hiscores_snapshots (
    id BIGINT AUTO_INCREMENT PRIMARY KEY,
    game VARCHAR(20) NOT NULL,
    mode VARCHAR(30) NOT NULL,
    rsn VARCHAR(12) NOT NULL,
    snapshot_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    data TEXT NOT NULL,
    INDEX idx_game_mode_rsn_time (game, mode, rsn, snapshot_at)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
