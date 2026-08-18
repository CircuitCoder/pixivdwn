-- Add new cover field in the fanbox_posts field so we can refer to arbitrary cover images
-- We don't use a foreign key here because sometime the post is synced before the cover
ALTER TABLE fanbox_posts ADD COLUMN cover_id TEXT DEFAULT NULL;

-- Backfill standalone covers stored by the previous idx IS NULL convention.
-- If a post accumulated multiple covers, the latest inserted image is current.
UPDATE fanbox_posts
SET cover_id = (
    SELECT id
    FROM fanbox_images
    WHERE fanbox_images.post_id = fanbox_posts.id
      AND fanbox_images.idx IS NULL
    ORDER BY fanbox_images.rowid DESC
    LIMIT 1
)
WHERE EXISTS (
    SELECT 1
    FROM fanbox_images
    WHERE fanbox_images.post_id = fanbox_posts.id
      AND fanbox_images.idx IS NULL
);

-- Next, create a columns to store thumbnails
ALTER TABLE images ADD COLUMN thumbnail BLOB DEFAULT NULL;
ALTER TABLE images ADD COLUMN thumbnail_date DATETIME DEFAULT NULL
    CHECK ((thumbnail IS NULL) = (thumbnail_date IS NULL));
ALTER TABLE fanbox_images ADD COLUMN thumbnail BLOB DEFAULT NULL;
ALTER TABLE fanbox_images ADD COLUMN thumbnail_date DATETIME DEFAULT NULL
    CHECK ((thumbnail IS NULL) = (thumbnail_date IS NULL));

-- Create a one-to-many table for files
CREATE TABLE fanbox_file_thumbnails (
    id TEXT PRIMARY KEY,
    file_id TEXT NOT NULL,
    subpath TEXT, -- Null stands for the whole file, not sure why a file can have a thumbnail
    thumbnail BLOB NOT NULL,
    thumbnail_date DATETIME NOT NULL,
    FOREIGN KEY (file_id) REFERENCES fanbox_files(id) ON DELETE CASCADE
);
