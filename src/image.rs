use crate::error::Error;
use crate::storage::{DirectoryStorageHandler, StorageHandler};
use std::process::{Command, Stdio};

pub const QEMU_IMG_PATH: &str = "qemu-img";
pub const QEMU_IMG_NAME: &str = "qemu.qcow2";
pub const QEMU_IMG_DEFAULT_FORMAT: &str = "qcow2";

pub trait Imager {
    fn create(&self, sh: DirectoryStorageHandler, name: &str, gbs: u32) -> Result<(), Error>;
    fn clone(&self, sh: DirectoryStorageHandler, orig: &str, new: &str) -> Result<(), Error>;
}

pub struct QEmuImager {
    pub image_format: &'static str,
}

impl QEmuImager {
    pub fn new(image_format: &'static str) -> Self {
        QEmuImager { image_format }
    }
}

impl Default for QEmuImager {
    fn default() -> Self {
        Self::new(QEMU_IMG_DEFAULT_FORMAT)
    }
}

impl Imager for QEmuImager {
    fn clone(&self, sh: DirectoryStorageHandler, orig: &str, new: &str) -> Result<(), Error> {
        if !sh.valid_filename(orig) || !sh.valid_filename(new) {
            return Err(Error::new("vm names are invalid"));
        }

        if !sh.vm_path_exists(orig, QEMU_IMG_NAME) {
            return Err(Error::new("original does not exist"));
        }

        if sh.vm_path_exists(new, QEMU_IMG_NAME) {
            return Err(Error::new("target already exists"));
        }

        match std::fs::copy(
            sh.vm_path(orig, QEMU_IMG_NAME).unwrap(),
            sh.vm_path(new, QEMU_IMG_NAME).unwrap(),
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
    }

    fn create(&self, sh: DirectoryStorageHandler, name: &str, gbs: u32) -> Result<(), Error> {
        if sh.vm_path_exists(name, QEMU_IMG_NAME) {
            return Err(Error::new(
                "filename already exists; did you already create this vm?",
            ));
        }

        if let Ok(filename) = sh.vm_path(name, QEMU_IMG_NAME) {
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
                        return Err(Error::new(&format!(
                            "process exited with code: {}",
                            st.code().expect("unknown")
                        )));
                    }
                }
                Err(e) => return Err(Error::from(e)),
            }
        } else {
            Err(Error::new("could not derive path name from vm name"))
        }
    }
}
