# Query

This guide shows some ways to work with the `pixivdwn query` subcommand.

`pixivdwn query` is used to query the local database about illustrations. You can filter by:
- `-t`: Tags (actual tags added to the illustration)
- `-b`: Bookmark tags (the tags you added during bookmarking)
- `-s`: State (is this illustration successfully synced?)
- `-d`: Image state (what's the download status of the images?)
- `-a`: Author ID (Numerical ID of the author. We plan to add filtering by author name in the future)
- `-i`: Illust ID (If you want to query a specific illustration)

You can also tweaks the output:

- Use `-o` to specify the ordering of the output.
- Use `-f` to specify the output format.

Check `pixivdwn query -h` for more details.