use crate::error::Error;
use crate::storage::{DirectoryStorageHandler, StorageHandler};
use std::process::{Command, Stdio};

pub const QEMU_IMG_PATH: &str = "qemu-img";
pub const QEMU_IMG_NAME: &str = "qemu.qcow2";
pub const QEMU_IMG_DEFAULT_FORMAT: &str = "qcow2";

pub trait Imager {
    fn import(&self, name: &str, orig_file: &str, format: &str) -> Result<(), Error>;
    fn create(&self, name: &str, gbs: u32) -> Result<(), Error>;
    fn clone(&self, orig: &str, new: &str) -> Result<(), Error>;
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
    fn import(&self, name: &str, orig_file: &str, format: &str) -> Result<(), Error> {
        if self.storage.vm_exists(name) {
            return Err(Error::new(
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
                    return Err(Error::new(&format!(
                        "process exited with code: {}",
                        st.code().expect("unknown")
                    )));
                }
            }
            Err(e) => return Err(Error::from(e)),
        }
    }

    fn clone(&self, orig: &str, new: &str) -> Result<(), Error> {
        if !self.storage.valid_filename(orig) || !self.storage.valid_filename(new) {
            return Err(Error::new("vm names are invalid"));
        }

        if !self.storage.vm_path_exists(orig, QEMU_IMG_NAME) {
            return Err(Error::new("original does not exist"));
        }

        if self.storage.vm_path_exists(new, QEMU_IMG_NAME) {
            return Err(Error::new("target already exists"));
        }

        match std::fs::copy(
            self.storage.vm_path(orig, QEMU_IMG_NAME).unwrap(),
            self.storage.vm_path(new, QEMU_IMG_NAME).unwrap(),
        ) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::from(e)),
        }
    }

    fn create(&self, name: &str, gbs: u32) -> Result<(), Error> {
        if self.storage.vm_path_exists(name, QEMU_IMG_NAME) {
            return Err(Error::new(
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
