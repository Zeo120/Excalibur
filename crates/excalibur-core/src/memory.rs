/// Live working-set / RSS memory breakdown for the Excalibur process tree.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MemorySnapshot {
    pub host_mb: f64,
    pub chromium_mb: f64,
    pub total_mb: f64,
    pub child_process_count: usize,
}

/// Samples working-set memory for the host process and all descendant Chromium processes.
pub fn sample_process_tree_memory() -> MemorySnapshot {
    #[cfg(target_os = "windows")]
    {
        sample_windows()
    }
    #[cfg(target_os = "linux")]
    {
        sample_linux()
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        MemorySnapshot::default()
    }
}

#[cfg(target_os = "windows")]
fn sample_windows() -> MemorySnapshot {
    use std::collections::{HashMap, HashSet};
    use windows_sys::Win32::{
        Foundation::{CloseHandle, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{
                GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
                PROCESS_VM_READ,
            },
        },
    };

    let self_pid = unsafe { GetCurrentProcessId() };
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();

    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap != INVALID_HANDLE_VALUE {
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut entry) != 0 {
                loop {
                    children_map
                        .entry(entry.th32ParentProcessID)
                        .or_default()
                        .push(entry.th32ProcessID);
                    if Process32NextW(snap, &mut entry) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
    }

    let mut descendants = HashSet::new();
    let mut stack = vec![self_pid];
    while let Some(pid) = stack.pop() {
        if let Some(kids) = children_map.get(&pid) {
            for &kid in kids {
                if kid != self_pid && descendants.insert(kid) {
                    stack.push(kid);
                }
            }
        }
    }

    let query_mb = |pid: u32| -> f64 {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid);
            if handle.is_null() {
                return 0.0;
            }
            let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
            let size = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let ok = K32GetProcessMemoryInfo(handle, &mut counters, size);
            CloseHandle(handle);
            if ok != 0 {
                counters.WorkingSetSize as f64 / (1024.0 * 1024.0)
            } else {
                0.0
            }
        }
    };

    let host_mb = query_mb(self_pid);
    let mut chromium_mb = 0.0;
    for &pid in &descendants {
        chromium_mb += query_mb(pid);
    }

    MemorySnapshot {
        host_mb,
        chromium_mb,
        total_mb: host_mb + chromium_mb,
        child_process_count: descendants.len(),
    }
}

#[cfg(target_os = "linux")]
fn sample_linux() -> MemorySnapshot {
    use std::collections::{HashMap, HashSet};

    let self_pid = std::process::id();
    let mut children_map: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut rss_kb_map: HashMap<u32, u64> = HashMap::new();

    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
                continue;
            };
            if let Ok(stat) = std::fs::read_to_string(entry.path().join("stat")) {
                if let Some(close_paren) = stat.rfind(')') {
                    let fields: Vec<&str> = stat[close_paren + 2..].split_whitespace().collect();
                    if let Some(ppid) = fields.get(1).and_then(|s| s.parse::<u32>().ok()) {
                        children_map.entry(ppid).or_default().push(pid);
                    }
                }
            }
            if let Ok(statm) = std::fs::read_to_string(entry.path().join("statm")) {
                if let Some(pages) = statm
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse::<u64>().ok())
                {
                    rss_kb_map.insert(pid, pages * 4);
                }
            }
        }
    }

    let mut descendants = HashSet::new();
    let mut stack = vec![self_pid];
    while let Some(pid) = stack.pop() {
        if let Some(kids) = children_map.get(&pid) {
            for &kid in kids {
                if kid != self_pid && descendants.insert(kid) {
                    stack.push(kid);
                }
            }
        }
    }

    let host_mb = rss_kb_map.get(&self_pid).copied().unwrap_or(0) as f64 / 1024.0;
    let chromium_kb: u64 = descendants
        .iter()
        .filter_map(|pid| rss_kb_map.get(pid))
        .sum();
    let chromium_mb = chromium_kb as f64 / 1024.0;

    MemorySnapshot {
        host_mb,
        chromium_mb,
        total_mb: host_mb + chromium_mb,
        child_process_count: descendants.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_process_tree_reports_positive_host_memory() {
        let snap = sample_process_tree_memory();
        assert!(snap.host_mb > 0.0, "expected host_mb > 0, got {snap:?}");
        assert!(snap.total_mb >= snap.host_mb);
    }
}
