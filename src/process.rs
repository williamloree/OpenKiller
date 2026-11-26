//! Process name, path and working set of a PID; forced kill with protected-PID and PID-reuse checks.

use crate::model::is_protected;
use std::collections::HashMap;
use windows::Win32::Foundation::{CloseHandle, ERROR_ACCESS_DENIED, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
    QueryFullProcessImageNameW, TerminateProcess,
};
use windows::core::PWSTR;

/// Owned process handle, closed on drop.
pub struct Proc(pub HANDLE);
impl Drop for Proc {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn strip_exe(name: &str) -> &str {
    match name.len().checked_sub(4) {
        Some(i) if name.is_char_boundary(i) && name[i..].eq_ignore_ascii_case(".exe") => &name[..i],
        _ => name,
    }
}

/// Display name for PIDs that have no image.
fn special_name(pid: u32) -> Option<&'static str> {
    match pid {
        0 => Some("Processus inactif du système"),
        4 => Some("System"),
        _ => None,
    }
}

/// PID -> executable name (without .exe) for every process, even inaccessible ones.
pub fn snapshot_names() -> HashMap<u32, String> {
    let mut map = HashMap::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return map;
        };
        let snap = Proc(snap);
        let mut e = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = Process32FirstW(snap.0, &mut e).is_ok();
        while ok {
            let len = e
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]);
            map.insert(e.th32ProcessID, strip_exe(&name).to_string());
            ok = Process32NextW(snap.0, &mut e).is_ok();
        }
    }
    map
}

pub fn name_of(pid: u32, names: &HashMap<u32, String>) -> String {
    special_name(pid)
        .map(str::to_string)
        .or_else(|| names.get(&pid).cloned())
        .unwrap_or_else(|| "Inconnu".into())
}

/// Working set (MB, rounded) and full image path; None when the process cannot be opened.
pub fn details(pid: u32) -> (Option<u64>, Option<String>) {
    if special_name(pid).is_some() {
        return (None, None);
    }
    unsafe {
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return (None, None);
        };
        let h = Proc(h);
        let mut pmc = PROCESS_MEMORY_COUNTERS {
            cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        let mem = GetProcessMemoryInfo(h.0, &mut pmc, pmc.cb)
            .ok()
            .map(|_| ((pmc.WorkingSetSize as f64) / 1_048_576.0).round() as u64);
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let path =
            QueryFullProcessImageNameW(h.0, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
                .ok()
                .map(|_| String::from_utf16_lossy(&buf[..len as usize]));
        (mem, path)
    }
}

#[derive(Debug, PartialEq)]
pub enum KillError {
    Protected,
    /// The PID now belongs to another program (or is gone).
    Changed,
    AccessDenied,
    Other(String),
}

impl KillError {
    pub fn message(&self) -> String {
        match self {
            KillError::Protected => "Processus protégé : OpenKiller refuse de le tuer".into(),
            KillError::Changed => "Le processus a changé, actualise".into(),
            KillError::AccessDenied => "Accès refusé (processus admin ?)".into(),
            KillError::Other(e) => format!("Échec : {e}"),
        }
    }
}

/// Forced kill (like taskkill /F) after checking the PID still runs the expected program.
pub fn kill(pid: u32, expected_name: &str) -> Result<(), KillError> {
    if is_protected(pid, std::process::id()) {
        return Err(KillError::Protected);
    }
    if snapshot_names().get(&pid).map(String::as_str) != Some(expected_name) {
        return Err(KillError::Changed);
    }
    unsafe {
        let h = OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|e| {
            if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                KillError::AccessDenied
            } else {
                KillError::Other(e.message())
            }
        })?;
        let h = Proc(h);
        TerminateProcess(h.0, 1).map_err(|e| {
            if e.code() == ERROR_ACCESS_DENIED.to_hresult() {
                KillError::AccessDenied
            } else {
                KillError::Other(e.message())
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exe_suffix() {
        assert_eq!(strip_exe("python.exe"), "python");
        assert_eq!(strip_exe("Code.EXE"), "Code");
        assert_eq!(strip_exe("System"), "System");
        assert_eq!(strip_exe("exe"), "exe");
        assert_eq!(strip_exe("é.exe"), "é");
    }

    #[test]
    fn own_process_is_visible_and_protected() {
        let names = snapshot_names();
        let me = std::process::id();
        assert!(names.contains_key(&me));
        assert_eq!(name_of(4, &names), "System");
        assert_eq!(name_of(0, &names), "Processus inactif du système");
        let (mem, path) = details(me);
        assert!(mem.is_some() && path.is_some());
        assert_eq!(kill(me, &names[&me]), Err(KillError::Protected));
        assert_eq!(kill(4, "System"), Err(KillError::Protected));
    }
}
