//! RAM gauge data, working-set trimming (`clean-ram`) and its elevated relaunch.

use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HWND, WAIT_OBJECT_0};
use windows::Win32::System::ProcessStatus::{
    EmptyWorkingSet, EnumProcesses, GetPerformanceInfo, PERFORMANCE_INFORMATION,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::{
    GetExitCodeProcess, INFINITE, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SET_QUOTA, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, w};

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct MemInfo {
    pub total: u64,
    pub used: u64,
    /// System file cache, capped at `used` (same estimate as v1).
    pub freeable: u64,
}

pub fn read() -> MemInfo {
    unsafe {
        let mut st = MEMORYSTATUSEX {
            dwLength: size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if GlobalMemoryStatusEx(&mut st).is_err() {
            return MemInfo::default();
        }
        let used = st.ullTotalPhys.saturating_sub(st.ullAvailPhys);
        let mut pi = PERFORMANCE_INFORMATION {
            cb: size_of::<PERFORMANCE_INFORMATION>() as u32,
            ..Default::default()
        };
        let cache = if GetPerformanceInfo(&mut pi, pi.cb).is_ok() {
            (pi.SystemCache as u64) * (pi.PageSize as u64)
        } else {
            0
        };
        MemInfo {
            total: st.ullTotalPhys,
            used,
            freeable: cache.min(used),
        }
    }
}

/// `openkiller.exe clean-ram`: EmptyWorkingSet on every process we can open. Returns how many were trimmed.
pub fn clean_ram() -> usize {
    let mut pids = vec![0u32; 4096];
    let mut needed = 0u32;
    unsafe {
        if EnumProcesses(pids.as_mut_ptr(), (pids.len() * 4) as u32, &mut needed).is_err() {
            return 0;
        }
        pids.truncate(needed as usize / 4);
        pids.iter()
            .filter(|&&pid| pid > 4)
            .filter(|&&pid| {
                match OpenProcess(
                    PROCESS_SET_QUOTA | PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    pid,
                ) {
                    Ok(h) => {
                        let ok = EmptyWorkingSet(h).is_ok();
                        let _ = CloseHandle(h);
                        ok
                    }
                    Err(_) => false,
                }
            })
            .count()
    }
}

#[derive(Debug, PartialEq)]
pub enum CleanError {
    Cancelled,
    Failed(String),
}

/// Relaunches this exe elevated with the single argument `clean-ram` and waits for it.
pub fn run_elevated_clean(owner: HWND) -> Result<(), CleanError> {
    let exe = std::env::current_exe().map_err(|e| CleanError::Failed(e.to_string()))?;
    let file = HSTRING::from(exe.as_os_str());
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        hwnd: owner,
        lpVerb: w!("runas"),
        lpFile: windows::core::PCWSTR(file.as_ptr()),
        lpParameters: w!("clean-ram"),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        if let Err(e) = ShellExecuteExW(&mut info) {
            return Err(if e.code() == ERROR_CANCELLED.to_hresult() {
                CleanError::Cancelled
            } else {
                CleanError::Failed(e.message())
            });
        }
        let mut code = 1u32;
        let waited = WaitForSingleObject(info.hProcess, INFINITE) == WAIT_OBJECT_0;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if waited && code == 0 {
            Ok(())
        } else {
            Err(CleanError::Failed(format!("code {code}")))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_plausible_values() {
        let m = super::read();
        assert!(m.total > 0 && m.used <= m.total && m.freeable <= m.used);
    }
}
