use async_trait::async_trait;
use std::ffi::OsStr;
use std::io::{Error, Write};
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::debug;

use windows::Win32::Foundation::{HANDLE, MAX_PATH};
use windows::Win32::Storage::FileSystem::{ReadFile, SearchPathW, WriteFile};
use windows::Win32::System::Console::*;
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::*;
use windows::core::*;

use crate::Result;
use crate::core::*;

type ResizeFn = Box<dyn Send + Sync + Fn(HPCON, WindowSize) -> windows::core::Result<()>>;
type CreateFn = Box<dyn Fn(WindowSize, HANDLE, HANDLE) -> windows::core::Result<HPCON>>;
type CloseFn = Box<dyn Fn(HPCON)>;
struct ConptyApi {
    create: CreateFn,
    resize: ResizeFn,
    close: CloseFn,
}

struct PtyReaderImpl {
    prevent_deadlock: Arc<AtomicBool>,
    out: SafeHandle,
}

impl Drop for PtyReaderImpl {
    fn drop(&mut self) {
        while !self
            .prevent_deadlock
            .load(std::sync::atomic::Ordering::Acquire)
        {
            std::thread::yield_now();
        }
    }
}

impl AsyncRead for PtyReaderImpl {
    fn poll_read(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        let raw_buf = buf.initialize_unfilled();
        // let len = cmp::min(buf.len(), <u32>::max_value() as usize) as u32;
        let mut bytes = 0;
        unsafe { ReadFile(self.out.0, Some(raw_buf), Some(&mut bytes), None) }?;
        if bytes == 0 {
            debug!("EOF");
            return std::task::Poll::Ready(Ok(()));
        }
        debug!("read {} bytes", bytes);
        buf.advance(bytes as usize);
        std::task::Poll::Ready(Ok(()))
    }
}

impl PtyReader for PtyReaderImpl {}

struct PtyWriterImpl {
    prevent_deadlock: Arc<AtomicBool>,
    hpcon: HPCON,
    resizer: ResizeFn,
    in_: SafeHandle,
}

impl Drop for PtyWriterImpl {
    fn drop(&mut self) {
        while !self
            .prevent_deadlock
            .load(std::sync::atomic::Ordering::Acquire)
        {
            std::thread::yield_now();
        }
    }
}

impl AsyncWrite for PtyWriterImpl {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        // let len = cmp::min(buf.len(), <u32>::max_value() as usize) as u32;
        let mut bytes = 0;
        unsafe { WriteFile(self.in_.0, Some(buf), Some(&mut bytes), None) }?;
        std::task::Poll::Ready(Ok(bytes as usize))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
        unimplemented!()
    }
    fn is_write_vectored(&self) -> bool {
        false
    }
}

#[async_trait]
impl PtyWriter for PtyWriterImpl {
    async fn window_change(&self, width: u16, height: u16) -> Result<()> {
        (self.resizer)(
            self.hpcon,
            WindowSize {
                rows: height,
                cols: width,
            },
        )?;
        Ok(())
    }
}

pub struct PtyCtlImpl {
    prevent_deadlock: Arc<AtomicBool>,
    pub hpcon: HPCON,
    pub handle: HANDLE,
    pub close: CloseFn,
}

impl Drop for PtyCtlImpl {
    fn drop(&mut self) {
        (self.close)(self.hpcon);
        self.prevent_deadlock
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

unsafe impl Send for PtyCtlImpl {}
unsafe impl Sync for PtyCtlImpl {}

#[async_trait]
impl PtyCtl for PtyCtlImpl {
    async fn wait(&mut self) -> Result<i32> {
        let mut exit_code: u32 = 0;
        unsafe { WaitForSingleObject(self.handle, INFINITE) };
        unsafe { GetExitCodeProcess(self.handle, &mut exit_code as *mut u32) }?;
        debug!("exit code: {}", exit_code);
        Ok(exit_code as i32)
    }
}

impl ConptyApi {
    fn new() -> Self {
        Self {
            create: Box::new(
                |win_size: WindowSize, conin: HANDLE, conout: HANDLE| unsafe {
                    CreatePseudoConsole(win_size.into(), conin, conout, 0)
                },
            ),
            resize: Box::new(|hpcon: HPCON, win_size: WindowSize| unsafe {
                ResizePseudoConsole(hpcon, win_size.into())
            }),
            close: Box::new(|hpcon: HPCON| unsafe { ClosePseudoConsole(hpcon) }),
        }
    }
}

impl From<WindowSize> for COORD {
    fn from(size: WindowSize) -> Self {
        COORD {
            X: size.cols as i16,
            Y: size.rows as i16,
        }
    }
}

#[derive(Debug, Default)]
struct SafeHandle(HANDLE);
impl From<HANDLE> for SafeHandle {
    fn from(handle: HANDLE) -> Self {
        SafeHandle(handle)
    }
}

unsafe impl Send for SafeHandle {}
unsafe impl Sync for SafeHandle {}

impl Drop for SafeHandle {
    fn drop(&mut self) {
        unsafe {
            self.0.free();
        }
    }
}

pub fn openpty(window_size: WindowSize, command: Script<'_, '_>) -> std::io::Result<BoxedPty> {
    let api = ConptyApi::new();

    let mut conout = SafeHandle::default();
    let mut conout_pty_handle = SafeHandle::default();
    unsafe { CreatePipe(&mut conout.0, &mut conout_pty_handle.0, None, 0) }?;

    let mut conin_pty_handle = SafeHandle::default();
    let mut conin = SafeHandle::default();
    unsafe { CreatePipe(&mut conin_pty_handle.0, &mut conin.0, None, 0) }?;

    let pty_handle = (api.create)(window_size.clone(), conin_pty_handle.0, conout_pty_handle.0)?;
    debug!("Pseudoconsole created with handle {:?}", pty_handle);

    let mut startup_info_ex = STARTUPINFOEXW::default();

    startup_info_ex.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
    let mut size: usize = 0;

    startup_info_ex.StartupInfo.dwFlags |= STARTF_USESTDHANDLES;

    let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut size) };
    debug!("Attribute list size: {}", size);
    let mut attr_list: Box<[u8]> = vec![0; size].into_boxed_slice();

