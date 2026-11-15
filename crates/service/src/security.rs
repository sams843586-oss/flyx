//! Права доступа Windows для канала и рабочей папки.

use std::io;
use std::path::Path;
use std::ptr::null_mut;

use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
use windows_sys::Win32::Security::Authorization::{
    ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::CreateDirectoryW;

/// Канал: система, администраторы и владелец — полный доступ; вошедший в систему
/// пользователь — чтение и запись данных, но не создание экземпляров канала.
pub const PIPE_SDDL: &str = "D:(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;OW)(A;;0x12008b;;;IU)";

/// Папка с конфигами: в них ключи серверов, обычным пользователям доступа нет.
pub const PRIVATE_DIR_SDDL: &str = "D:P(A;OICI;GA;;;SY)(A;OICI;GA;;;BA)";

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Атрибуты безопасности из строки SDDL. Дескриптор живёт до конца процесса:
/// таких объектов создаётся два за всё время работы.
pub struct Security(SECURITY_ATTRIBUTES);

unsafe impl Send for Security {}
unsafe impl Sync for Security {}

impl Security {
    pub fn from_sddl(sddl: &str) -> io::Result<Self> {
        let mut descriptor = null_mut();
        let ok = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(sddl).as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Security(SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        }))
    }

    pub fn as_ptr(&self) -> *const SECURITY_ATTRIBUTES {
        &self.0
    }
}

/// Создаёт папку, закрытую от обычных пользователей. Родительская папка обязана
/// быть доступной на запись только администраторам, иначе её можно подменить заранее.
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    let security = Security::from_sddl(PRIVATE_DIR_SDDL)?;
    let name = wide(&path.to_string_lossy());
    if unsafe { CreateDirectoryW(name.as_ptr(), security.as_ptr()) } != 0 {
        return Ok(());
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS && path.is_dir() {
        return Ok(());
    }
    Err(io::Error::last_os_error())
}
