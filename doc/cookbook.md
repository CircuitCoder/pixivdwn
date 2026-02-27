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