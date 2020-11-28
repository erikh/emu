use crate::error::Error;
use crate::image::QEMU_IMG_NAME;
use crate::storage::StorageHandler;
//use std::process::{Command, Stdio};

pub enum Architecture {
    X86_64,
}

pub trait EmulatorLauncher {
    fn emulator_path(&self) -> Option<String>;
    fn emulator_args(
        &self,
        vm_name: String,
        sh: Box<dyn StorageHandler>,
    ) -> Result<Vec<String>, Error>;
}

pub struct Configuration {
    pub memory: u32, // megabytes
    pub cpus: u32,
    pub cores: u32,
    pub threads: u32,
}

pub struct QemuLauncher {
    arch: Architecture,
    config: Configuration,
}

impl Default for QemuLauncher {
    fn default() -> Self {
        Self::new(Architecture::X86_64, Configuration::default())
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            memory: 4096,
            cpus: 1,
            cores: 4,
            threads: 4,
        }
    }
}

impl Configuration {
    pub fn valid(&self) -> Result<(), Error> {
        // FIXME fill this in later
        Ok(())
    }
}

impl QemuLauncher {
    pub fn new(arch: Architecture, config: Configuration) -> Self {
        QemuLauncher { arch, config }
    }

    pub fn valid(&self) -> Result<(), Error> {
        self.config.valid()
    }
}

impl EmulatorLauncher for QemuLauncher {
    fn emulator_path(&self) -> Option<String> {
        match self.arch {
            Architecture::X86_64 => return Some(String::from("qemu-system-x86_64")),
        }
    }

    fn emulator_args(
        &self,
        vm_name: String,
        sh: Box<dyn StorageHandler>,
    ) -> Result<Vec<String>, Error> {
        if self.valid().is_ok() {
            if sh.vm_path_exists(&vm_name, QEMU_IMG_NAME) {
                let img_path = sh.vm_path(&vm_name, QEMU_IMG_NAME)?;

                Ok(vec![
                    String::from("-m"),
                    format!("{}", self.config.memory),
                    String::from("-smp"),
                    format!(
                        "cpus={},cores={},threads={}",
                        self.config.cpus, self.config.cores, self.config.threads
                    ),
                    String::from("-drive"),
                    format!("file={},if=virtio,media=disk", img_path),
                ])
            } else {
                Err(Error::new("vm image does not exist"))
            }
        } else {
            Err(Error::new(&format!(
                "vm configuration is invalid: {:?}",
                self.valid(),
            )))
        }
    }
}
