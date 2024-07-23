CREATE TABLE IF NOT EXISTS users (
    id       INTEGER PRIMARY KEY,
    username TEXT    UNIQUE      NOT NULL,
    password TEXT                NOT NULL
);

CREATE TABLE IF NOT EXISTS banned_words (
    id       INTEGER PRIMARY KEY,
    word     TEXT    UNIQUE       NOT NULL,
    full_ban BOOLEAN
);

CREATE TABLE IF NOT EXISTS banned_ips (
    id INTEGER PRIMARY KEY,
    ip TEXT    UNIQUE      NOT NULL
);

CREATE TABLE IF NOT EXISTS drawings (
    id         INTEGER   PRIMARY KEY,
    data       BLOB      NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    author     TEXT      NOT NULL,
    approved   BOOLEAN   DEFAULT FALSE
);
