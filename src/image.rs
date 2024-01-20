use crate::storage::{DirectoryStorageHandler, StorageHandler};
use anyhow::{anyhow, Result};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

pub const QEMU_IMG_PATH: &str = "qemu-img";
pub const QEMU_IMG_DEFAULT_FORMAT: &str = "qcow2";

fn qemu_img_name() -> String {
    format!(
        "{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        QEMU_IMG_DEFAULT_FORMAT,
    )
}

pub trait Imager {
    fn import(&self, name: &str, orig_file: PathBuf, format: &str) -> Result<()>;
    fn create(&self, name: &str, gbs: usize) -> Result<()>;
    fn clone(&self, orig: &str, new: &str) -> Result<()>;
}

pub struct QEmuImager {
    pub image_format: &'static str,
    storage: DirectoryStorageHandler,
}

impl QEmuImager {
    pub fn new(image_format: &'static str, storage: DirectoryStorageHandler) -> Self {
        QEmuImager {
            image_format,
            storage,
        }
    }
}

impl Default for QEmuImager {
    fn default() -> Self {
        Self::new(QEMU_IMG_DEFAULT_FORMAT, DirectoryStorageHandler::default())
    }
}

impl Imager for QEmuImager {
    fn import(&self, name: &str, orig_file: PathBuf, format: &str) -> Result<()> {
        if self.storage.vm_exists(name) {
            return Err(anyhow!(
                "file already exists, please delete the original vm",
            ));
        }

        match self.storage.vm_root(name) {
            Ok(path) => std::fs::create_dir_all(path)?,
            Err(e) => return Err(e),
        };

        let filename = qemu_img_name();

        let status = Command::new(&filename)
            .args(vec![
                "convert",
                "-f",
                format,
                "-O",
                QEMU_IMG_DEFAULT_FORMAT,
                orig_file.to_str().unwrap(),
                &self.storage.vm_path(name, &filename)?,
            ])
            .status();

        match status {
            Ok(st) => {
                if st.success() {
                    return Ok(());
                } else {
                    return Err(anyhow!(
                        "process exited with code: {}",
                        st.code().expect("unknown")
                    ));
                }
            }
            Err(e) => return Err(anyhow!(e)),
        }
    }

    fn clone(&self, orig: &str, new: &str) -> Result<()> {
        if !self.storage.valid_filename(orig) || !self.storage.valid_filename(new) {
            return Err(anyhow!("vm names are invalid"));
        }

        for disk in self.storage.disk_list(orig)? {
            let filename = disk.file_name().unwrap().to_str().unwrap();
            if !self.storage.vm_path_exists(orig, filename) {
                return Err(anyhow!("original does not exist"));
            }

            if self.storage.vm_path_exists(new, filename) {
                return Err(anyhow!("target already exists"));
            }

            match std::fs::copy(
                self.storage.vm_path(orig, filename).unwrap(),
                self.storage.vm_path(new, filename).unwrap(),
            ) {
                Ok(_) => {}
                Err(e) => return Err(anyhow!(e)),
            }
        }

        Ok(())
    }

    fn create(&self, name: &str, gbs: usize) -> Result<()> {
        let filename = qemu_img_name();

        if self.storage.vm_path_exists(name, &filename) {
            return Err(anyhow!(
                "filename already exists; did you already create this vm?",
            ));
        }

        if let Ok(filename) = self.storage.vm_path(name, &filename) {
            let status = Command::new(QEMU_IMG_PATH)
                .args(vec![
                    "create",
                    "-f",
                    self.image_format,
                    &filename,
                    &format!("{}G", gbs),
                ])
                .stderr(Stdio::null())
                .stdout(Stdio::null())
                .status();

            match status {
                Ok(st) => {
                    if st.success() {
                        return Ok(());
                    } else {
                        return Err(anyhow!(
                            "process exited with code: {}",
                            st.code().expect("unknown")
                        ));
                    }
                }
                Err(e) => return Err(anyhow!(e)),
            }
        } else {
            Err(anyhow!("could not derive path name from vm name"))
        }
    }
}
