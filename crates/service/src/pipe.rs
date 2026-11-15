//! Сервер именованного канала: клиент присылает строку, получает строку, соединение закрывается.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use flexconnect_core::ipc::{open_client, MAX_REQUEST, PIPE_NAME};
use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_PIPE_CONNECTED, HANDLE, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};

use crate::security::{wide, Security, PIPE_SDDL};

/// Сколько клиентов обслуживается одновременно; остальным сразу отказ.
const MAX_CLIENTS: usize = 16;

fn read_line(pipe: &mut File) -> io::Result<String> {
    let mut data = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = pipe.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        data.extend_from_slice(&chunk[..n]);
        if let Some(end) = data.iter().position(|b| *b == b'\n') {
            data.truncate(end);
            break;
        }
        if data.len() > MAX_REQUEST {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "запрос слишком длинный"));
        }
    }
    String::from_utf8(data).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "запрос не в UTF-8"))
}

fn serve_client(mut pipe: File, handler: &(dyn Fn(&str) -> String + Send + Sync)) {
    if let Ok(line) = read_line(&mut pipe) {
        let mut reply = handler(line.trim());
        reply.push('\n');
        if pipe.write_all(reply.as_bytes()).is_ok() {
            // ждём, пока клиент дочитает ответ, и только потом рвём соединение
            pipe.sync_data().ok();
        }
    }
    unsafe { DisconnectNamedPipe(pipe.as_raw_handle() as HANDLE) };
}

/// Принимает клиентов, пока не поднят флаг `stop`. Чтобы цикл заметил флаг,
/// после его установки нужно вызвать [`wake`].
pub fn serve<F>(stop: Arc<AtomicBool>, handler: F) -> io::Result<()>
where
    F: Fn(&str) -> String + Send + Sync + 'static,
{
    let security = Security::from_sddl(PIPE_SDDL)?;
    let name = wide(PIPE_NAME);
    let handler = Arc::new(handler);
    let active = Arc::new(AtomicUsize::new(0));
    let mut first = true;

    loop {
        // Флаг «первый экземпляр» не даёт запуститься, если канал с этим именем
        // уже занял другой процесс.
        let open_mode = PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
        let handle = unsafe {
            CreateNamedPipeW(
                name.as_ptr(),
                open_mode,
                PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                PIPE_UNLIMITED_INSTANCES,
                MAX_REQUEST as u32,
                MAX_REQUEST as u32,
                0,
                security.as_ptr(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        first = false;
        let pipe = unsafe { File::from_raw_handle(handle as _) };

        let connected = unsafe { ConnectNamedPipe(handle, null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        if stop.load(Ordering::SeqCst) {
            return Ok(());
        }
        if !connected {
            continue;
        }
        if active.fetch_add(1, Ordering::SeqCst) >= MAX_CLIENTS {
            active.fetch_sub(1, Ordering::SeqCst);
            unsafe { DisconnectNamedPipe(handle) };
            continue;
        }
        let handler = handler.clone();
        let active = active.clone();
        std::thread::spawn(move || {
            serve_client(pipe, &*handler);
            active.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

/// Будит цикл [`serve`], подключаясь к каналу как клиент.
pub fn wake() {
    drop(open_client());
}
