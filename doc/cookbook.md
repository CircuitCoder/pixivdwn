# Cookbook

This guide contains some frequently-used patterns and recipes for scripting with `pixivdwn`.

More example will be added as time goes on. If you have any suggestions for more recipes, please open an issue or a pull request at [our GitHub repository](https://github.com/CircuitCoder/pixivdwn).

## Counting the number of all successfully synced illustrations

```bash
pixivdwn query -s normal -f count
```

## Re-syncing all failed illustrations

```bash
pixivdwn query -s masked | pixivdwn illust -l -
pixivdwn query -s unlisted | pixivdwn illust -l -
```

## Query for multiple tags (AND)

```bash
pixivdwn query -t tag1 -b bookmark-tag2
```

## Sync all supported fanbox users, then download all missing attachments

```bash
pixivdwn fanbox sync --term on-hit
pixivdwn fanbox attachment image --downloaded false | pixivdwn fanbox download image -p -l -
pixivdwn fanbox attachment file --downloaded false | pixivdwn fanbox download file -p -l -
```