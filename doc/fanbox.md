# Fanbox

`pixivdwn` supports downloading from [Fanbox](https://fanbox.cc/). The workflow is mostly the same as Pixiv, you can first sync posts and then query + download files and images. It also takes extra care to make sure that after you cancel the subscription to a creator, syncing doesn't overwrite existing posts with the updated but inaccessible ones.

Currently you can sync posts by:
- A given post ID
- A given creator ID (sync all posts)
- All posts from supported creators

We plan to add another option to sync from all _followed_ creators. See `pixivdwn fanbox sync -h` for more details.

Fanbox's post body is in a rich WYSIWYG format. We tries to extract all images and files within the post body. The two types of downloadable attachments are tracked separately, so to download all images and files, use:

```bash
pixivdwn fanbox attachment image --downloaded false | pixivdwn fanbox download image -p -l -
pixivdwn fanbox attachment file --downloaded false | pixivdwn fanbox download file -p -l -
```

Run `pixivdwn fanbox attachment -h` for more options about attachment queries, and `pixivdwn fanbox download -h` for more options about downloading.