use std::path::Path;

use reqwest::header;
use tracing::{debug, info};

use super::dev::*;

pub async fn dl(ctx: &Context, url: impl AsRef<str>, expire: Option<u64>) -> Result<String> {
    let Some(cdir) = &ctx.cache_dir else {
        whatever!("cache dir not set, cannot download {}", url.as_ref())
    };
    let url = url.as_ref();
    let Ok(url2) = url.parse::<reqwest::Url>() else {
        whatever!("invalid url: {}", url)
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    use base64::Engine;
    let name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(url);
    let path = cdir.join(&name).to_string_lossy().to_string();
    let vl = ctx.cache.get(&name, "").await?;
    if let Some((version, _)) = &vl
        && let (Ok(t), Some(expire)) = (version.parse::<u64>(), expire)
        && now - t < expire
    {
        info!("cache hit for url: {}", url);
        return Ok(path);
    }

    let mut req = reqwest::Request::new(reqwest::Method::GET, url2);
    req.headers_mut().insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("dv-wrap/0.1"),
    );

    if let Some((_, e)) = &vl {
        req.headers_mut().insert(
            header::IF_NONE_MATCH,
            header::HeaderValue::from_str(e).unwrap(),
        );
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;
    if ctx.dry_run {
        return Ok(path);
    }
    let Ok(mut resp) = client.execute(req).await else {
        whatever!("failed to fetch url: {}", url)
    };
    if resp.status().is_success() {
        let etag = resp
            .headers()
            .get(header::ETAG)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string();
        let mut opt = tokio::fs::OpenOptions::new();
        opt.create(true).write(true).truncate(true);
        let mut file = loop {
            match opt.open(&path).await {
                Ok(file) => break Ok(file),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    if let Some(parent) = Path::new(&path).parent() {
                        debug!("try to create dir {}", parent.display());
                        tokio::fs::create_dir_all(parent).await?;
                    }
                }
                Err(e) => break Err(e),
            }
        }?;
        while let Some(chunk) = resp.chunk().await? {
            tokio::io::copy(&mut chunk.as_ref(), &mut file).await?;
        }
        ctx.cache.set(&name, "", &now.to_string(), &etag).await?;
    } else if resp.status().as_u16() == 304 {
        ctx.cache
            .set(&name, "", &now.to_string(), &vl.unwrap().1)
            .await?;
    } else {
        whatever!("failed to fetch url: {}, status: {}", url, resp.status())
    }
    Ok(path)
}
