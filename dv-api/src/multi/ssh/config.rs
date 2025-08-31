use std::{collections::HashMap, sync::Arc};

use os2::Os;
use russh::client::{self, AuthResult, Handle};
use tokio::io::AsyncReadExt;
use tracing::{debug, info, warn};

use crate::whatever;

use super::{Client, SSHSession, dev::*};

pub async fn create(host: String, cfg: &mut Config) -> Result<BoxedUser> {
    let (clients, user) = connect(&host, cfg.get("passwd").cloned()).await?;
    if cfg.get("user").is_none() {
        cfg.set("user", user.clone());
    }
    let os = cfg.get("os").map(|s| s.as_str()).unwrap_or("");
    let mut os = os.into();
    let client = clients.last().expect("no session");
    let env = detect2(client, &mut os).await?;
    let channel = client.channel_open_session().await?;
    channel.request_subsystem(true, "sftp").await?;
    let sftp = russh_sftp::client::SftpSession::new(channel.into_stream()).await?;

    let home = match os {
        Os::Linux(_) | Os::MacOs | Os::Unix => env.get("HOME"),
        Os::Windows => env.get("HOMEPATH"),
        _ => None,
    }
    .cloned();
    cfg.variables.extend(env);
    cfg.set("os", os.to_string());
    let sys = SSHSession {
        session: clients,
        sftp,
        home,
    };
    let u: BoxedUser = sys.into();
    cfg.is_system.get_or_insert_default();
    Ok(u)
}

async fn connect_impl(
    cfg: &russh_config::Config,
    clients: &mut Vec<Handle<Client>>,
    passwd: Option<String>,
) -> Result<()> {
    let session = if let Some(jh) = &cfg.proxy_jump {
        let host_cfg = russh_config::parse_home(jh)?; //with host
        Box::pin(connect_impl(&host_cfg, clients, None)).await?;
        debug!("proxy jump to {}", jh);
        let s = clients
            .last()
            .unwrap()
            .channel_open_direct_tcpip(cfg.host_name.clone(), cfg.port as u32, "127.0.0.1", 0)
            .await?
            .into_stream();
        client::connect_stream(Arc::new(client::Config::default()), s, Client {}).await?
    } else {
        client::connect(
            Arc::new(client::Config::default()),
            (cfg.host_name.clone(), cfg.port),
            Client {},
        )
        .await?
    };
    clients.push(session);
    let session = clients.last_mut().unwrap();
    let mut res = session.authenticate_none(&cfg.user).await?;
    let AuthResult::Failure {
        mut remaining_methods,
        ..
    } = res
    else {
        return Ok(());
    };
    warn!("authenticate_none failed");
    use russh::{MethodKind, keys};
    if let (Some(path), true) = (
        &cfg.identity_file,
        remaining_methods.contains(&MethodKind::PublicKey),
    ) {
        let kp = keys::load_secret_key(path, None)?;
        let private_key = keys::PrivateKeyWithHashAlg::new(Arc::new(kp), None);
        res = session
            .authenticate_publickey(&cfg.user, private_key)
            .await?;
        let AuthResult::Failure {
            remaining_methods: s,
            ..
        } = res
        else {
            return Ok(());
        };
        remaining_methods = s;
        warn!("authenticate_publickey with {} failed", path);
    }
    if let (Some(passwd), true) = (passwd, remaining_methods.contains(&MethodKind::Password)) {
        res = session.authenticate_password(&cfg.user, passwd).await?;
        if res.success() {
            return Ok(());
        }
        warn!("authenticate_password failed");
    }
    whatever!("ssh connect {} {} failed", cfg.host_name, cfg.user)
}

async fn connect(host: &str, passwd: Option<String>) -> Result<(Vec<Handle<Client>>, String)> {
    let host_cfg = russh_config::parse_home(host)?; //with host
    let mut clients = vec![];
    connect_impl(&host_cfg, &mut clients, passwd).await?;
    Ok((clients, host_cfg.user))
}

async fn detect2(h: &Handle<Client>, os: &mut Os) -> Result<HashMap<String, String>> {
    info!("detecting os: {}", os);
    if os.is_linux() {
        detect(h, os).await
    } else {
        warn!("{} os not supported", os);
        Ok(Default::default())
    }
}
async fn detect(h: &Handle<Client>, os: &mut Os) -> Result<HashMap<String, String>> {
    async fn _extract(
        h: &Handle<Client>,
        cmd: &str,
        mut insert: impl FnMut(&str, &str),
    ) -> std::result::Result<(), russh::Error> {
        let mut channel = h.channel_open_session().await?;
        channel.exec(true, cmd).await?;
        let mut output = String::with_capacity(1024);
        channel.make_reader().read_to_string(&mut output).await?;

        for line in output.split('\n') {
            let mut kv = line.splitn(2, '=');
            let Some(key) = kv.next() else {
                continue;
            };
            let Some(value) = kv.next() else {
                continue;
            };
            insert(key, value);
        }
        Ok(())
    }
    async fn extract_special<const S: usize>(
        h: &Handle<Client>,
        cmd: &str,
        keys: &[&str; S],
    ) -> std::result::Result<[Option<String>; S], russh::Error> {
        let mut values = [const { None }; S];
        _extract(h, cmd, |key, value| {
            if let Some(i) = keys.iter().position(|&k| key == k) {
                values[i] = Some(value.to_string());
            }
        })
        .await?;
        Ok(values)
    }
    async fn extract_all(
        h: &Handle<Client>,
        cmd: &str,
    ) -> std::result::Result<HashMap<String, String>, russh::Error> {
        let mut map = HashMap::new();
        _extract(h, cmd, |key, value| {
            map.insert(key.to_string(), value.to_string());
        })
        .await?;
        Ok(map)
    }

    let env = extract_all(h, "env").await?;

    let [os_d] = extract_special(
        h,
        "sh -c 'cat /etc/os-release 2>/dev/null || cat /usr/lib/os-release 2>/dev/null'",
        &["ID"],
    )
    .await?;
    debug!("os_d: {:?}", os_d);
    if let Some(os_d) = os_d {
        *os = os_d.into();
    }

    Ok(env)
}
