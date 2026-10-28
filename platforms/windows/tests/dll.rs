//! Run after `cargo build -p ime-windows --target <native-target>`.
//! This loads the DLL without installing or registering the text service.
#![cfg(windows)]
use std::ffi::c_void;
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::IClassFactory;
use windows::Win32::System::LibraryLoader::*;
use windows::Win32::UI::TextServices::ITfTextInputProcessor;
use windows::core::*;

struct Library(HMODULE);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

#[test]
fn dll_exports_factory_and_unload_without_registration() {
    use std::os::windows::ffi::OsStrExt;
    let executable = std::env::current_exe().unwrap();
    let directory = executable.parent().unwrap().parent().unwrap();
    let path = directory.join("bopomofo_tsf.dll");
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let library = Library(
            LoadLibraryExW(
                PCWSTR(path.as_ptr()),
                None,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
            .unwrap(),
        );
        for name in [
            s!("DllGetClassObject"),
            s!("DllCanUnloadNow"),
            s!("DllRegisterServer"),
            s!("DllUnregisterServer"),
        ] {
            assert!(GetProcAddress(library.0, name).is_some());
        }
        let unload: unsafe extern "system" fn() -> HRESULT =
            std::mem::transmute(GetProcAddress(library.0, s!("DllCanUnloadNow")).unwrap());
        let get_factory: unsafe extern "system" fn(
            *const GUID,
            *const GUID,
            *mut *mut c_void,
        ) -> HRESULT =
            std::mem::transmute(GetProcAddress(library.0, s!("DllGetClassObject")).unwrap());
        assert_eq!(unload(), S_OK);
        let clsid = GUID::from_u128(0xfadf9e21_04bb_4d07_b7a4_a4ed8ff4123c);
        let mut pointer = std::ptr::null_mut();
        assert_eq!(get_factory(&clsid, &IClassFactory::IID, &mut pointer), S_OK);
        let factory = IClassFactory::from_raw(pointer);
        let service: ITfTextInputProcessor = factory.CreateInstance(None).unwrap();
        assert_eq!(unload(), S_FALSE);
        service.Deactivate().unwrap();
        drop(service);
        drop(factory);
        assert_eq!(unload(), S_OK);
    }
}
