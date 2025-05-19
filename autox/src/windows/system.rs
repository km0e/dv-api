use std::{ffi::OsStr, ops::Deref, os::windows::ffi::OsStrExt};
use tracing::info;

use windows::{
    Win32::System::{Services::*, SystemInformation::GetTickCount},
    core::{Free, HRESULT, PCWSTR},
};

struct ServiceControlHandle(SC_HANDLE);

impl ServiceControlHandle {
    fn new(
        manager: SC_HANDLE,
        name: impl AsRef<OsStr>,
        access: u32,
    ) -> windows::core::Result<Self> {
        let name = name
            .as_ref()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let name = PCWSTR(name.as_ptr());
        Ok(unsafe { OpenServiceW(manager, name, access) }?.into())
    }
    fn delete(&self) -> windows::core::Result<()> {
        unsafe { DeleteService(self.0) }?;
        Ok(())
    }
    fn change(&self, ty: SERVICE_START_TYPE) -> windows::core::Result<()> {
        unsafe {
            ChangeServiceConfigW(
                self.0,
                ENUM_SERVICE_TYPE(SERVICE_NO_CHANGE),
                ty,
                SERVICE_ERROR(SERVICE_NO_CHANGE),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
        }?;
        Ok(())
    }
    fn refresh(&self, status: &mut SERVICE_STATUS_PROCESS) -> windows::core::Result<()> {
        unsafe {
            let mut bytes_needed: u32 = 0;
            QueryServiceStatusEx(
                self.0,
                SC_STATUS_PROCESS_INFO,
                Some(::core::slice::from_raw_parts_mut(
                    status as *mut _ as *mut u8,
                    size_of::<SERVICE_STATUS_PROCESS>(),
                )),
                &mut bytes_needed,
            )
        }
    }
    fn wait_not(
        &self,
        status: &mut SERVICE_STATUS_PROCESS,
        state: SERVICE_STATUS_CURRENT_STATE,
    ) -> windows::core::Result<()> {
        let mut check_point = unsafe { GetTickCount() };
        let mut old_check_point = status.dwCheckPoint;
        while status.dwCurrentState == state {
            let mut wait_time = status.dwWaitHint as u64 / 10;
            wait_time = wait_time.clamp(1000, 10000);
            std::thread::sleep(std::time::Duration::from_millis(wait_time));
            self.refresh(status)?;
            if status.dwCheckPoint > old_check_point {
                check_point = unsafe { GetTickCount() };
                old_check_point = status.dwCheckPoint;
            } else if unsafe { GetTickCount() } - check_point > status.dwWaitHint {
                return Err(windows::core::Error::new(
                    HRESULT(status.dwCurrentState.0 as i32),
                    format!(
                        "Service not {}",
                        match state {
                            SERVICE_STATUS_CURRENT_STATE(SERVICE_START) => "started",
                            SERVICE_STATUS_CURRENT_STATE(SERVICE_STOP) => "stopped",
                            _ => "changed",
                        }
                    ),
                ));
            }
        }
        Ok(())
    }
    fn start(&self) -> windows::core::Result<()> {
        unsafe { StartServiceW(self.0, None) }
    }
}

impl From<SC_HANDLE> for ServiceControlHandle {
    fn from(handle: SC_HANDLE) -> Self {
        Self(handle)
    }
}

impl From<ServiceControlHandle> for SC_HANDLE {
    fn from(handle: ServiceControlHandle) -> Self {
        handle.0
    }
}

impl Drop for ServiceControlHandle {
    fn drop(&mut self) {
        unsafe {
            self.0.free();
        }
    }
}

impl Deref for ServiceControlHandle {
    type Target = SC_HANDLE;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct AutoX {
    sch_scmanager: ServiceControlHandle,
}

unsafe impl Send for AutoX {}
unsafe impl Sync for AutoX {}

impl AutoX {
    pub fn new() -> windows::core::Result<Self> {
        info!("AutoX::new");
        let sch_scmanager =
            unsafe { OpenSCManagerW(PCWSTR::null(), PCWSTR::null(), SC_MANAGER_ALL_ACCESS) }?
                .into();
        Ok(Self { sch_scmanager })
    }
    fn install(&self, name: impl AsRef<str>, cmd: impl AsRef<str>) -> windows::core::Result<()> {
        let name = name
            .as_ref()
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let name = PCWSTR(name.as_ptr());
        let cmd = cmd
            .as_ref()
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let cmd = PCWSTR(cmd.as_ptr());
        let _: ServiceControlHandle = unsafe {
            CreateServiceW(
                *self.sch_scmanager,
                name,
                name,
                SERVICE_ALL_ACCESS,
                SERVICE_WIN32_OWN_PROCESS,
                SERVICE_DEMAND_START,
                SERVICE_ERROR_NORMAL,
                cmd,
                None,
                None,
                None,
                None,
                None,
            )
        }?
        .into();
        Ok(())
    }
    pub fn uninstall(&self, name: impl AsRef<OsStr>) -> windows::core::Result<()> {
        ServiceControlHandle::new(*self.sch_scmanager, name, SERVICE_ALL_ACCESS)?.delete()
    }
    pub fn enable(&self, name: impl AsRef<OsStr>) -> windows::core::Result<()> {
        ServiceControlHandle::new(*self.sch_scmanager, name, SERVICE_CHANGE_CONFIG)?
            .change(SERVICE_AUTO_START)
    }
    pub fn disable(&self, name: impl AsRef<OsStr>) -> windows::core::Result<()> {
        ServiceControlHandle::new(*self.sch_scmanager, name, SERVICE_CHANGE_CONFIG)?
            .change(SERVICE_DEMAND_START)
    }
    pub fn start(&self, name: impl AsRef<OsStr>) -> windows::core::Result<()> {
        let sch_service = ServiceControlHandle::new(*self.sch_scmanager, name, SERVICE_START)?;
        let mut status = SERVICE_STATUS_PROCESS::default();
        sch_service.refresh(&mut status)?;
        if status.dwCurrentState == SERVICE_STATUS_CURRENT_STATE(SERVICE_START) {
            return Ok(());
        }
        sch_service.wait_not(&mut status, SERVICE_STOP_PENDING)?;
        sch_service.start()?;
        sch_service.refresh(&mut status)?;
        if status.dwCurrentState == SERVICE_STATUS_CURRENT_STATE(SERVICE_START) {
            return Ok(());
        }
        sch_service.wait_not(&mut status, SERVICE_START_PENDING)?;
        if status.dwCurrentState != SERVICE_STATUS_CURRENT_STATE(SERVICE_START) {
            return Err(windows::core::Error::new(
                HRESULT(status.dwCurrentState.0 as i32),
                "Service not started",
            ));
        }
        Ok(())
    }
    pub fn stop(&self, name: impl AsRef<OsStr>) -> windows::core::Result<()> {
        let sch_service = ServiceControlHandle::new(
            *self.sch_scmanager,
            name,
            SERVICE_STOP | SERVICE_QUERY_STATUS,
        )?;
        let mut status = SERVICE_STATUS_PROCESS::default();
        sch_service.refresh(&mut status)?;
        if status.dwCurrentState == SERVICE_STOPPED {
            return Ok(());
        }
        sch_service.wait_not(&mut status, SERVICE_START_PENDING)?;
        // TODO: stop dependents
        let mut p = SERVICE_CONTROL_STATUS_REASON_PARAMSW::default();
        unsafe {
            ControlServiceExW(
                *sch_service,
                SERVICE_CONTROL_STOP,
                SERVICE_CONTROL_STATUS_REASON_INFO,
                &mut p as *mut _ as *mut _,
            )?
        };
        status.dwCurrentState = p.ServiceStatus.dwCurrentState;
        sch_service.wait_not(&mut status, SERVICE_START_PENDING)?;
        if status.dwCurrentState != SERVICE_STOPPED {
            return Err(windows::core::Error::new(
                HRESULT(status.dwCurrentState.0 as i32),
                "Service not stopped",
            ));
        }
        Ok(())
    }
    pub fn setup(&self, name: impl AsRef<str>, cmd: impl AsRef<str>) -> windows::core::Result<()> {
        let name = name.as_ref();
        self.install(name, cmd)?;
        self.enable(name)?;
        self.start(name)?;
        Ok(())
    }
    pub fn destroy(&self, name: impl AsRef<str>) -> windows::core::Result<()> {
        let name = name.as_ref();
        self.stop(name)?;
        self.uninstall(name)?;
        Ok(())
    }
    pub fn reload(&self, name: impl AsRef<str>) -> windows::core::Result<()> {
        let name = name.as_ref();
        let sch_service = ServiceControlHandle::new(
            *self.sch_scmanager,
            name,
            SERVICE_STOP | SERVICE_QUERY_STATUS | SERVICE_PAUSE_CONTINUE,
        )?;
        let mut status = SERVICE_STATUS_PROCESS::default();
        sch_service.refresh(&mut status)?;
        if status.dwCurrentState == SERVICE_STOPPED {
            return self.start(name);
        }
        sch_service.wait_not(&mut status, SERVICE_START_PENDING)?;
        // TODO: stop dependents
        let mut p = SERVICE_CONTROL_STATUS_REASON_PARAMSW::default();
        unsafe {
            ControlServiceExW(
                *sch_service,
                SERVICE_CONTROL_PARAMCHANGE,
                SERVICE_CONTROL_STATUS_REASON_INFO,
                &mut p as *mut _ as *mut _,
            )?
        };
        status.dwCurrentState = p.ServiceStatus.dwCurrentState;
        sch_service.wait_not(&mut status, SERVICE_START_PENDING)?;
        if status.dwCurrentState != SERVICE_STOPPED {
            return Err(windows::core::Error::new(
                HRESULT(status.dwCurrentState.0 as i32),
                "Service not stopped",
            ));
        }
        Ok(())
    }
}
