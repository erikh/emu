use super::traits::ImageHandler;
use crate::util::path_exists;
use anyhow::{anyhow, Result};
use kdam::{tqdm, BarExt};
use std::{
    fs::remove_file,
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
};

pub const QEMU_IMG_PATH: &str = "qemu-img";
pub const QEMU_IMG_DEFAULT_FORMAT: &str = "qcow2";

pub fn qemu_img_name() -> String {
    format!(
        "qemu-{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        QEMU_IMG_DEFAULT_FORMAT,
    )
}

#[derive(Debug, Clone)]
pub struct QEmuImageHandler {
    format: String,
}

impl Default for QEmuImageHandler {
    fn default() -> Self {
        Self {
            format: QEMU_IMG_DEFAULT_FORMAT.to_string(),
        }
    }
}

impl ImageHandler for QEmuImageHandler {
    fn import(&self, new_file: PathBuf, orig_file: PathBuf, format: String) -> Result<()> {
        Command::new(QEMU_IMG_PATH)
            .args(vec![
                "convert",
                "-f",
                &format,
                "-O",
                &self.format,
                orig_file.to_str().unwrap(),
                new_file.to_str().unwrap(),
            ])
            .status()?;
        Ok(())
    }

    fn create(&self, target: PathBuf, gbs: usize) -> Result<()> {
        let filename = target.join(qemu_img_name());

        if path_exists(filename.clone()) {
            return Err(anyhow!(
                "filename already exists; did you already create this vm?",
            ));
        }

        let status = Command::new(QEMU_IMG_PATH)
            .args(vec![
                "create",
                "-f",
                &self.format,
                filename.to_str().unwrap(),
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
    }

    fn remove(&self, disk: PathBuf) -> Result<()> {
        if !path_exists(disk.clone()) {
            return Err(anyhow!("filename does not exist"));
        }

        Ok(remove_file(disk)?)
    }

    fn clone_image(&self, old: PathBuf, new: PathBuf) -> Result<()> {
        let mut oldf = std::fs::OpenOptions::new();
        oldf.read(true);
        let mut oldf = oldf.open(old)?;
        let mut newf = std::fs::OpenOptions::new();
        newf.write(true);
        newf.create_new(true);
        let mut newf = newf.open(new.clone())?;
        let mut buf = [0_u8; 4096];
        let len = oldf.metadata()?.len();
        let mut pb = tqdm!(total = len.try_into().unwrap());
        pb.set_description(new.file_name().unwrap().to_string_lossy());
        pb.unit_scale = true;
        pb.unit = "B".to_string();
        for _ in 0..len / 4096 {
            oldf.read(&mut buf)?;
            newf.write(&buf)?;
            newf.flush()?;
            pb.update(4096)?;
        }
        Ok(())
    }
}
