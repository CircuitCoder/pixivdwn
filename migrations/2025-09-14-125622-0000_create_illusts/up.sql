-- Your SQL goes here

CREATE TABLE illusts (
    id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    author_id INTEGER NOT NULL,

    -- YYYY-MM-DDTHH:MM:SS.SSS+TZ
    create_date TEXT NOT NULL,
    update_date TEXT NOT NULL,

    x_restrict INTEGER NOT NULL,
    ai_type INTEGER NOT NULL,

    illust_state INTEGER NOT NULL,
    -- TODO: illust data

    bookmark_id INTEGER NOT NULL,
    bookmark_private BOOLEAN NOT NULL,

    last_fetch TEXT,
    last_successful_fetch TEXT
);

CREATE TABLE tags (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    tag TEXT NOT NULL UNIQUE
);

CREATE TABLE illust_tags (
    illust_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (illust_id, tag_id),
    FOREIGN KEY (illust_id) REFERENCES illusts(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);

CREATE TABLE illust_bookmark_tags (
    illust_id INTEGER NOT NULL,
    tag_id INTEGER NOT NULL,
    PRIMARY KEY (illust_id, tag_id),
    FOREIGN KEY (illust_id) REFERENCES illusts(id) ON DELETE CASCADE,
    FOREIGN KEY (tag_id) REFERENCES tags(id) ON DELETE CASCADE
);
