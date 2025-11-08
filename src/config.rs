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
    pub fanbox_header_full: Option<Vec<(String, String)>>,

    pub pixiv_base_dir: Option<std::path::PathBuf>,
    pub fanbox_base_dir: Option<std::path::PathBuf>,
}

impl Session {
    pub fn new(
        pixiv_cookie: Option<String>,
        fanbox_cookie: Option<String>,
        fanbox_header_full: Option<String>,
        pixiv_base_dir: Option<std::path::PathBuf>,
        fanbox_base_dir: Option<std::path::PathBuf>,
    ) -> anyhow::Result<Self> {
        let pixiv = pixiv_cookie
            .map(|e| UIDSession::try_from(e.as_str()))
            .transpose()?;
        let fanbox = fanbox_cookie
            .map(|e| UIDSession::try_from(e.as_str()))
            .transpose()?;

        let fanbox_header_full = fanbox_header_full
            .map(|e| -> anyhow::Result<Vec<_>> {
                let mut ret = Vec::new();
                for row in e
                    .trim()
                    .split("\n")
                    .filter(|e| {
                        !(e.starts_with("GET")
                            || e.starts_with("Host: ")
                            || e.starts_with("host: "))
                    })
                    .map(|e| -> anyhow::Result<_> {
                        let mut segs = e.splitn(2, ": ");
                        let name = segs.next().unwrap();
                        let value = segs
                            .next()
                            .ok_or_else(|| anyhow::anyhow!("Unable to parse header: {}", e))?;
                        Ok((name.to_owned(), value.to_owned()))
                    })
                {
                    ret.push(row?);
                }
                Ok(ret)
            })
            .transpose()?;

        tracing::debug!("Parsed full headers: {:#?}", fanbox_header_full);

        Ok(Self {
            pixiv,
            fanbox,
            fanbox_header_full,
            pixiv_base_dir,
            fanbox_base_dir,
        })
    }

    pub fn get_pixiv_base_dir(&self) -> anyhow::Result<&std::path::PathBuf> {
        self.pixiv_base_dir.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Pixiv base directory is not set.")
        })
    }

    pub fn get_fanbox_base_dir(&self) -> anyhow::Result<&std::path::PathBuf> {
        self.fanbox_base_dir.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Fanbox base directory is not set.")
        })
    }
}
