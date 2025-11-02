# pixivdwn

`pixivdwn` is an "incremental" Pixiv downloader, which is optimized to run as an unattended periodic background job. It keeps track of illustrations in a local database, and enables the user to selectively download some of the illustrations.

## Quickstart

To download all bookmarked illustrations with bookmark tag "meow", run the following script periodically:

```bash
# Sync all bookmarks with bookmark tag "meow" into local database
# This process is terminated when any existing illustration is found
pixivdwn bookmarks -t meow --term on-hit

# Query for all illustrations with bookmark tag "meow" that has not been downloaded yet,
# and pipe their ids into the download command
pixivdwn query -b meow -d not-fully-downloaded -o id-desc | pixivdwn download -b ./illusts -l - -p
```
