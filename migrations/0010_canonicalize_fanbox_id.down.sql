-- Down migration
-- Revert the canonicalization by doing the exactly reverse.

-- We also need to delete all posts that has missing body, because in the old schema
-- body is NOT NULL.

DELETE FROM fanbox_posts WHERE body IS NULL;

ALTER TABLE fanbox_posts RENAME TO fanbox_posts_new;
ALTER TABLE fanbox_images RENAME TO fanbox_images_new;
ALTER TABLE fanbox_files RENAME TO fanbox_files_new;

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

-- Use false for is_adult

INSERT INTO fanbox_posts (id, creator_id, title, body, is_body_rich, fee, published_datetime, updated_datetime, fetched_at, is_adult)
    SELECT CAST(id AS TEXT), creator_id, title, body, is_body_rich, fee, published_datetime, updated_datetime, fetched_at, FALSE
    FROM fanbox_posts_new;
INSERT INTO fanbox_images (id, post_id, url, width, height, ext, idx, path, downloaded_at)
    SELECT id, CAST(post_id AS TEXT), url, width, height, ext, idx, path, downloaded_at
    FROM fanbox_images_new;
INSERT INTO fanbox_files (id, post_id, name, url, size, ext, idx, path, downloaded_at)
    SELECT id, CAST(post_id AS TEXT), name, url, size, ext, idx, path, downloaded_at
    FROM fanbox_files_new;

DROP TABLE fanbox_posts_new;
DROP TABLE fanbox_images_new;
DROP TABLE fanbox_files_new;
