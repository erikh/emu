use crate::error::Error;
use crate::image::QEMU_IMG_NAME;
use crate::qmp::{Client, UnixSocket};
use crate::storage::{DirectoryStorageHandler, StorageHandler};
use fork::{daemon, Fork};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command};

pub enum Architecture {
    X86_64,
}

pub trait EmulatorLauncher {
    fn launch_vm(
        &self,
        name: &str,
        cdrom: Option<&str>,
        extra_disk: Option<&str>,
        detach: bool,
        headless: bool,
        sh: DirectoryStorageHandler,
    ) -> Result<Child, Error>;

    fn shutdown_vm(&self, name: &str, sh: DirectoryStorageHandler) -> Result<(), Error>;

    fn emulator_path(&self) -> String;
    fn emulator_cpu(&self) -> String;

    fn emulator_args(
        &self,
        vm_name: &str,
        cdrom: Option<&str>,
        extra_disk: Option<&str>,
        headless: bool,
        sh: DirectoryStorageHandler,
    ) -> Result<Vec<String>, Error>;
}

pub struct QemuLauncher {
    arch: Architecture,
    dsh: DirectoryStorageHandler,
}

impl Default for QemuLauncher {
    fn default() -> Self {
        Self::new(Architecture::X86_64, DirectoryStorageHandler::default())
    }
}

impl QemuLauncher {
    pub fn new(arch: Architecture, dsh: DirectoryStorageHandler) -> Self {
        QemuLauncher { arch, dsh }
    }

    pub fn valid(&self, vm_name: &str) -> Result<(), Error> {
        self.dsh.config(vm_name)?.valid()
    }

    fn hostfwd_rules(&self, vm_name: &str) -> Result<String, Error> {
        let config = self.dsh.config(vm_name)?;
        config.check_ports()?;
        let mut res = String::new();
        for (host, guest) in config.ports {
            res += &format!(",hostfwd=tcp:127.0.0.1:{}-:{}", host, guest);
        }

        Ok(res)
    }
}

impl EmulatorLauncher for QemuLauncher {
    fn shutdown_vm(&self, name: &str, sh: DirectoryStorageHandler) -> Result<(), Error> {
        let stream = UnixStream::connect(sh.monitor_path(name)?)?;
        let mut us = UnixSocket::new(stream)?;
        us.handshake()?;
        us.send_command("qmp_capabilities", None)?;
        us.send_command("system_powerdown", None)?;
        Ok(())
    }

    fn launch_vm(
        &self,
        name: &str,
        cdrom: Option<&str>,
        extra_disk: Option<&str>,
        detach: bool,
        headless: bool,
        sh: DirectoryStorageHandler,
    ) -> Result<Child, Error> {
        match self.emulator_args(name, cdrom, extra_disk, headless, sh) {
            Ok(args) => {
                let mut cmd = Command::new(self.emulator_path());

                let spawnres = if detach {
                    if let Ok(Fork::Child) = daemon(false, false) {
                        cmd.args(args).spawn()
                    } else {
                        Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "could not fork",
                        ))
                    }
                } else {
                    cmd.args(args).spawn()
                };

                match spawnres {
                    Ok(child) => Ok(child),
                    Err(e) => Err(Error::from(e)),
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }

    fn emulator_path(&self) -> String {
        match self.arch {
            Architecture::X86_64 => return String::from("/bin/qemu-system-x86_64"),
        }
    }

    fn emulator_cpu(&self) -> String {
        match self.arch {
            Architecture::X86_64 => return String::from("host"),
        }
    }

    fn emulator_args(
        &self,
        vm_name: &str,
        cdrom: Option<&str>,
        extra_disk: Option<&str>,
        headless: bool,
        sh: DirectoryStorageHandler,
    ) -> Result<Vec<String>, Error> {
        if self.valid(vm_name).is_ok() {
            if sh.vm_path_exists(vm_name, QEMU_IMG_NAME) {
                let img_path = sh.vm_path(vm_name, QEMU_IMG_NAME)?;
                let mon = sh.monitor_path(vm_name).unwrap();
                let config = self.dsh.config(vm_name)?;

                let mut v = vec![
                    String::from("-nodefaults"),
                    String::from("-chardev"),
                    format!("socket,server,nowait,id=char0,path={}", mon),
                    String::from("-mon"),
                    String::from("chardev=char0,mode=control,pretty=on"),
                    String::from("-machine"),
                    String::from("accel=kvm"),
                    String::from("-vga"),
                    config.vga,
                    String::from("-m"),
                    format!("{}M", config.memory),
                    String::from("-cpu"),
                    self.emulator_cpu(),
                    String::from("-smp"),
                    format!("cpus=1,cores={},maxcpus={}", config.cpus, config.cpus,),
                    String::from("-drive"),
                    format!(
                        "driver=qcow2,if={},file={},cache=none,media=disk",
                        config.image_interface, img_path
                    ),
                    String::from("-nic"),
                    format!("user{}", self.hostfwd_rules(vm_name)?),
                ];

                v.push(String::from("-display"));
                if !headless {
                    v.push(String::from("gtk"));
                } else {
                    v.push(String::from("none"));
                }

                if let Some(cd) = cdrom {
                    match std::fs::metadata(cd) {
                        Ok(_) => {
                            v.push(String::from("-cdrom"));
                            v.push(String::from(cd));
                        }
                        Err(e) => {
                            return Err(Error::new(&format!("error locating cdrom file: {}", e)))
                        }
                    }
                }

                if let Some(cd) = extra_disk {
                    match std::fs::metadata(cd) {
                        Ok(_) => {
                            v.push(String::from("-drive"));
                            v.push(format!("file={},media=cdrom", cd));
                        }
                        Err(e) => {
                            return Err(Error::new(&format!("error locating cdrom file: {}", e)))
                        }
                    }
                }

                Ok(v)
            } else {
                Err(Error::new("vm image does not exist"))
            }
        } else {
            Err(Error::new(&format!(
                "vm configuration is invalid: {:?}",
                self.valid(vm_name),
            )))
        }
    }
}
