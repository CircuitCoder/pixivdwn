pub struct Session {
  pub uid: u64,
  pub pixiv_cookie: String,
  pub fanbox_cookie: Option<String>,
}

impl Session {
  pub fn new(pixiv_cookie: String, fanbox_cookie: Option<String>) -> anyhow::Result<Self> {
    let uid_seg = pixiv_cookie.split("_").next()
      .ok_or_else(|| anyhow::anyhow!("Invalid pixiv cookie"))?;
    let uid = uid_seg.parse::<u64>()
      .map_err(|_| anyhow::anyhow!("Invalid uid in pixiv cookie"))?;
    Ok(Self { uid, pixiv_cookie, fanbox_cookie })
  }
}
