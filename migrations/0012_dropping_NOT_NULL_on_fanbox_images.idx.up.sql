-- Add up migration script here

ALTER TABLE fanbox_images ALTER COLUMN idx DROP NOT NULL;
