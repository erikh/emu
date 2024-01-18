use crate::storage::{DirectoryStorageHandler, StorageHandler};
use anyhow::{anyhow, Result};
use std::process::{Command, Stdio};

pub const QEMU_IMG_PATH: &str = "qemu-img";
pub const QEMU_IMG_NAME: &str = "qemu.qcow2";
pub const QEMU_IMG_DEFAULT_FORMAT: &str = "qcow2";

pub trait Imager {
    fn import(&self, name: &str, orig_file: &str, format: &str) -> Result<()>;
    fn create(&self, name: &str, gbs: u32) -> Result<()>;
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
    fn import(&self, name: &str, orig_file: &str, format: &str) -> Result<()> {
        if self.storage.vm_exists(name) {
            return Err(anyhow!(
                "file already exists, please delete the original vm",
            ));
        }

        match self.storage.vm_root(name) {
            Ok(path) => std::fs::create_dir_all(path)?,
            Err(e) => return Err(e),
        };

        let status = Command::new(QEMU_IMG_PATH)
            .args(vec![
                "convert",
                "-f",
                format,
                "-O",
                "qcow2",
                orig_file,
                &self.storage.vm_path(name, QEMU_IMG_NAME)?,
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

        if !self.storage.vm_path_exists(orig, QEMU_IMG_NAME) {
            return Err(anyhow!("original does not exist"));
        }

        if self.storage.vm_path_exists(new, QEMU_IMG_NAME) {
            return Err(anyhow!("target already exists"));
        }

        match std::fs::copy(
            self.storage.vm_path(orig, QEMU_IMG_NAME).unwrap(),
            self.storage.vm_path(new, QEMU_IMG_NAME).unwrap(),
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn create(&self, name: &str, gbs: u32) -> Result<()> {
        if self.storage.vm_path_exists(name, QEMU_IMG_NAME) {
            return Err(anyhow!(
                "filename already exists; did you already create this vm?",
            ));
        }

        if let Ok(filename) = self.storage.vm_path(name, QEMU_IMG_NAME) {
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
