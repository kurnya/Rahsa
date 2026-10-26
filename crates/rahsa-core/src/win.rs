//! Small Win32 helpers shared by the desktop app and the native host.

use std::ffi::c_void;
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, BOOL, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
};
use windows_sys::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows_sys::Win32::System::StationsAndDesktops::{
    CloseDesktop, GetUserObjectInformationW, OpenInputDesktop, DESKTOP_READOBJECTS, UOI_NAME,
};
use windows_sys::Win32::System::SystemInformation::GetTickCount;
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use zeroize::Zeroizing;

const CF_UNICODETEXT: u32 = 13;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn from_wide_ptr(p: *const u16) -> String {
    let mut len = 0;
    while *p.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, len))
}

/// String SID of the user running this process, e.g. `S-1-5-21-…`.
pub fn current_user_sid() -> std::io::Result<String> {
    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut len = 0u32;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let ok = GetTokenInformation(token, TokenUser, buf.as_mut_ptr() as *mut c_void, len, &mut len);
        CloseHandle(token);
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut s: *mut u16 = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut s) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let out = from_wide_ptr(s);
        LocalFree(s as *mut c_void);
        Ok(out)
    }
}

/// Named pipe used between `rahsa-host.exe` and the app. Per user, so two
/// people logged into the same machine never see each other's pipe.
pub fn pipe_name() -> std::io::Result<String> {
    Ok(format!(r"\\.\pipe\rahsa-{}", current_user_sid()?))
}

/// Security attributes granting access only to the current user (and nobody
/// else — not even Administrators or SYSTEM via the DACL).
pub struct OwnerOnlySecurity {
    sd: PSECURITY_DESCRIPTOR,
    pub attrs: SECURITY_ATTRIBUTES,
}

unsafe impl Send for OwnerOnlySecurity {}
unsafe impl Sync for OwnerOnlySecurity {}

impl OwnerOnlySecurity {
    pub fn new() -> std::io::Result<Box<OwnerOnlySecurity>> {
        let sddl = wide(&format!("D:P(A;;GA;;;{})", current_user_sid()?));
        let mut sd: PSECURITY_DESCRIPTOR = null_mut();
        unsafe {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), 1, &mut sd, null_mut()) == 0 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(Box::new(OwnerOnlySecurity {
            sd,
            attrs: SECURITY_ATTRIBUTES {
                nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd,
                bInheritHandle: 0 as BOOL,
            },
        }))
    }
}

impl Drop for OwnerOnlySecurity {
    fn drop(&mut self) {
        unsafe {
            LocalFree(self.sd as *mut c_void);
        }
    }
}

/// Full image path of a process, used to verify the pipe client is our host.
pub fn process_image_path(pid: u32) -> std::io::Result<PathBuf> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let mut buf = vec![0u16; 32768];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(PathBuf::from(String::from_utf16_lossy(&buf[..len as usize])))
    }
}

/// DPAPI, current-user scope. Used for the pairing store (keys the app needs
/// while the vault is locked) — never for the vault key in the MVP.
pub fn dpapi_protect(data: &[u8]) -> std::io::Result<Vec<u8>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        let desc = wide("Rahsa");
        if CryptProtectData(&input, desc.as_ptr(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let v = std::slice::from_raw_parts(out.pbData, out.cbData as usize).to_vec();
        LocalFree(out.pbData as *mut c_void);
        Ok(v)
    }
}

pub fn dpapi_unprotect(data: &[u8]) -> std::io::Result<Zeroizing<Vec<u8>>> {
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        if CryptUnprotectData(&input, null_mut(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let slice = std::slice::from_raw_parts_mut(out.pbData, out.cbData as usize);
        let v = Zeroizing::new(slice.to_vec());
        slice.iter_mut().for_each(|b| *b = 0);
        LocalFree(out.pbData as *mut c_void);
        Ok(v)
    }
}

/// Milliseconds since the last keyboard/mouse input anywhere in the session.
pub fn idle_millis() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut lii) == 0 {
            return 0;
        }
        GetTickCount().wrapping_sub(lii.dwTime) as u64
    }
}

/// True while the Windows lock screen (Winlogon desktop) is active.
pub fn is_workstation_locked() -> bool {
    unsafe {
        let desk = OpenInputDesktop(0, 0, DESKTOP_READOBJECTS);
        if desk.is_null() {
            return true;
        }
        let mut buf = [0u16; 64];
        let mut needed = 0u32;
        let ok = GetUserObjectInformationW(
            desk as HANDLE,
            UOI_NAME,
            buf.as_mut_ptr() as *mut c_void,
            (buf.len() * 2) as u32,
            &mut needed,
        );
        CloseDesktop(desk);
        if ok == 0 {
            return false;
        }
        let name = from_wide_ptr(buf.as_ptr());
        !name.eq_ignore_ascii_case("Default")
    }
}

fn with_clipboard<T>(f: impl FnOnce() -> T) -> std::io::Result<T> {
    unsafe {
        for _ in 0..10 {
            if OpenClipboard(null_mut()) != 0 {
                let r = f();
                CloseClipboard();
                return Ok(r);
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        Err(std::io::Error::last_os_error())
    }
}

unsafe fn set_global(format: u32, bytes: &[u8]) -> bool {
    let h = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1));
    if h.is_null() {
        return false;
    }
    let p = GlobalLock(h) as *mut u8;
    if p.is_null() {
        return false;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
    GlobalUnlock(h);
    // Ownership passes to the system on success.
    !SetClipboardData(format, h as HANDLE).is_null()
}

/// Put text on the clipboard, excluded from Win+V history and cloud sync.
/// Returns the clipboard sequence number so the caller can later clear it only
/// if nobody has copied something else in the meantime.
pub fn clipboard_set_secret(text: &str) -> std::io::Result<u32> {
    let utf16: Zeroizing<Vec<u8>> =
        Zeroizing::new(text.encode_utf16().chain(std::iter::once(0)).flat_map(|u| u.to_le_bytes()).collect());
    let ok = with_clipboard(|| unsafe {
        EmptyClipboard();
        let exclude = RegisterClipboardFormatW(wide("ExcludeClipboardContentFromMonitorProcessing").as_ptr());
        let history = RegisterClipboardFormatW(wide("CanIncludeInClipboardHistory").as_ptr());
        let cloud = RegisterClipboardFormatW(wide("CanUploadToCloudClipboard").as_ptr());
        let zero = 0u32.to_le_bytes();
        set_global(exclude, &zero);
        set_global(history, &zero);
        set_global(cloud, &zero);
        set_global(CF_UNICODETEXT, &utf16)
    })?;
    if !ok {
        return Err(std::io::Error::other("SetClipboardData gagal"));
    }
    Ok(unsafe { GetClipboardSequenceNumber() })
}

/// Clear the clipboard if it still holds what we put there.
pub fn clipboard_clear_if(seq: u32) {
    unsafe {
        if GetClipboardSequenceNumber() != seq {
            return;
        }
    }
    let _ = with_clipboard(|| unsafe {
        EmptyClipboard();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sid_and_pipe_name() {
        let sid = current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"));
        assert!(pipe_name().unwrap().ends_with(&sid));
    }

    #[test]
    fn dpapi_round_trip() {
        let c = dpapi_protect(b"pairing").unwrap();
        assert_ne!(c, b"pairing");
        assert_eq!(&dpapi_unprotect(&c).unwrap()[..], b"pairing");
    }

    #[test]
    fn own_process_path() {
        let p = process_image_path(std::process::id()).unwrap();
        assert_eq!(p, std::env::current_exe().unwrap());
    }
}
