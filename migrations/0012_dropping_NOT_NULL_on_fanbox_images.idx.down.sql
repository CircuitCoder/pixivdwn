-- Add down migration script here

ALTER TABLE fanbox_images ALTER COLUMN idx SET NOT NULL;
