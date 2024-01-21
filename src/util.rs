pub fn pid_running(pid: u32) -> bool {
    std::fs::metadata(&format!("/proc/{}", pid)).map_or_else(|_| false, |_| true)
}
