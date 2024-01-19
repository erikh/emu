use crate::{
    image::{Imager, QEmuImager},
    launcher::{
        emulators::qemu::{self, linux},
        EmulatorController, Launcher, RuntimeConfig,
    },
    qmp::{Client, UnixSocket},
    storage::{DirectoryStorageHandler, StorageHandler, SystemdStorage},
    template::Systemd,
};
use anyhow::{anyhow, Result};
use std::{
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Command, Stdio},
};

pub(crate) fn list() -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    match dsh.vm_list() {
        Ok(list) => {
            for vm in list {
                println!("{}", vm)
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn supervised() -> Result<()> {
    let s = SystemdStorage::default();
    match s.list() {
        Ok(list) => {
            for vm in list {
                println!("{}", vm)
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

pub(crate) fn create(vm_name: &str, size: usize) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    if dsh.vm_exists(vm_name) {
        return Err(anyhow!("vm already exists"));
    }

    match dsh.vm_root(vm_name) {
        Ok(path) => std::fs::create_dir_all(path)?,
        Err(e) => return Err(e),
    };

    if let Err(e) = dsh.create_monitor(vm_name) {
        return Err(e);
    }

    let imager = QEmuImager::default();
    imager.create(vm_name, size)
}

pub(crate) fn delete(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    if !dsh.vm_exists(vm_name) {
        return Err(anyhow!("vm doesn't exist"));
    }

    match dsh.vm_root(vm_name) {
        Ok(path) => std::fs::remove_dir_all(path)?,
        Err(e) => return Err(e),
    };

    if let Err(_) = unsupervise(vm_name) {
        println!("Could not remove systemd unit; assuming it was never installed")
    }

    Ok(())
}

pub(crate) fn supervise(vm_name: &str, cdrom: Option<PathBuf>) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    if !dsh.vm_exists(vm_name) {
        return Err(anyhow!("vm doesn't exist"));
    }

    let ss = SystemdStorage::default();
    ss.init()?;

    let emu = linux::Emulator {};
    let rc = RuntimeConfig {
        cdrom,
        dsh,
        extra_disk: None,
        headless: true,
    };

    let t = Systemd::new(Box::new(emu), ss);
    if let Err(e) = t.write(vm_name, &rc) {
        return Err(e);
    }

    reload_systemd()
}

pub(crate) fn unsupervise(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    let s = SystemdStorage::default();
    s.remove(vm_name)?;

    reload_systemd()
}

pub(crate) fn reload_systemd() -> Result<()> {
    match Command::new("/bin/systemctl")
        .args(vec!["--user", "daemon-reload"])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
    {
        Ok(es) => {
            if es.success() {
                Ok(())
            } else {
                Err(anyhow!("systemctl exited uncleanly: {}", es))
            }
        }
        Err(e) => Err(anyhow!(e)),
    }
}

pub(crate) fn shutdown(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    let controller = qemu::EmulatorController::new(dsh);
    controller.shutdown(vm_name)
}

pub(crate) fn run(
    vm_name: &str,
    cdrom: Option<PathBuf>,
    extra_disk: Option<PathBuf>,
    detach: bool,
    headless: bool,
) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    let emu = linux::Emulator {};
    let rc = RuntimeConfig {
        cdrom,
        extra_disk,
        headless,
        dsh,
    };

    let launcher = Launcher::new(Box::new(emu), rc);
    let result = launcher.launch(vm_name, detach)?;

    match result {
        Some(es) => {
            if es.success() {
                Ok(())
            } else {
                Err(anyhow!("qemu exited uncleanly: {}", es))
            }
        }
        None => Ok(()),
    }
}

pub(crate) fn import(vm_name: &str, from_file: PathBuf, format: &str) -> Result<()> {
    let imager = QEmuImager::default();
    imager.import(vm_name, from_file, format)
}

pub(crate) fn clone(from: &str, to: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(to) {
        return Err(anyhow!("invalid VM name"));
    }

    if dsh.vm_exists(to) {
        return Err(anyhow!("vm already exists"));
    }

    match dsh.vm_root(to) {
        Ok(path) => std::fs::create_dir_all(path)?,
        Err(e) => return Err(e),
    };

    if let Err(e) = dsh.create_monitor(to) {
        return Err(e);
    }

    let imager = QEmuImager::default();
    imager.clone(from, to)
}

pub(crate) fn show_config(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    println!("{}", dsh.config(vm_name)?.to_string());
    Ok(())
}

pub(crate) fn config_set(vm_name: &str, key: &str, value: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut config = dsh.config(vm_name)?;
    config.set_machine_value(key, value)?;
    dsh.write_config(vm_name, config)
}

pub(crate) fn port_map(vm_name: &str, hostport: u16, guestport: u16) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut config = dsh.config(vm_name)?;
    config.map_port(hostport, guestport);
    dsh.write_config(vm_name, config)
}

pub(crate) fn port_unmap(vm_name: &str, hostport: u16) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut config = dsh.config(vm_name)?;
    config.unmap_port(hostport);
    dsh.write_config(vm_name, config)
}

pub(crate) fn qmp(vm_name: &str, command: &str, args: Option<&str>) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let stream = UnixStream::connect(dsh.monitor_path(vm_name)?)?;
    let mut us = UnixSocket::new(stream)?;
    us.handshake()?;
    us.send_command("qmp_capabilities", None)?;
    let val = match args {
        Some(args) => us.send_command(command, Some(serde_json::from_str(args)?))?,
        None => us.send_command(command, None)?,
    };

    println!("{}", val);
    Ok(())
}
