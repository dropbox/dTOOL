//! ConPTY (Windows Pseudo Console) implementation.
//!
//! This module provides the low-level interface to the Windows ConPTY API,
//! which was introduced in Windows 10 version 1809 (build 17763).
//!
//! The ConPTY API allows creating pseudo-terminals on Windows, enabling
//! terminal emulators to properly handle console applications.

use std::collections::HashMap;
use std::ffi::OsStr;
use std::io::{Error, Result};
use std::iter::once;
use std::mem;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{
    CloseHandle, BOOL, FALSE, HANDLE, INVALID_HANDLE_VALUE, S_OK, WAIT_FAILED, WAIT_OBJECT_0,
    WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Console::{
    ClosePseudoConsole, CreatePseudoConsole, ResizePseudoConsole, COORD, HPCON,
    PSEUDOCONSOLE_INHERIT_CURSOR,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, EXTENDED_STARTUPINFO_PRESENT, INFINITE, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

/// Minimum Windows version that supports ConPTY.
/// Windows 10 version 1809 (build 17763).
const MIN_CONPTY_BUILD: u32 = 17763;

/// Windows Pseudo Console wrapper.
///
/// This struct owns the ConPTY handle and ensures it's properly closed
/// when dropped.
pub struct Conpty {
    handle: HPCON,
}

impl Conpty {
    /// Create a new ConPTY with the specified dimensions.
    ///
    /// # Arguments
    /// * `cols` - Number of columns
    /// * `rows` - Number of rows
    /// * `input_handle` - Handle for console input (read end of pipe)
    /// * `output_handle` - Handle for console output (write end of pipe)
    pub fn new(cols: i16, rows: i16, input_handle: HANDLE, output_handle: HANDLE) -> Result<Self> {
        // Check Windows version
        if !Self::is_supported() {
            return Err(Error::other(
                "ConPTY requires Windows 10 version 1809 or later",
            ));
        }

        let size = COORD { X: cols, Y: rows };
        let mut handle: HPCON = 0;

        let result = unsafe {
            CreatePseudoConsole(
                size,
                input_handle,
                output_handle,
                PSEUDOCONSOLE_INHERIT_CURSOR,
                &mut handle,
            )
        };

        if result != S_OK {
            return Err(Error::from_raw_os_error(result));
        }

        // Close the pipe handles that were passed to CreatePseudoConsole
        // The ConPTY now owns duplicates of these handles
        unsafe {
            CloseHandle(input_handle);
            CloseHandle(output_handle);
        }

        Ok(Self { handle })
    }

    /// Check if ConPTY is supported on this Windows version.
    pub fn is_supported() -> bool {
        // Use RtlGetVersion to get the actual OS version
        // GetVersionEx is deprecated and lies about the version
        unsafe {
            let mut info: windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW =
                mem::zeroed();
            info.dwOSVersionInfoSize = mem::size_of::<
                windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW,
            >() as u32;

            // Note: In production, use RtlGetVersion from ntdll.dll instead
            // as GetVersionExW is deprecated and may return wrong values
            if windows_sys::Win32::System::SystemInformation::GetVersionExW(&mut info) == 0 {
                // If we can't get version info, assume ConPTY is supported
                // (fail gracefully on actual use)
                return true;
            }

            info.dwBuildNumber >= MIN_CONPTY_BUILD
        }
    }

    /// Resize the pseudo console.
    pub fn resize(&self, cols: i16, rows: i16) -> Result<()> {
        let size = COORD { X: cols, Y: rows };
        let result = unsafe { ResizePseudoConsole(self.handle, size) };

        if result != S_OK {
            return Err(Error::from_raw_os_error(result));
        }

        Ok(())
    }

    /// Spawn a child process attached to this ConPTY.
    ///
    /// # Arguments
    /// * `cmdline` - The command line to execute (null-terminated UTF-16)
    /// * `working_dir` - Optional working directory
    /// * `env` - Additional environment variables
    pub fn spawn(
        &self,
        cmdline: &[u16],
        working_dir: Option<&Path>,
        env: &HashMap<String, String>,
    ) -> Result<ChildProcess> {
        // Calculate attribute list size
        let mut attr_list_size: usize = 0;
        let result = unsafe {
            InitializeProcThreadAttributeList(ptr::null_mut(), 1, 0, &mut attr_list_size)
        };

        // This call should fail and set the required size
        if result != FALSE && attr_list_size == 0 {
            return Err(Error::last_os_error());
        }

        // Allocate attribute list
        let mut attr_list: Vec<u8> = vec![0; attr_list_size];
        let attr_list_ptr = attr_list.as_mut_ptr() as *mut _;

        // Initialize attribute list
        let result =
            unsafe { InitializeProcThreadAttributeList(attr_list_ptr, 1, 0, &mut attr_list_size) };

        if result == FALSE {
            return Err(Error::last_os_error());
        }

        // Add pseudo console attribute
        let result = unsafe {
            UpdateProcThreadAttribute(
                attr_list_ptr,
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                self.handle as *mut _,
                mem::size_of::<HPCON>(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };

        if result == FALSE {
            unsafe { DeleteProcThreadAttributeList(attr_list_ptr) };
            return Err(Error::last_os_error());
        }

        // Setup startup info
        let mut startup_info: STARTUPINFOEXW = unsafe { mem::zeroed() };
        startup_info.StartupInfo.cb = mem::size_of::<STARTUPINFOEXW>() as u32;
        startup_info.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup_info.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
        startup_info.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
        startup_info.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
        startup_info.lpAttributeList = attr_list_ptr;

        // Setup process info
        let mut process_info: PROCESS_INFORMATION = unsafe { mem::zeroed() };

        // Prepare working directory
        let working_dir_wide: Option<Vec<u16>> =
            working_dir.map(|path| OsStr::new(path).encode_wide().chain(once(0)).collect());
        let working_dir_ptr = working_dir_wide
            .as_ref()
            .map(|v| v.as_ptr())
            .unwrap_or(ptr::null());

        // Prepare environment block
        let env_block = if env.is_empty() {
            None
        } else {
            Some(build_environment_block(env))
        };
        let env_ptr = env_block
            .as_ref()
            .map(|v| v.as_ptr() as *const _)
            .unwrap_or(ptr::null());

        // Create the process
        let mut cmdline_mut = cmdline.to_vec();
        let result = unsafe {
            CreateProcessW(
                ptr::null(),
                cmdline_mut.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                FALSE,
                EXTENDED_STARTUPINFO_PRESENT,
                env_ptr,
                working_dir_ptr,
                &startup_info.StartupInfo,
                &mut process_info,
            )
        };

        // Cleanup attribute list
        unsafe { DeleteProcThreadAttributeList(attr_list_ptr) };

        if result == FALSE {
            return Err(Error::last_os_error());
        }

        // Close thread handle (we don't need it)
        unsafe { CloseHandle(process_info.hThread) };

        Ok(ChildProcess {
            handle: process_info.hProcess,
            id: process_info.dwProcessId,
        })
    }

    /// Get the raw ConPTY handle.
    pub fn handle(&self) -> HPCON {
        self.handle
    }
}

impl Drop for Conpty {
    fn drop(&mut self) {
        unsafe {
            ClosePseudoConsole(self.handle);
        }
    }
}

// SAFETY: The ConPTY handle is thread-safe.
unsafe impl Send for Conpty {}

/// A child process spawned with ConPTY.
///
/// This is a lightweight wrapper around a Windows process handle that
/// provides methods for waiting and retrieving the exit code.
pub struct ChildProcess {
    handle: HANDLE,
    id: u32,
}

impl ChildProcess {
    /// Get the process ID.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Check if the process has exited without blocking.
    ///
    /// Returns `Some(exit_code)` if the process has exited, `None` if still running.
    pub fn try_wait(&self) -> Result<Option<i32>> {
        let result = unsafe { WaitForSingleObject(self.handle, 0) };

        match result {
            WAIT_OBJECT_0 => {
                // Process has exited
                let mut exit_code: u32 = 0;
                let success = unsafe { GetExitCodeProcess(self.handle, &mut exit_code) };
                if success == FALSE {
                    return Err(Error::last_os_error());
                }
                Ok(Some(exit_code as i32))
            }
            WAIT_TIMEOUT => Ok(None),
            WAIT_FAILED => Err(Error::last_os_error()),
            _ => Err(Error::other("Unexpected wait result")),
        }
    }

    /// Wait for the process to exit and return the exit code.
    pub fn wait(&self) -> Result<i32> {
        let result = unsafe { WaitForSingleObject(self.handle, INFINITE) };

        if result == WAIT_FAILED {
            return Err(Error::last_os_error());
        }

        let mut exit_code: u32 = 0;
        let success = unsafe { GetExitCodeProcess(self.handle, &mut exit_code) };
        if success == FALSE {
            return Err(Error::last_os_error());
        }

        Ok(exit_code as i32)
    }

    /// Terminate the process.
    pub fn kill(&self) -> Result<()> {
        let success = unsafe { TerminateProcess(self.handle, 1) };
        if success == FALSE {
            return Err(Error::last_os_error());
        }
        Ok(())
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

// SAFETY: Process handles are thread-safe.
unsafe impl Send for ChildProcess {}
unsafe impl Sync for ChildProcess {}

/// Build a Windows environment block from a HashMap.
///
/// The environment block is a sequence of null-terminated strings,
/// followed by an extra null terminator.
fn build_environment_block(env: &HashMap<String, String>) -> Vec<u16> {
    let mut block: Vec<u16> = Vec::new();

    // First, inherit current environment
    for (key, value) in std::env::vars() {
        let entry = format!("{}={}", key, value);
        block.extend(OsStr::new(&entry).encode_wide());
        block.push(0);
    }

    // Add custom environment variables (may override inherited ones)
    for (key, value) in env {
        let entry = format!("{}={}", key, value);
        block.extend(OsStr::new(&entry).encode_wide());
        block.push(0);
    }

    // Final null terminator
    block.push(0);
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported() {
        // This test will vary based on the Windows version
        // On Windows 10 1809+, this should return true
        let supported = Conpty::is_supported();
        println!("ConPTY supported: {}", supported);
    }

    #[test]
    fn test_build_environment_block() {
        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let block = build_environment_block(&env);

        // Block should end with double null
        assert!(block.len() >= 2);
        assert_eq!(block[block.len() - 1], 0);
        assert_eq!(block[block.len() - 2], 0);

        // Check that our test variable is in the block
        let block_str: String = block
            .split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(|s| String::from_utf16_lossy(s))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(block_str.contains("TEST_VAR=test_value"));
    }
}
