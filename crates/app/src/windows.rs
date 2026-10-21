//! Loads the shader compiler DX12 needs from the system directory, so a machine
//! that does not ship it (Windows before 10 1803) refuses DX12 instead of
//! failing before `main` is reached.

use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;

/// The answer found at startup, which is the one the window backend acts on.
static FOUND: OnceLock<bool> = OnceLock::new();

/// Loads the compiler once and records whether the machine has it.
pub fn note_shader_compiler() {
    let found = load_shader_compiler();
    let _ = FOUND.set(found);
}

/// Whether the machine has the compiler DX12 needs, false until one is recorded.
pub fn shader_compiler() -> bool {
    FOUND.get().copied().unwrap_or(false)
}

/// Loads the DLL from the system directory and says whether it was found.
fn load_shader_compiler() -> bool {
    const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x0000_0800;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
        fn LoadLibraryExW(
            file_name: *const u16,
            file: *mut core::ffi::c_void,
            flags: u32,
        ) -> *mut core::ffi::c_void;
    }

    let name: Vec<u16> = std::ffi::OsStr::new("d3dcompiler_47.dll")
        .encode_wide()
        .chain(Some(0))
        .collect();
    // SAFETY: the name is nul-terminated and outlives the call, and the search is
    // pinned to the system directory so no file beside the exe can stand in.
    unsafe {
        if SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32) == 0 {
            return false;
        }
        !LoadLibraryExW(
            name.as_ptr(),
            std::ptr::null_mut(),
            LOAD_LIBRARY_SEARCH_SYSTEM32,
        )
        .is_null()
    }
}
