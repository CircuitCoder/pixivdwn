-- Change fanbox_posts.id to INTEGER. To do this, move the original fanbox tables
-- to temporary tables, create new tables with the correct schema, copy data over,
-- and drop the temporary tables.

-- Also, drop the NOT NULL constraint on fanbox_posts.body and fanbox_posts.is_body_rich,
-- for restricted posts.

-- Also also, drop the is_adult field because it's actually not related to the post.

ALTER TABLE fanbox_posts RENAME TO fanbox_posts_old;
ALTER TABLE fanbox_images RENAME TO fanbox_images_old;
ALTER TABLE fanbox_files RENAME TO fanbox_files_old;

CREATE TABLE fanbox_posts (
    id INTEGER PRIMARY KEY,
    creator_id TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT,
    is_body_rich BOOLEAN,

    fee INTEGER NOT NULL,
    published_datetime TEXT NOT NULL,
    updated_datetime TEXT NOT NULL,

    fetched_at TEXT NOT NULL,

    -- body and is_body_rich should be both NULL or NOT NULL
    CHECK ((body IS NULL AND is_body_rich IS NULL) OR (body IS NOT NULL AND is_body_rich IS NOT NULL))
);

CREATE TABLE fanbox_images (
    id TEXT PRIMARY KEY,
    post_id INTEGER NOT NULL,

    url TEXT NOT NULL,
    width INTEGER NOT NULL,
    height INTEGER NOT NULL,
    ext TEXT NOT NULL,
    idx INTEGER NOT NULL,

    path TEXT,
    downloaded_at TEXT,

    FOREIGN KEY (post_id) REFERENCES fanbox_posts(id) ON DELETE CASCADE,

    UNIQUE(path)
);

CREATE TABLE fanbox_files (
    id TEXT PRIMARY KEY,
    post_id INTEGER NOT NULL,

    name TEXT NOT NULL,
    url TEXT NOT NULL,
    size INTEGER NOT NULL,
    ext TEXT NOT NULL,
    idx INTEGER NOT NULL,

    path TEXT,
    downloaded_at TEXT,

    FOREIGN KEY (post_id) REFERENCES fanbox_posts(id) ON DELETE CASCADE,

    UNIQUE(path)
);

INSERT INTO fanbox_posts (id, creator_id, title, body, is_body_rich, fee, published_datetime, updated_datetime, fetched_at)
    SELECT CAST(id AS INTEGER), creator_id, title, body, is_body_rich, fee, published_datetime, updated_datetime, fetched_at
    FROM fanbox_posts_old;
INSERT INTO fanbox_images (id, post_id, url, width, height, ext, idx, path, downloaded_at)
    SELECT id, CAST(post_id AS INTEGER), url, width, height, ext, idx, path, downloaded_at
    FROM fanbox_images_old;
INSERT INTO fanbox_files (id, post_id, name, url, size, ext, idx, path, downloaded_at)
    SELECT id, CAST(post_id AS INTEGER), name, url, size, ext, idx, path, downloaded_at
    FROM fanbox_files_old;

DROP TABLE fanbox_posts_old;
DROP TABLE fanbox_images_old;
DROP TABLE fanbox_files_old;
