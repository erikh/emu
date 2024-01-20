use crate::{image::QEMU_IMG_DEFAULT_FORMAT, launcher, storage::StorageHandler};
use anyhow::{anyhow, Result};
use std::path::PathBuf;

const QEMU_BIN_NAME: &str = "qemu-system-x86_64";

macro_rules! append_vec {
    ( $v:expr, $( $x:expr ),* ) => {
        {
            $(
                $v.push($x.into());
            )*
        }
    };
}

macro_rules! into_vec {
    ( $( $x:expr ),* ) => {
        {
            let mut temp_vec = Vec::new();

            $(
                temp_vec.push($x.into());
            )*

            temp_vec
        }
    };
}

pub struct Emulator {}

impl Emulator {
    fn hostfwd_rules(&self, vm_name: &str, rc: &launcher::RuntimeConfig) -> Result<String> {
        let config = rc.dsh.config(vm_name)?;
        config.check_ports()?;
        let mut res = String::new();
        for (host, guest) in config.ports {
            res += &format!(",hostfwd=tcp:127.0.0.1:{}-:{}", host, guest);
        }

        Ok(res)
    }

    fn cdrom_rules(&self, v: &mut Vec<String>, disk: Option<PathBuf>, index: u8) -> Result<()> {
        if let Some(cd) = disk {
            match std::fs::metadata(&cd) {
                Ok(_) => {
                    append_vec!(
                        v,
                        "-drive",
                        format!("file={},media=cdrom,index={}", cd.display(), index)
                    );
                }
                Err(e) => return Err(anyhow!("error locating cdrom file: {}", e)),
            }
        }
        Ok(())
    }

    fn display_rule(&self, v: &mut Vec<String>, headless: bool) {
        append_vec!(v, "-display");
        if !headless {
            append_vec!(v, "gtk");
        } else {
            append_vec!(v, "none");
        }
    }
}

impl launcher::Emulator for Emulator {
    fn args(&self, vm_name: &str, rc: &launcher::RuntimeConfig) -> Result<Vec<String>> {
        let config = rc.dsh.config(vm_name)?;
        if config.valid().is_ok() {
            let disk_list = rc.dsh.disk_list(vm_name)?;
            let mut disks = Vec::new();
            for (x, disk) in disk_list.iter().enumerate() {
                disks.push("-drive".to_string());
                disks.push(format!(
                    "driver={},if={},file={},cache=writethrough,media=disk,index={}",
                    QEMU_IMG_DEFAULT_FORMAT,
                    config.machine.image_interface,
                    disk.display(),
                    x
                ));
            }

            let mon = rc.dsh.monitor_path(vm_name)?;

            let mut v: Vec<String> = into_vec![
                "-nodefaults",
                "-chardev",
                format!("socket,server=on,wait=off,id=char0,path={}", mon.display()),
                "-mon",
                "chardev=char0,mode=control,pretty=on",
                "-machine",
                "accel=kvm",
                "-vga",
                config.machine.vga,
                "-m",
                format!("{}M", config.machine.memory),
                "-cpu",
                config.machine.cpu_type,
                "-smp",
                format!(
                    "cpus={},cores={},maxcpus={}",
                    config.machine.cpus, config.machine.cpus, config.machine.cpus
                ),
                "-nic",
                format!("user{}", self.hostfwd_rules(vm_name, rc)?)
            ];

            v.append(&mut disks);

            self.display_rule(&mut v, rc.headless);
            self.cdrom_rules(&mut v, rc.cdrom.clone(), (disks.len() + 2) as u8)?;
            self.cdrom_rules(&mut v, rc.extra_disk.clone(), (disks.len() + 3) as u8)?;

            Ok(v)
        } else {
            Err(anyhow!("vm configuration is invalid: {:?}", config.valid(),))
        }
    }

    fn bin(&self) -> Result<String> {
        Ok(QEMU_BIN_NAME.to_string())
    }
}
