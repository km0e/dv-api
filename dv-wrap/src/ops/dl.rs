use std::path::Path;

use reqwest::header;
use tracing::{debug, info};

use super::dev::*;

pub struct Dl<C: AsRefContext> {
    ctx: C,
    req: reqwest::Request,
    now: u64,
    etag: Option<String>,
}

impl<C: AsRefContext> Dl<C> {
    pub async fn new<U: AsRef<str>>(
        ctx: C,
        url: U,
        expire: Option<u64>,
    ) -> Result<(String, Option<Self>)> {
        let c = ctx.as_ref();
        let Some(cdir) = &c.cache_dir else {
            bail!("cache dir not set, cannot download {}", url.as_ref())
        };
        let url = url.as_ref();
        let Ok(url2) = url.parse::<reqwest::Url>() else {
            bail!("invalid url: {}", url)
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs();

        use base64::Engine;
        let name = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(url);
        let path = cdir.join(&name).to_string_lossy().to_string();
        let vl = c.db.get(&name, "").await?;
        let exsists = match tokio::fs::metadata(&path).await {
            Ok(m) if m.is_file() => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            Err(e) => Err(e)?, // other error
            _ => {
                bail!("cache file {} exists but not a file", path)
            }
        };
        if exsists
            && let Some((version, _)) = &vl
            && let (Ok(t), Some(expire)) = (version.parse::<u64>(), expire)
            && now - t < expire
        {
            info!("cache hit {path} for url: {url}");
            return Ok((path, None));
        }

        let mut req = reqwest::Request::new(reqwest::Method::GET, url2);
        req.headers_mut().insert(
            header::USER_AGENT,
            header::HeaderValue::from_static("dv-wrap/0.1"),
        );

        if exsists && let Some((_, e)) = &vl {
            req.headers_mut().insert(
                header::IF_NONE_MATCH,
                header::HeaderValue::from_str(e).unwrap(),
            );
        }
        drop(c);
        Ok((
            path,
            Some(Self {
                ctx,
                req,
                now,
                etag: vl.map(|v| v.1),
            }),
        ))
    }
    pub async fn execute<P: AsRef<str>>(self, path: P) -> Result<()> {
        let ctx = self.ctx.as_ref();
        let path = path.as_ref();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        let Ok(mut resp) = client.execute(self.req).await else {
            bail!("failed to fetch url")
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
            debug!("downloaded to {}", path);
            ctx.db.set(path, "", &self.now.to_string(), &etag).await?;
        } else if resp.status().as_u16() == 304 {
            debug!("not modified in server, use cache {}", path);
            ctx.db
                .set(
                    path,
                    "",
                    &self.now.to_string(),
                    &self.etag.expect("etag should exists"),
                )
                .await?;
        } else {
            bail!("failed to fetch url, status: {}", resp.status())
        }
        Ok(())
    }
}
