# Database plumbing

Sometime you might want to directly modify the database. The most frequent case is to move the physical file around and update the path in the database. If you find yourself in this scenario, you have two choices:
- `pixivdwn database` provides some low-level plumbing commands.
- You can also directly use `sqlite3` to inspect and modify the database. We try our best to keep the database schema simple and intuitive.

## Moving files

`pixivdwn database file canonicalize` moves the file from the path that's specified in the database, into a new path calculated based on the currently given base dir.

However, use the command to move the file in fs has some drawback. If you intend to move the entire base dir (which is the most common case), the best way is to do a single RENAME on the entire directory. `pixivdwn database file canonicalize` doesn't do this. Also, sometime even cross-device moves can benifits from offloading (e.g. ZFS).

In these cases, you can first move the files by yourself, then issue an `pixivdwn database file canonicalize --skip-file`, which checks for the existence of the file, but doesn't acutally do any filesystem operations. This does introduce a temporary inconsistency between the database and the filesystem, so make sure you stop any scheduled background tasks.

## Checking filesystem consistency

`pixivdwn database file fsck` checks for the existence of pointed files on disks. Right now there is no checksums. In the future we might add checksums into the database.

## Path format

We strongly recommend using the absolute path format, because that's much easier to work with. Since we can canonicalize the path in the database after a base dir move, the benifits of using relative path deminishes.