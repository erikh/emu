use crate::image::QEMU_IMG_NAME;
use crate::launcher;
use crate::storage::StorageHandler;
use anyhow::{anyhow, Result};
use std::path::PathBuf;

macro_rules! append_vec {
    ( $v:expr, $( $x:expr ),* ) => {
        {
            $(
                $v.push($x.into());
            )*
        }
    };
}

macro_rules! string_vec {
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
                    append_vec!(v, "-drive");
                    append_vec!(
                        v,
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
            if rc.dsh.vm_path_exists(vm_name, QEMU_IMG_NAME) {
                let img_path = rc.dsh.vm_path(vm_name, QEMU_IMG_NAME)?;
                let mon = rc.dsh.monitor_path(vm_name)?;

                let mut v: Vec<String> = string_vec![
                    "-nodefaults",
                    "-chardev",
                    format!("socket,server=on,wait=off,id=char0,path={}", mon),
                    "-mon",
                    "chardev=char0,mode=control,pretty=on",
                    "-machine",
                    "accel=kvm",
                    "-vga",
                    config.vga,
                    "-m",
                    format!("{}M", config.memory),
                    "-cpu",
                    config.cpu_type,
                    "-smp",
                    format!(
                        "cpus={},cores={},maxcpus={}",
                        config.cpus, config.cpus, config.cpus
                    ),
                    "-drive",
                    format!(
                        "driver=qcow2,if={},file={},cache=none,media=disk,index=0",
                        config.image_interface, img_path
                    ),
                    "-nic",
                    format!("user{}", self.hostfwd_rules(vm_name, rc)?)
                ];

                self.display_rule(&mut v, rc.headless);
                self.cdrom_rules(&mut v, rc.cdrom.clone(), 2)?;
                self.cdrom_rules(&mut v, rc.extra_disk.clone(), 3)?;

                Ok(v)
            } else {
                Err(anyhow!("vm image does not exist"))
            }
        } else {
            Err(anyhow!("vm configuration is invalid: {:?}", config.valid(),))
        }
    }

    fn bin(&self) -> Result<String> {
        Ok(String::from("/bin/qemu-system-x86_64"))
    }
}
