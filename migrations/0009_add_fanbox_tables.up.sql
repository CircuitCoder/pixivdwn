CREATE TABLE fanbox_posts (
    id TEXT PRIMARY KEY,
    creator_id TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    is_body_rich BOOLEAN NOT NULL,

    fee INTEGER NOT NULL,
    published_datetime TEXT NOT NULL,
    updated_datetime TEXT NOT NULL,

    fetched_at TEXT NOT NULL,

    is_adult BOOLEAN NOT NULL
);

CREATE TABLE fanbox_images (
    id TEXT PRIMARY KEY,
    post_id TEXT NOT NULL,

    url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    ext TEXT NOT NULL,
    idx INTEGER NOT NULL,

    path TEXT,
    downloaded_at TEXT,
    
    FOREIGN KEY (post_id) REFERENCES fanbox_posts(id) ON DELETE CASCADE

    UNIQUE(path)
);

CREATE TABLE fanbox_files (
    id TEXT PRIMARY KEY,
    post_id TEXT NOT NULL,

    name TEXT NOT NULL,
    url TEXT NOT NULL,
    size INTEGER NOT NULL,
    ext TEXT NOT NULL,
    idx INTEGER NOT NULL,

    path TEXT,
    downloaded_at TEXT,

    FOREIGN KEY (post_id) REFERENCES fanbox_posts(id) ON DELETE CASCADE

    UNIQUE(path)
);
