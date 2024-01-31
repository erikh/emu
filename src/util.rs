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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    fn test_pid_running() -> Result<()> {
        assert!(pid_running(1));
        assert!(!pid_running(8675309));
        Ok(())
    }

    #[test]
    fn test_path_exists() -> Result<()> {
        assert!(path_exists(PathBuf::from("/")));
        assert!(!path_exists(PathBuf::from("/nonexistent")));
        Ok(())
    }

    #[test]
    fn test_valid_filename() -> Result<()> {
        for item in vec!["../one", "/vmlinuz", "im\x00smrt", "one/../two"] {
            assert!(!valid_filename(item));
        }

        for item in vec!["qemu-8675309.qcow2", "config"] {
            assert!(valid_filename(item));
        }

        Ok(())
    }
}
