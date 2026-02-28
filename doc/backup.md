# Database & filesystem backup

Since this is a alpha release, the behavior of this tool might change, and expect some bugs. Therefore, it's recommended to regularly backup your database and downloaded files.

The downloader works mostly in an incremental way. Existing files normally won't be deleted / overwritten unless you explicitly ask so. So the recommended way to backup the file and database is to place them onto a filesystem that supports snapshots (e.g. ZFS, Btrfs), and then take snapshots regularly. The overhead is minimal, basically the only data that's stored redundantly is the database.

Alternatively, `pixivdwn` tries to be careful with mtime and ctime (overwritten files are first deleted, then recreated, so ctime is correct). So you can also use `rsync` to backup the files. For the database, you can try `sqlitediff`, but backing up the entire database file should also be fine.