CREATE TABLE images (
    illust_id INTEGER NOT NULL,
    page INTEGER NOT NULL,
    url TEXT NOT NULL,

    download_date TEXT,
    path TEXT,

    PRIMARY KEY (illust_id, page),
    FOREIGN KEY (illust_id) REFERENCES illusts(id), -- Do NOT cascade delete

    UNIQUE (path)
)
