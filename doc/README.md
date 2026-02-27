# pixivdwn

![Crates.io Version](https://img.shields.io/crates/v/pixivdwn)

`pixivdwn` is an "incremental" Pixiv downloader, in the sense that it tries to cache fetched information, patch them during update, and allowing powerful querying. It is optimized to run as an unattended periodic background job. It keeps track of illustrations in a local database, and enables the user to selectively download some of the illustrations.

The two basic tracked entities are "illustrations" and "images", which roughly correspond to galleries and individual image files. The primary way to use `pixivdwn` is to first sync illustration metadata into the local database through various means (e.g. an illustrator's works, a user's bookmarks, an individual illustration, etc.). Then, we can query about the status of the illustrations' images, and download them if they are currently outdated or missing. Fanbox is analogous.

Run `pixivdwn -h` for more details.

## Quickstart

To download all bookmarked illustrations with bookmark tag "meow", run the following script periodically:

```bash
# pixivdwn supports .env file
cat >> .env << EOF
DATABASE_URL=sqlite://./db.sqlite
PIXIV_COOKIE=<your cookie>
export PIXIV_BASE_DIR=./illusts
EOF

# Setup the database and/or run database upgrade
# This will create a db.sqlite file at your PWD
pixivdwn setup

# Sync all bookmarks with bookmark tag "meow" into local database
# This process is terminated when any existing illustration is found (useful for periodic runs)
pixivdwn bookmarks -t meow --term on-hit

# Query for all illustrations with bookmark tag "meow" that has updated / unfetched images
# and download them into ./illusts directory
pixivdwn query -b meow -d outdated-downloaded -o id-desc | pixivdwn download -l - -p
```