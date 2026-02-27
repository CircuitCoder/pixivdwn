# Caveats

This file records some caveats of `pixivdwn`. Maybe if you encounter some weird behaviors, you can check this file to see if it's a known issue. This can also be a list of potential improvements!

- Work detail seems to trim `create_date` and `update_date` timezones. But all the timezones I got from bookmark lists are always JST. So not sure if the information is really useful in anyway. For consistent comparison, we may need to convert everything into UTC instead.