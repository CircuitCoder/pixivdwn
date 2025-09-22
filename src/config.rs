pub struct PixivSession {
    pub uid: u64,
    pub cookie: String,
}

impl TryFrom<&str> for PixivSession {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let uid_seg = value
            .split("_")
            .next()
            .ok_or_else(|| anyhow::anyhow!("Invalid pixiv cookie"))?;
        let uid = uid_seg
            .parse::<u64>()
            .map_err(|_| anyhow::anyhow!("Invalid uid in pixiv cookie"))?;
        Ok(Self {
            uid,
            cookie: value.to_string(),
        })
    }
}

pub struct Session {
    pub pixiv: Option<PixivSession>,
    #[allow(unused)]
    pub fanbox: Option<String>,
}

impl Session {
    pub fn new(
        pixiv_cookie: Option<String>,
        fanbox_cookie: Option<String>,
    ) -> anyhow::Result<Self> {
        let pixiv = if let Some(cookie) = pixiv_cookie {
            Some(cookie.as_str().try_into()?)
        } else {
            None
        };

        Ok(Self {
            pixiv,
            fanbox: fanbox_cookie,
        })
    }
}
