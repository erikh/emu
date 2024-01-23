use std::path::PathBuf;

pub fn pid_running(pid: u32) -> bool {
    path_exists(PathBuf::from(format!("/proc/{}", pid)))
}

pub fn path_exists(path: PathBuf) -> bool {
    std::fs::metadata(path).is_ok()
}

pub fn valid_filename(name: &str) -> bool {
    !(name.contains("..") || name.contains(std::path::MAIN_SEPARATOR) || name.contains("\x00"))
}
