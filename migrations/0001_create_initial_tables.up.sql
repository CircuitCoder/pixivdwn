CREATE TABLE illusts (
    id INTEGER PRIMARY KEY,
    title TEXT,
    author_id INTEGER,

    illust_state INTEGER NOT NULL,

    -- YYYY-MM-DDTHH:MM:SS.SSS+TZ
    create_date TEXT,
    update_date TEXT,

    x_restrict INTEGER CHECK (x_restrict in (0, 1, 2)),
    ai_type INTEGER CHECK (ai_type in (0, 1, 2)),

    bookmark_id INTEGER, -- Maybe null for unbookmarked
    bookmark_private BOOLEAN,

    last_fetch TEXT NOT NULL,
    last_successful_fetch TEXT,
    last_full_fetch TEXT,

    FOREIGN KEY (author_id) REFERENCES authors(id)
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

CREATE TABLE authors (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL
)
