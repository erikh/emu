use super::{
    config_storage::XDGConfigStorage,
    image::QEMU_IMG_DEFAULT_FORMAT,
    qmp::messages::GenericReturn,
    traits::{ConfigStorageHandler, Launcher},
    vm::VM,
};
use crate::{qmp::client::Client, util::pid_running};
use anyhow::{anyhow, Result};
use fork::{daemon, Fork};
use std::{
    fs::{read_to_string, remove_file},
    path::PathBuf,
    process::Command,
    process::ExitStatus,
    sync::Arc,
    thread::sleep,
    time::Duration,
};

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

#[derive(Debug, Clone)]
pub struct QEmuLauncher {
    config: Arc<Box<dyn ConfigStorageHandler>>,
}

impl Default for QEmuLauncher {
    fn default() -> Self {
        Self {
            config: Arc::new(Box::new(XDGConfigStorage::default())),
        }
    }
}

impl QEmuLauncher {
    fn hostfwd_rules(&self, vm: &VM) -> Result<String> {
        let config = vm.config();
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

    fn args(&self, vm: &VM) -> Result<Vec<String>> {
        let config = vm.config();
        let disk_list = self.config.disk_list(vm)?;
        let mut disks = Vec::new();
        for (x, disk) in disk_list.iter().enumerate() {
            disks.push("-drive".to_string());
            disks.push(format!(
                "driver={},if={},file={},cache=none,media=disk,index={},snapshot=on",
                QEMU_IMG_DEFAULT_FORMAT,
                config.machine.image_interface,
                disk.display(),
                x
            ));
        }

        let mon = self.config.monitor_path(vm);

        let mut v: Vec<String> = into_vec![
            "-nodefaults",
            "-chardev",
            format!("socket,server=on,wait=off,id=char0,path={}", mon.display()),
            "-snapshot",
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
            format!("user{}", self.hostfwd_rules(vm)?)
        ];

        v.append(&mut disks);

        self.display_rule(&mut v, vm.headless());
        self.cdrom_rules(&mut v, vm.cdrom(), (disks.len() + 2) as u8)?;
        self.cdrom_rules(&mut v, vm.extra_disk(), (disks.len() + 3) as u8)?;

        Ok(v)
    }

    pub fn qmp_command(&self, vm: &VM, mut f: impl FnMut(Client) -> Result<()>) -> Result<()> {
        match Client::new(self.config.monitor_path(vm)) {
            Ok(mut us) => {
                us.handshake()?;
                us.send_command::<GenericReturn>("qmp_capabilities", None)?;
                f(us)?;
            }
            Err(_) => return Err(anyhow!("{} is not running or not monitored", vm)),
        }

        Ok(())
    }
}

impl Launcher for QEmuLauncher {
    fn delete_snapshot(&self, vm: &VM, name: String) -> Result<()> {
        self.qmp_command(vm, |mut c| c.snapshot_delete(&name))?;
        eprintln!("Deleted snapshot '{}'", name);
        Ok(())
    }

    fn snapshot(&self, vm: &VM, name: String) -> Result<()> {
        self.qmp_command(vm, |mut c| c.snapshot_save(&name))?;
        eprintln!("Saved current state to snapshot '{}'", name);
        Ok(())
    }

    fn restore(&self, vm: &VM, name: String) -> Result<()> {
        self.qmp_command(vm, |mut c| c.snapshot_load(&name))?;
        eprintln!("Restored from snapshot '{}'", name);
        Ok(())
    }

    fn shutdown_immediately(&self, vm: &VM) -> Result<()> {
        self.qmp_command(vm, |mut c| {
            c.send_command::<GenericReturn>("system_powerdown", None)?;
            Ok(())
        })
    }

    fn shutdown_wait(&self, vm: &VM) -> Result<ExitStatus> {
        self.shutdown_immediately(vm)?;

        let pidfile = self.config.pidfile(vm);
        let pid = read_to_string(pidfile.clone())?.parse::<u32>()?;
        let mut total = Duration::new(0, 0);
        let amount = Duration::new(0, 50);
        while pid_running(pid) {
            total += amount;
            sleep(amount);
            if amount > Duration::new(10, 0) {
                eprintln!("Waiting for qemu to quit...");
                total = Duration::new(0, 0);
            }
        }
        remove_file(pidfile)?;

        Ok(ExitStatus::default())
    }

    fn launch_attached(&self, vm: &VM) -> Result<ExitStatus> {
        let args = self.args(vm)?;
        let mut cmd = Command::new(QEMU_BIN_NAME);
        Ok(cmd.args(args).spawn()?.wait()?)
    }

    fn launch_detached(&self, vm: &VM) -> Result<()> {
        let args = self.args(vm)?;
        let mut cmd = Command::new(QEMU_BIN_NAME);
        if let Ok(Fork::Child) = daemon(false, false) {
            match cmd.args(args).spawn() {
                Ok(mut child) => {
                    std::fs::write(
                        &self.config.pidfile(vm),
                        format!("{}", child.id()).as_bytes(),
                    )?;
                    child.wait()?;
                    Ok(())
                }
                Err(e) => Err(anyhow!(e)),
            }
        } else {
            return Err(anyhow!("could not fork"));
        }
    }
}
