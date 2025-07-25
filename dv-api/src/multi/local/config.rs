use os2::Os;

use super::{This, dev::*};

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
    info.variables.extend(std::env::vars());

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let name = if cfg!(target_os = "windows") {
            "USERNAME"
        } else if cfg!(target_os = "linux") {
            "USER"
        } else {
            unreachable!()
        };
        if let Some(p) = info.variables.get(name) {
            info.set("user", p.clone());
        }
    }

    info.is_system.get_or_insert_with(is_user_admin);

    let u: BoxedUser = This::new().await?.into();
    let os = os2::detect();
    match info.get("os") {
        Some(os_str) if Os::from(os_str).compatible(&os) => {}
        _ => {
            info.set("os", os.to_string());
        }
    }
    Ok(u)
}
