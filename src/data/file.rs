use std::path::Path;

use crate::data::{RequestArgumenter, RequestExt};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

pub async fn download<W: std::io::Write, R: RequestArgumenter>(
    req_arg: R,
    url: &str,
    mut dst: W,
    show_progress: bool,
) -> anyhow::Result<(usize, [u8; 32])> {
    let fetch_ctx = crate::fetch::FetchCtxGuard::begin().await;
    let client = fetch_ctx.client();

    let req = client.get(url).prepare_with(req_arg)?.build()?;

    let resp = client.execute(req).await?;
    let status = resp.status();

    if !status.is_success() {
        anyhow::bail!("Failed to download: HTTP {}", status);
    }

    let size = resp.headers().get("Content-Length").and_then(|e| {
        let s = e.to_str().ok()?;
        s.parse().ok()
    });
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
    let mut total_length = 0;
    let mut digest = Sha256::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        digest.update(&chunk);
        dst.write_all(&chunk)?;
        if let Some(ref mut bar) = bar {
            bar.inc(chunk.len() as u64);
            total_length += chunk.len();
        }
    }

    if let Some(bar) = bar {
        bar.finish();
    }

    Ok((total_length, digest.finalize().into()))
}

pub async fn download_to_tmp<R: RequestArgumenter>(
    req_arg: R,
    base_dir: &Path,
    url: &str,
    show_progress: bool,
) -> anyhow::Result<(NamedTempFile, usize, [u8; 32])> {
    let mut tmp_file = NamedTempFile::with_prefix_in("pixivdwn_", base_dir)?;
    let mut buffered_file = std::io::BufWriter::new(tmp_file.as_file_mut());
    let (file_len, digest) = download(req_arg, url, &mut buffered_file, show_progress).await?;
    drop(buffered_file);
    Ok((tmp_file, file_len, digest))
}
