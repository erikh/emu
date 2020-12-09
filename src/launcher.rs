use crate::error::Error;
use crate::storage::DirectoryStorageHandler;
use fork::{daemon, Fork};
use std::process::Command;

pub struct RuntimeConfig {
    pub cdrom: Option<String>,
    pub extra_disk: Option<String>,
    pub headless: bool,
    pub dsh: DirectoryStorageHandler,
}

pub trait Emulator {
    fn args(&self, vm_name: &str, rc: &RuntimeConfig) -> Result<Vec<String>, Error>;
    fn bin(&self) -> Result<String, Error>;
}

pub trait EmulatorController {
    fn shutdown(&self, vm_name: &str) -> Result<(), Error>;
}

pub struct Launcher {
    emu: Box<dyn Emulator>,
    rc: RuntimeConfig,
}

impl Launcher {
    pub fn new(emu: Box<dyn Emulator>, rc: RuntimeConfig) -> Self {
        Self { emu, rc }
    }

    pub fn launch(
        &self,
        vm_name: &str,
        detach: bool,
    ) -> Result<Option<std::process::ExitStatus>, Error> {
        let args = self.emu.args(vm_name, &self.rc)?;
        let mut cmd = Command::new(self.emu.bin()?);
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
            Ok(mut child) => {
                if !detach {
                    Ok(Some(child.wait()?))
                } else {
                    Ok(None)
                }
            }
            Err(e) => Err(Error::from(e)),
        }
    }
}

pub mod emulators {
    pub mod qemu {
        use crate::error::Error;
        use crate::launcher;
        use crate::qmp::{Client, UnixSocket};
        use crate::storage::{DirectoryStorageHandler, StorageHandler};
        use std::os::unix::net::UnixStream;

        pub struct EmulatorController {
            dsh: DirectoryStorageHandler,
        }

        impl EmulatorController {
            pub fn new(dsh: DirectoryStorageHandler) -> Self {
                Self { dsh }
            }
        }

        impl launcher::EmulatorController for EmulatorController {
            fn shutdown(&self, name: &str) -> Result<(), Error> {
                let stream = UnixStream::connect(self.dsh.monitor_path(name)?)?;
                let mut us = UnixSocket::new(stream)?;
                us.handshake()?;
                us.send_command("qmp_capabilities", None)?;
                us.send_command("system_powerdown", None)?;
                Ok(())
            }
        }

        pub mod linux {
            use crate::error::Error;
            use crate::image::QEMU_IMG_NAME;
            use crate::launcher;
            use crate::storage::StorageHandler;

            pub struct Emulator {}

            impl Emulator {
                fn hostfwd_rules(
                    &self,
                    vm_name: &str,
                    rc: &launcher::RuntimeConfig,
                ) -> Result<String, Error> {
                    let config = rc.dsh.config(vm_name)?;
                    config.check_ports()?;
                    let mut res = String::new();
                    for (host, guest) in config.ports {
                        res += &format!(",hostfwd=tcp:127.0.0.1:{}-:{}", host, guest);
                    }

                    Ok(res)
                }
            }

            impl launcher::Emulator for Emulator {
                fn args(
                    &self,
                    vm_name: &str,
                    rc: &launcher::RuntimeConfig,
                ) -> Result<Vec<String>, Error> {
                    let config = rc.dsh.config(vm_name)?;
                    if config.valid().is_ok() {
                        if rc.dsh.vm_path_exists(vm_name, QEMU_IMG_NAME) {
                            let img_path = rc.dsh.vm_path(vm_name, QEMU_IMG_NAME)?;
                            let mon = rc.dsh.monitor_path(vm_name)?;

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
                                config.cpu_type,
                                String::from("-smp"),
                                format!("cpus=1,cores={},maxcpus={}", config.cpus, config.cpus),
                                String::from("-drive"),
                                format!(
                                    "driver=qcow2,if={},file={},cache=none,media=disk,index=0",
                                    config.image_interface, img_path
                                ),
                                String::from("-nic"),
                                format!("user{}", self.hostfwd_rules(vm_name, rc)?),
                            ];

                            v.push(String::from("-display"));
                            if !rc.headless {
                                v.push(String::from("gtk"));
                            } else {
                                v.push(String::from("none"));
                            }

                            if let Some(cd) = rc.cdrom.clone() {
                                match std::fs::metadata(&cd) {
                                    Ok(_) => {
                                        v.push(String::from("-cdrom"));
                                        v.push(String::from(cd));
                                    }
                                    Err(e) => {
                                        return Err(Error::new(&format!(
                                            "error locating cdrom file: {}",
                                            e
                                        )))
                                    }
                                }
                            }

                            if let Some(cd) = rc.extra_disk.clone() {
                                match std::fs::metadata(&cd) {
                                    Ok(_) => {
                                        v.push(String::from("-drive"));
                                        v.push(format!("file={},media=cdrom", cd));
                                    }
                                    Err(e) => {
                                        return Err(Error::new(&format!(
                                            "error locating cdrom file: {}",
                                            e
                                        )))
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
                            config.valid(),
                        )))
                    }
                }

                fn bin(&self) -> Result<String, Error> {
                    Ok(String::from("/bin/qemu-system-x86_64"))
                }
            }
        }
    }
}
