-- This migration does the following:
-- 1. Add a `verified_date` column to `images`, marking the last time that we verified the image was still the newest.
-- 2. Allow multiple versions of a same image. Remove the primary key (illust_id, page)
-- 3. Now forces `path`, `verified_date` and `downloaded_date` to be NOT NULL.
-- 4. Add a (illust_id, verified_date) index.
--    The migration fails if there exists rows that violates this constraint.
-- Because this involves primary key modification, we have to create a new table.

ALTER TABLE images RENAME TO images_old;

CREATE TABLE images (
	illust_id INTEGER NOT NULL,
	page INTEGER NOT NULL,
	url TEXT NOT NULL,

	download_date TEXT NOT NULL,
	verified_date TEXT NOT NULL,
	path TEXT NOT NULL,

	width INTEGER,
	height INTEGER,
	ugoira_frames TEXT,

	FOREIGN KEY (illust_id) REFERENCES illusts(id), -- Do NOT cascade delete

	UNIQUE (path)
);

INSERT INTO images (illust_id, page, url, download_date, verified_date, path, width, height, ugoira_frames)
	SELECT illust_id, page, url, download_date, download_date, path, width, height, ugoira_frames
	FROM images_old;

CREATE INDEX images_illust_id_verified_date_idx ON images (illust_id, verified_date);

DROP TABLE images_old;
