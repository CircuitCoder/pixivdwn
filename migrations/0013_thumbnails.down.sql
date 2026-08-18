DROP TABLE fanbox_file_thumbnails;

ALTER TABLE fanbox_images DROP COLUMN thumbnail_date;
ALTER TABLE fanbox_images DROP COLUMN thumbnail;

ALTER TABLE images DROP COLUMN thumbnail_date;
ALTER TABLE images DROP COLUMN thumbnail;

ALTER TABLE fanbox_posts DROP COLUMN cover_id;
