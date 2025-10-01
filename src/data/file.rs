use crate::data::{RequestArgumenter, RequestExt};
use futures::StreamExt;
use tempfile::NamedTempFile;

pub async fn download<W: std::io::Write, R: RequestArgumenter>(
    req_arg: R,
    url: &str,
    mut dst: W,
    show_progress: bool,
) -> anyhow::Result<()> {
    let fetch_ctx = crate::fetch::FetchCtxGuard::begin().await;
    let client = fetch_ctx.client();

    let req = client.get(url).prepare_with(req_arg)?.build()?;

    let resp = client.execute(req).await?;
    let status = resp.status();

    if !status.is_success() {
        anyhow::bail!("Failed to download: HTTP {}", status);
    }

    let size = resp.content_length();
    let mut bar = if !show_progress {
        None
    } else {
        let bar = if let Some(size) = size {
            indicatif::ProgressBar::new(size)
        } else {
            indicatif::ProgressBar::new_spinner()
        };
        bar.set_style(indicatif::ProgressStyle::with_template(
            "ETA {eta_precise} {elapsed_precise} | {wide_bar} {percent}% | {binary_bytes}/{binary_total_bytes} [{binary_bytes_per_sec}]"
        ).unwrap().progress_chars("##-"));
        Some(bar)
    };

    // FIXME: check MIME

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        dst.write_all(&chunk)?;
        if let Some(ref mut bar) = bar {
            bar.inc(chunk.len() as u64);
        }
    }

    if let Some(bar) = bar {
        bar.finish();
    }

    Ok(())
}

pub async fn download_to_tmp<R: RequestArgumenter>(
    req_arg: R,
    base_dir: &str,
    url: &str,
    show_progress: bool,
) -> anyhow::Result<NamedTempFile> {
    let mut tmp_file = NamedTempFile::with_prefix_in("pixivdwn_", base_dir)?;
    let mut buffered_file = std::io::BufWriter::new(tmp_file.as_file_mut());
    download(req_arg, url, &mut buffered_file, show_progress).await?;
    drop(buffered_file);
    Ok(tmp_file)
}
