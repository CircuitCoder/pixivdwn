-- Down migration
-- Revert images schema to the previous form:
-- - Remove verified_date and (illust_id, verified_date) index
-- - Restore nullable path/download_date
-- - Restore PRIMARY KEY (illust_id, page)
--
-- This may collapse multiple image versions for the same (illust_id, page).
-- We keep the latest version by verified_date; ties are resolved by rowid.

ALTER TABLE images RENAME TO images_new;

CREATE TABLE images (
	illust_id INTEGER NOT NULL,
	page INTEGER NOT NULL,
	url TEXT NOT NULL,

	download_date TEXT,
	path TEXT,

	width INTEGER,
	height INTEGER,
	ugoira_frames TEXT,

	PRIMARY KEY (illust_id, page),
	FOREIGN KEY (illust_id) REFERENCES illusts(id), -- Do NOT cascade delete

	UNIQUE (path)
);

INSERT OR REPLACE INTO images (illust_id, page, url, download_date, path, width, height, ugoira_frames)
	SELECT illust_id, page, url, download_date, path, width, height, ugoira_frames
	FROM images_new
	ORDER BY verified_date ASC, rowid ASC;

DROP TABLE images_new;
