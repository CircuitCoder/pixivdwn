ALTER TABLE illusts ADD COLUMN content_desc TEXT;
ALTER TABLE illusts ADD COLUMN content_is_howto BOOLEAN;
ALTER TABLE illusts ADD COLUMN content_is_original BOOLEAN;

ALTER TABLE illusts ADD COLUMN last_successful_content_fetch TEXT;
