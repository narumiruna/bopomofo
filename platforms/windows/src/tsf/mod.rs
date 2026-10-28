//! STA-only COM implementation. All COM calls are made without RefCell borrows.
#![allow(non_snake_case)]

mod candidate;
mod edit;
mod key;
mod registration;
mod service;
mod window;

use crate::session::boundary;
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use windows::Win32::Foundation::*;
use windows::Win32::System::Com::*;
use windows::Win32::System::LibraryLoader::*;
use windows::core::*;

pub(super) const CLSID_SERVICE: GUID = GUID::from_u128(0xfadf9e21_04bb_4d07_b7a4_a4ed8ff4123c);
pub(super) const GUID_PROFILE: GUID = GUID::from_u128(0x83c941d1_d8e3_45dc_95a9_73ad918efef7);
pub(super) const GUID_CANDIDATES: GUID = GUID::from_u128(0x01d4f1cb_8c22_441e_a421_99173b25ae96);
pub(super) const LANGUAGE: u16 = 0x0404;

static OBJECTS: AtomicUsize = AtomicUsize::new(0);
static LOCKS: AtomicUsize = AtomicUsize::new(0);

pub(super) struct ModuleGuard;
impl ModuleGuard {
    pub fn new() -> Self {
        OBJECTS.fetch_add(1, Ordering::AcqRel);
        Self
    }
}
impl Drop for ModuleGuard {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) fn com<T>(call: impl FnOnce() -> Result<T>) -> Result<T> {
    boundary(call).unwrap_or_else(|_| Err(E_UNEXPECTED.into()))
}

pub(super) fn module_path() -> Result<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    let mut module = HMODULE::default();
    let mut path = vec![0u16; 32768];
    unsafe {
        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(DllGetClassObject as *const () as *const u16),
            &mut module,
        )?;
        let count = GetModuleFileNameW(Some(module), &mut path) as usize;
        if count == 0 || count >= path.len() {
            return Err(E_FAIL.into());
        }
        Ok(PathBuf::from(std::ffi::OsString::from_wide(&path[..count])))
    }
}

#[implement(IClassFactory, Agile = false)]
struct ClassFactory {
    _module: ModuleGuard,
}
impl IClassFactory_Impl for ClassFactory_Impl {
    fn CreateInstance(
        &self,
        outer: Ref<IUnknown>,
        iid: *const GUID,
        result: *mut *mut c_void,
    ) -> Result<()> {
        com(|| {
            if result.is_null() || iid.is_null() {
                return Err(E_POINTER.into());
            }
            unsafe { result.write(std::ptr::null_mut()) };
            if !outer.is_null() {
                return Err(CLASS_E_NOAGGREGATION.into());
            }
            let service: windows::Win32::UI::TextServices::ITfTextInputProcessor =
                service::Service::new().into();
            unsafe { service.query(iid, result).ok() }
        })
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        com(|| {
            if lock.as_bool() {
                LOCKS.fetch_add(1, Ordering::AcqRel);
                Ok(())
            } else {
                LOCKS
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                        count.checked_sub(1)
                    })
                    .map(|_| ())
                    .map_err(|_| E_UNEXPECTED.into())
            }
        })
    }
}

/// COM guarantees readable GUID pointers and a writable output pointer.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn DllGetClassObject(
    clsid: *const GUID,
    iid: *const GUID,
    result: *mut *mut c_void,
) -> HRESULT {
    com(|| {
        if clsid.is_null() || iid.is_null() || result.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { result.write(std::ptr::null_mut()) };
        if unsafe { *clsid } != CLSID_SERVICE {
            return Err(CLASS_E_CLASSNOTAVAILABLE.into());
        }
        let factory: IClassFactory = ClassFactory {
            _module: ModuleGuard::new(),
        }
        .into();
        unsafe { factory.query(iid, result).ok() }
    })
    .into()
}

#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> HRESULT {
    boundary(|| {
        if OBJECTS.load(Ordering::Acquire) == 0 && LOCKS.load(Ordering::Acquire) == 0 {
            S_OK
        } else {
            S_FALSE
        }
    })
    .unwrap_or(E_UNEXPECTED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::UI::TextServices::*;

    #[test]
    fn com_factory_interfaces_locks_and_idempotent_deactivation() {
        unsafe {
            assert_eq!(
                DllGetClassObject(std::ptr::null(), std::ptr::null(), std::ptr::null_mut()),
                E_POINTER
            );
            let mut pointer = std::ptr::null_mut();
            assert_eq!(
                DllGetClassObject(&GUID_PROFILE, &IClassFactory::IID, &mut pointer),
                CLASS_E_CLASSNOTAVAILABLE
            );
            assert!(pointer.is_null());
            assert_eq!(
                DllGetClassObject(&CLSID_SERVICE, &IClassFactory::IID, &mut pointer),
                S_OK
            );
            let factory = IClassFactory::from_raw(pointer);
            assert_eq!(DllCanUnloadNow(), S_FALSE);
            factory.LockServer(true).unwrap();
            factory.LockServer(false).unwrap();
            let outer: IUnknown = factory.cast().unwrap();
            let aggregated: Result<ITfTextInputProcessor> = factory.CreateInstance(&outer);
            assert_eq!(aggregated.unwrap_err().code(), CLASS_E_NOAGGREGATION);
            let service: ITfTextInputProcessor = factory.CreateInstance(None).unwrap();
            assert!(service.cast::<ITfKeyEventSink>().is_ok());
            assert!(service.cast::<ITfThreadMgrEventSink>().is_ok());
            assert!(service.cast::<IAgileObject>().is_err());
            assert_eq!(service.Activate(None, 0).unwrap_err().code(), E_POINTER);
            service.Deactivate().unwrap();
            service.Deactivate().unwrap();
        }
    }

    #[test]
    fn com_panic_maps_to_hresult() {
        let result: Result<()> = com(|| panic!("COM boundary test"));
        assert_eq!(result.unwrap_err().code(), E_UNEXPECTED);
    }
}
