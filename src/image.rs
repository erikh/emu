use crate::error::Error;
use crate::storage::{DirectoryStorageHandler, StorageHandler};
use std::process::{Command, Stdio};

pub const QEMU_IMG_PATH: &str = "qemu-img";
pub const QEMU_IMG_NAME: &str = "qemu.raw";
pub const QEMU_IMG_DEFAULT_FORMAT: &str = "raw";

pub trait Imager {
    fn create(&self, sh: DirectoryStorageHandler, name: &str, gbs: u32) -> Result<(), Error>;
    //fn clone(sh: StorageHandler, orig: String, new: String) -> Result<(), Error>;
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
    fn create(&self, sh: DirectoryStorageHandler, name: &str, gbs: u32) -> Result<(), Error> {
        let exists = sh.vm_path_exists(name, QEMU_IMG_NAME);
        if exists {
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
