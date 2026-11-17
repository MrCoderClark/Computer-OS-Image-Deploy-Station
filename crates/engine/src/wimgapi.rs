use anyhow::{Context, Result, bail};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use tracing::info;

type HANDLE = *mut std::ffi::c_void;
type DWORD = u32;
type BOOL = i32;
type LPCWSTR = *const u16;

const WIM_GENERIC_READ: DWORD = 0x80000000;
const WIM_GENERIC_WRITE: DWORD = 0x40000000;
const WIM_OPEN_EXISTING: DWORD = 3;
const WIM_CREATE_ALWAYS: DWORD = 2;
const WIM_COMPRESS_XPRESS: DWORD = 1;
#[allow(dead_code)]
const WIM_COMPRESS_LZX: DWORD = 2;

type WIMCreateFileFn = unsafe extern "system" fn(
    LPCWSTR, DWORD, DWORD, DWORD, DWORD, *mut DWORD,
) -> HANDLE;
type WIMSetTemporaryPathFn = unsafe extern "system" fn(HANDLE, LPCWSTR) -> BOOL;
type WIMLoadImageFn = unsafe extern "system" fn(HANDLE, DWORD) -> HANDLE;
type WIMApplyImageFn = unsafe extern "system" fn(HANDLE, LPCWSTR, DWORD) -> BOOL;
type WIMCaptureImageFn = unsafe extern "system" fn(HANDLE, LPCWSTR, DWORD) -> HANDLE;
type WIMCloseHandleFn = unsafe extern "system" fn(HANDLE) -> BOOL;

pub struct WimSession {
    lib: libloading::Library,
}

impl WimSession {
    pub fn load() -> Result<Self> {
        let lib = unsafe {
            libloading::Library::new("wimgapi.dll")
                .context("Failed to load wimgapi.dll — is this running in WinPE or a system with the ADK?")?
        };
        info!("WIMGAPI loaded successfully");
        Ok(Self { lib })
    }

    pub fn apply_image(
        &self,
        wim_path: &Path,
        image_index: u32,
        target_path: &Path,
    ) -> Result<()> {
        info!(
            "Applying image index {} from {} to {}",
            image_index,
            wim_path.display(),
            target_path.display()
        );

        let wim_path_w = to_wide(wim_path.as_os_str());
        let target_path_w = to_wide(target_path.as_os_str());
        let temp_dir = std::env::temp_dir();
        let temp_path_w = to_wide(temp_dir.as_os_str());

        unsafe {
            let create_file: libloading::Symbol<WIMCreateFileFn> =
                self.lib.get(b"WIMCreateFile\0").context("WIMCreateFile not found")?;
            let set_temp: libloading::Symbol<WIMSetTemporaryPathFn> =
                self.lib.get(b"WIMSetTemporaryPath\0").context("WIMSetTemporaryPath not found")?;
            let load_image: libloading::Symbol<WIMLoadImageFn> =
                self.lib.get(b"WIMLoadImage\0").context("WIMLoadImage not found")?;
            let apply_image: libloading::Symbol<WIMApplyImageFn> =
                self.lib.get(b"WIMApplyImage\0").context("WIMApplyImage not found")?;
            let close_handle: libloading::Symbol<WIMCloseHandleFn> =
                self.lib.get(b"WIMCloseHandle\0").context("WIMCloseHandle not found")?;

            let mut creation_result: DWORD = 0;
            let wim_handle = create_file(
                wim_path_w.as_ptr(),
                WIM_GENERIC_READ,
                WIM_OPEN_EXISTING,
                0,
                0,
                &mut creation_result,
            );

            if wim_handle.is_null() {
                bail!("WIMCreateFile failed for {}", wim_path.display());
            }

            let result = set_temp(wim_handle, temp_path_w.as_ptr());
            if result == 0 {
                close_handle(wim_handle);
                bail!("WIMSetTemporaryPath failed");
            }

            let image_handle = load_image(wim_handle, image_index);
            if image_handle.is_null() {
                close_handle(wim_handle);
                bail!("WIMLoadImage failed for index {image_index}");
            }

            info!("Applying image to {}...", target_path.display());
            let result = apply_image(image_handle, target_path_w.as_ptr(), 0);
            if result == 0 {
                close_handle(image_handle);
                close_handle(wim_handle);
                bail!("WIMApplyImage failed");
            }

            info!("Image applied successfully");
            close_handle(image_handle);
            close_handle(wim_handle);
        }

        Ok(())
    }

    pub fn capture_image(
        &self,
        wim_path: &Path,
        source_path: &Path,
    ) -> Result<()> {
        info!(
            "Capturing {} to {}",
            source_path.display(),
            wim_path.display()
        );

        let wim_path_w = to_wide(wim_path.as_os_str());
        let source_path_w = to_wide(source_path.as_os_str());
        let temp_dir = std::env::temp_dir();
        let temp_path_w = to_wide(temp_dir.as_os_str());

        unsafe {
            let create_file: libloading::Symbol<WIMCreateFileFn> =
                self.lib.get(b"WIMCreateFile\0").context("WIMCreateFile not found")?;
            let set_temp: libloading::Symbol<WIMSetTemporaryPathFn> =
                self.lib.get(b"WIMSetTemporaryPath\0").context("WIMSetTemporaryPath not found")?;
            let capture_image: libloading::Symbol<WIMCaptureImageFn> =
                self.lib.get(b"WIMCaptureImage\0").context("WIMCaptureImage not found")?;
            let close_handle: libloading::Symbol<WIMCloseHandleFn> =
                self.lib.get(b"WIMCloseHandle\0").context("WIMCloseHandle not found")?;

            let mut creation_result: DWORD = 0;
            let wim_handle = create_file(
                wim_path_w.as_ptr(),
                WIM_GENERIC_WRITE,
                WIM_CREATE_ALWAYS,
                WIM_COMPRESS_XPRESS,
                0,
                &mut creation_result,
            );

            if wim_handle.is_null() {
                bail!("WIMCreateFile failed for {}", wim_path.display());
            }

            let result = set_temp(wim_handle, temp_path_w.as_ptr());
            if result == 0 {
                close_handle(wim_handle);
                bail!("WIMSetTemporaryPath failed");
            }

            info!("Capturing from {}...", source_path.display());
            let image_handle = capture_image(wim_handle, source_path_w.as_ptr(), 0);
            if image_handle.is_null() {
                close_handle(wim_handle);
                bail!("WIMCaptureImage failed");
            }

            info!("Capture completed successfully");
            close_handle(image_handle);
            close_handle(wim_handle);
        }

        Ok(())
    }
}

fn to_wide(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}
