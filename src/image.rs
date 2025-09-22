use crate::config::Session;
use futures::StreamExt;

pub enum DownloadSource {
    Pixiv,
    #[allow(unused)]
    Fanbox,
}

pub async fn download<W: std::io::Write>(
    session: &Session,
    src: DownloadSource,
    url: &str,
    mut dst: W,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();

    let mut req = client.get(url)
        .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/58.0.3029.110 Safari/537.36");
    match src {
        DownloadSource::Pixiv => {
            req = req.header("Referer", "https://www.pixiv.net/");
            if let Some(pixiv) = &session.pixiv {
                req = req.header("Cookie", format!("PHPSESSID={};", pixiv.cookie));
            }
        }
        DownloadSource::Fanbox => unimplemented!(),
    }

    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!("Failed to download: HTTP {}", status);
    }

    // FIXME: check MIME

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        dst.write_all(&chunk)?;
    }

    Ok(())
}
