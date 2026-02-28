# Download

This guide talks about the downloading process. Assume you've already [synced](sync.md) some illustrations into the local database. Now you can download the images of these illustrations.

The `pixivdwn download` subcommand is used to download images. You can specify a list of IDs into the command line, or just like `pixivdwn illust`, you can pass in a file (or stdin) with `-l` for `pixivdwn` to read the list of IDs from.

```bash
pixivdwn download 114 514
echo -e "114\n514" | pixivdwn download -l -
```

A frequently used pattern is downloading the images of a illustration that either has not finished downloading, or was updated since some of the downloaded files. This pattern can be achieved by:

```bash
pixivdwn query -d outdated | pixivdwn download -l -
```

Updated illustrations can have downloaded but outdated images. The default behavior of `pixivdwn download` for these images is to re-download and verify if the file have changed. If not, the timestamp on the image is bumped to the current time. If changed, the old file will be preserved with a suffix containing it's hash. You can use `--on-existing` option to change the behavior. Check `pixivdwn download help`

You can use `-p` to show a progress bar.