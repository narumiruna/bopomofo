//! Registration exports are called only by the explicit authorization scripts.
use super::{CLSID_SERVICE, GUID_PROFILE, LANGUAGE, com, module_path};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::Registry::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

const SERVER_KEY: PCWSTR =
    w!("SOFTWARE\\Classes\\CLSID\\{FADF9E21-04BB-4D07-B7A4-A4ED8FF4123C}\\InprocServer32");
const CATEGORIES: [GUID; 2] = [GUID_TFCAT_TIP_KEYBOARD, GUID_TFCAT_TIPCAP_UIELEMENTENABLED];

struct Apartment(bool);
impl Apartment {
    fn new() -> Result<Self> {
        let code = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if code == RPC_E_CHANGED_MODE {
            return Ok(Self(false));
        }
        code.ok()?;
        Ok(Self(true))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn registry_view() -> REG_SAM_FLAGS {
    if cfg!(target_pointer_width = "64") {
        KEY_WOW64_64KEY
    } else {
        KEY_WOW64_32KEY
    }
}

fn write_string(key: HKEY, name: PCWSTR, text: &[u16]) -> Result<()> {
    let bytes = unsafe {
        std::slice::from_raw_parts(text.as_ptr().cast::<u8>(), std::mem::size_of_val(text))
    };
    unsafe { RegSetValueExW(key, name, None, REG_SZ, Some(bytes)).ok() }
}

fn register_com() -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let path: Vec<_> = module_path()?
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Refuse an unrelated or previous installation instead of overwriting its path.
    let mut existing = vec![0u16; 32768];
    let mut size = (existing.len() * 2) as u32;
    let flags = RRF_RT_REG_SZ
        | if cfg!(target_pointer_width = "64") {
            RRF_SUBKEY_WOW6464KEY
        } else {
            RRF_SUBKEY_WOW6432KEY
        };
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            SERVER_KEY,
            PCWSTR::null(),
            flags,
            None,
            Some(existing.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    if status == ERROR_SUCCESS {
        let units = size as usize / 2;
        if units != path.len() || existing[..units] != path {
            return Err(HRESULT::from_win32(ERROR_ALREADY_EXISTS.0).into());
        }
    } else if status != ERROR_FILE_NOT_FOUND {
        return status.ok();
    }
    let mut raw = HKEY::default();
    unsafe {
        RegCreateKeyExW(
            HKEY_LOCAL_MACHINE,
            SERVER_KEY,
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE | registry_view(),
            None,
            &mut raw,
            None,
        )
        .ok()?;
    }
    let key = Key(raw);
    write_string(key.0, PCWSTR::null(), &path)?;
    write_string(
        key.0,
        w!("ThreadingModel"),
        &"Apartment\0".encode_utf16().collect::<Vec<_>>(),
    )
}

fn unregister_com() -> Result<()> {
    // Open the exact registry view before deleting only this service's CLSID.
    let mut raw = HKEY::default();
    let status = unsafe {
        RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Classes\\CLSID"),
            None,
            KEY_WRITE | KEY_READ | registry_view(),
            &mut raw,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok()?;
    let key = Key(raw);
    let result = unsafe { RegDeleteTreeW(key.0, w!("{FADF9E21-04BB-4D07-B7A4-A4ED8FF4123C}")) };
    if result == ERROR_FILE_NOT_FOUND {
        Ok(())
    } else {
        result.ok()
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllRegisterServer() -> HRESULT {
    com(|| {
        let _apartment = Apartment::new()?;
        register_com()?;
        let result = (|| {
            let profiles: ITfInputProcessorProfiles = unsafe {
                CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)?
            };
            let categories: ITfCategoryMgr =
                unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
            let description: Vec<_> = "bopomofo (development fixture)".encode_utf16().collect();
            let icon: Vec<_> = Vec::new();
            unsafe {
                profiles.Register(&CLSID_SERVICE)?;
                profiles.AddLanguageProfile(
                    &CLSID_SERVICE,
                    LANGUAGE,
                    &GUID_PROFILE,
                    &description,
                    &icon,
                    0,
                )?;
                for category in CATEGORIES {
                    categories.RegisterCategory(&CLSID_SERVICE, &category, &CLSID_SERVICE)?;
                }
            }
            Ok(())
        })();
        if result.is_err() {
            // Roll back only our fixed identifiers after a partial registration.
            let _ = unregister_tsf();
            let _ = unregister_com();
        }
        result
    })
    .into()
}

fn unregister_tsf() -> Result<()> {
    let profiles: ITfInputProcessorProfiles =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)? };
    let categories: ITfCategoryMgr =
        unsafe { CoCreateInstance(&CLSID_TF_CategoryMgr, None, CLSCTX_INPROC_SERVER)? };
    let mut error = None;
    unsafe {
        for category in CATEGORIES {
            if let Err(current) =
                categories.UnregisterCategory(&CLSID_SERVICE, &category, &CLSID_SERVICE)
            {
                error = Some(current);
            }
        }
        if let Err(current) = profiles.Unregister(&CLSID_SERVICE) {
            error = Some(current);
        }
    }
    error.map_or(Ok(()), Err)
}

#[unsafe(no_mangle)]
pub extern "system" fn DllUnregisterServer() -> HRESULT {
    com(|| {
        let _apartment = Apartment::new()?;
        let tsf = unregister_tsf();
        let registry = unregister_com();
        tsf.and(registry)
    })
    .into()
}
