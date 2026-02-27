# Sync

This guide details the primary ways to sync the metadata of illustrations into the local database, and some caveats.

There is currently two ways to sync illustrations:
- Sync from your bookmarks
- Sync individual illustrations by their IDs

We plan to add support to sync an illustrator's works, see [#20](https://github.com/CircuitCoder/pixivdwn/issues/20).

## Sync from bookmarks

```bash
pixivdwn bookmarks [-t BOOKMARK TAG] [-p] [--term on-hit]
```

Pixiv have two different API endpoint for public and private bookmarks. Use `-p` to sync private bookmarks, and no `-p` to sync public bookmarks.

One perculiarity of Pixiv's bookmark system is that it always lists the bookmarks in reverse of the order you bookmarked them. This means that most of the time, if we only want to sync the **new** bookmarks, we can stop once we encounter an illustration that's already in the database. This can be done by adding the `--term on-hit` option. There is two caveats to this approach:

1. If you synced an illustration in a separate say, say, by syncing by ID, or removed an old bookmark and re-added one, then the syncing will stop prematurely. We plan to address these problem in the future by checking if the illustration metadata and bookmark metadata is completely unchanged ([#21](https://github.com/CircuitCoder/pixivdwn/issues/21)).
2. If you **edited** an old bookmark (not removing and re-adding it), then `--term on-hit` will not pick up the changes in your bookmark tags. This is especially problematic because syncing by ID also does not update the bookmark tags, see the section below about syncing by ID.

Therefore, you may want to periodically run a full sync without `--term on-hit`.

The `-t` option accepts an **bookmark tag**, which is the tags you set when you bookmark an illustration, and correspond to the `-b` option in `pixivdwn query`. If the tag you want to filter is not set during the bookmarking, you can always just sync everything, and then filter them during query.

## Sync by ID

```bash
pixivdwn illust [ILLUST ID]...
```

This command is useful when you want to specifically update some illustrations. The list of IDs may come from a query. An example for this is that you may want to re-sync all the previously failed illustrations to check if the illustrator has republished them.

To make this process easier, `pixivdwn illust` can accept a list of IDs from a file or stdin with the `-l` option. For the aforementioned example, you can:

```bash
pixivdwn query -s masked | pixivdwn illust -l -
```

Because the limition of Pixiv's API, syncing by ID does not update the bookmark information. If you added/removed/edited the bookmark on this illustration, you need to sync it through the bookmark syncing procedure. We plan to add a option to make another call to Pixiv's bookmark API and get the updated bookmark information, see [#22](https://github.com/CircuitCoder/pixivdwn/issues/22).