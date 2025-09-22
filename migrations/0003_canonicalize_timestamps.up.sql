-- Update all fetched timestamps to UTC
UPDATE illusts SET create_date = datetime(create_date, 'utc') WHERE create_date IS NOT NULL;
UPDATE illusts SET update_date = datetime(update_date, 'utc') WHERE update_date IS NOT NULL;