    #[allow(clippy::cast_ptr_alignment)]
    {
        startup_info_ex.lpAttributeList =
            LPPROC_THREAD_ATTRIBUTE_LIST(attr_list.as_mut_ptr() as *mut _);
    }

    unsafe {
        InitializeProcThreadAttributeList(
            Some(startup_info_ex.lpAttributeList),
            1,
            None,
            &mut size as *mut usize,
        )
    }?;

    debug!("Attribute list initialized");

    // Set thread attribute list's Pseudo Console to the specified ConPTY.
    unsafe {
        UpdateProcThreadAttribute(
            startup_info_ex.lpAttributeList,
            0,
            PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
            Some(pty_handle.0 as *mut _),
            mem::size_of::<HPCON>(),
            None,
            None,
        )
    }?;
    // Prepare child process creation arguments.
    let abs_path = |path: &str| {
        debug!("searching for {}", path);
        let mut program = vec![0; MAX_PATH as usize];
        let mut filename = path
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>();
        let len = unsafe {
            SearchPathW(
                None,
                PCWSTR::from_raw(filename.as_mut_ptr()),
                w!(".exe"),
                Some(program.as_mut_slice()),
                None,
            )
        };
        if len == 0 {
            return Err(Error::last_os_error());
        }
        unsafe {
            program.set_len(len as usize);
        }
        Ok(program)
    };
    let mut cmdline = match command {
        Script::Whole(cmd) => win32_string(cmd),
        Script::Split { program, args } => {
            let mut program = abs_path(program)?;
            for arg in args {
                program.push(' ' as u16);
                program.extend(arg.encode_utf16());
            }
            program.push(0);
            program
        }
        Script::Script { executor, input } => {
            let mut program = abs_path(executor.as_ref())?;
            let mut tmp = tempfile::NamedTempFile::with_suffix(".ps1")?;
            for line in input {
                tmp.write_all(line.as_bytes())?;
            }
            tmp.write_all(
                "\r\nRemove-Item $MyInvocation.MyCommand.Path\r\n"
                    .to_string()
                    .as_bytes(),
            )?;

            let path = tmp.into_temp_path().keep()?;
            program.extend(" -f ".encode_utf16());
            program.extend(path.as_os_str().encode_wide());
            program.push(0);
            debug!("command line: {}", String::from_utf16_lossy(&program));
            debug!("script content: {}", std::fs::read_to_string(&path)?);
            program
        }
    };
    let creation_flags = EXTENDED_STARTUPINFO_PRESENT;

    let mut proc_info = PROCESS_INFORMATION::default();

    unsafe {
        CreateProcessW(
            None,
            Some(PWSTR::from_raw(cmdline.as_mut_ptr())),
            None,
            None,
            false,
            creation_flags,
            None,
            None,
            &mut startup_info_ex.StartupInfo as *mut STARTUPINFOW,
            &mut proc_info as *mut PROCESS_INFORMATION,
        )
    }?;

    unsafe {
        DeleteProcThreadAttributeList(startup_info_ex.lpAttributeList);
    }
    let prevent_deadlock = Arc::new(AtomicBool::new(false));
    Ok(BoxedPty::new(
        PtyCtlImpl {
            prevent_deadlock: prevent_deadlock.clone(),
            hpcon: pty_handle,
            handle: proc_info.hProcess,
            close: api.close,
        },
        PtyWriterImpl {
            prevent_deadlock: prevent_deadlock.clone(),
            hpcon: pty_handle,
            resizer: api.resize,
            in_: conin,
        },
        PtyReaderImpl {
            prevent_deadlock,
            out: conout,
        },
    ))
}

pub fn win32_string<S: AsRef<OsStr> + ?Sized>(value: &S) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
