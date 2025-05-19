use os2::Os;
use tracing::warn;

use super::{This, dev::*};

fn detect() -> Os {
    if cfg!(target_os = "linux") {
        etc_os_release::OsRelease::open()
            .inspect_err(|e| warn!("can't open [/etc/os-release | /usr/lib/os-release]: {}", e))
            .map(|os_release| os_release.id().into())
            .unwrap_or("linux".into())
    } else if cfg!(target_os = "macos") {
        "macos".into()
    } else if cfg!(target_os = "windows") {
        "windows".into()
    } else {
        "unknown".into()
    }
}

#[cfg(windows)]
fn is_user_admin() -> bool {
    use windows_sys::Win32::Security::{
        AllocateAndInitializeSid, CheckTokenMembership, FreeSid, SECURITY_NT_AUTHORITY,
    };
    use windows_sys::Win32::System::SystemServices::{
        DOMAIN_ALIAS_RID_ADMINS, SECURITY_BUILTIN_DOMAIN_RID,
    };

    unsafe {
        let mut sid = std::ptr::null_mut();
        let success = AllocateAndInitializeSid(
            &SECURITY_NT_AUTHORITY,
            2,
            SECURITY_BUILTIN_DOMAIN_RID as u32,
            DOMAIN_ALIAS_RID_ADMINS as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut sid,
        ) != 0;

        if !success {
            return false;
        }

        let mut is_member = 0;
        let check_success = CheckTokenMembership(std::ptr::null_mut(), sid, &mut is_member) != 0;

        FreeSid(sid);

        check_success && is_member != 0
    }
}
#[cfg(not(windows))]
fn is_user_admin() -> bool {
    rustix::process::getuid().is_root()
}

pub async fn create(info: &mut Config) -> Result<BoxedUser> {
    if let Some(session) = {
        #[cfg(unix)]
        {
            std::env::var("XDG_SESSION_TYPE").ok()
        }
        #[cfg(target_os = "windows")]
        {
            None::<String>
        }
    } {
        info.set("SESSION", session);
    }
    if let Some(user) = {
        #[cfg(unix)]
        {
            std::env::var("USER").ok()
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var("USERNAME").ok()
        }
    } {
        info.set("USER", user);
    }
    info.is_system.get_or_insert_with(is_user_admin);

    let u: BoxedUser = This::new(info.is_system.unwrap()).await?.into();
    let os = detect();
    match info.get("os") {
        Some(os_str) if Os::from(os_str).compatible(&os) => {}
        _ => {
            info.set("os", os.to_string());
        }
    }
    Ok(u)
}
