pub struct UIDSession {
    pub uid: u64,
    pub cookie: String,
}

impl TryFrom<&str> for UIDSession {
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
    pub pixiv: Option<UIDSession>,
    pub fanbox: Option<UIDSession>,
    pub fanbox_full: Option<String>,
}

impl Session {
    pub fn new(
        pixiv_cookie: Option<String>,
        fanbox_cookie: Option<String>,
        fanbox_full: Option<String>,
    ) -> anyhow::Result<Self> {
        let pixiv = pixiv_cookie.map(|e| UIDSession::try_from(e.as_str())).transpose()?;
        let fanbox = fanbox_cookie.map(|e| UIDSession::try_from(e.as_str())).transpose()?;

        Ok(Self {
            pixiv,
            fanbox,
            fanbox_full,
        })
    }
}
