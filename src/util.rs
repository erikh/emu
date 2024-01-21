pub fn pid_running(pid: u32) -> bool {
    path_exists(format!("/proc/{}", pid))
}

pub fn path_exists(path: String) -> bool {
    std::fs::metadata(path).map_or_else(|_| false, |_| true)
}
