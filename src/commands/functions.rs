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
    sync::Arc,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    sync::Mutex,
};

pub(crate) fn list() -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    dsh.vm_list().map(|list| {
        list.iter().for_each(|vm| {
            let supervised = systemd_supervised(&vm.name()).map_or_else(|_| false, |_| true);

            let status = if supervised {
                match systemd_active(&vm.name()) {
                    Ok(_) => "supervised: running",
                    Err(_) => "supervised: not running",
                }
                .to_string()
            } else if qemu_active(&vm.name()) {
                format!("pid: {}", qemu_pid(&vm.name()).unwrap()).to_string()
            } else {
                "unsupervised".to_string()
            };

            println!(
                "{} ({}) (size: {:.2})",
                vm.name(),
                status,
                byte_unit::Byte::from_u128(vm.size().unwrap() as u128)
                    .unwrap()
                    .get_appropriate_unit(byte_unit::UnitType::Decimal)
            );
        });
    })
}

pub(crate) fn rename(old: &str, new: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    match dsh.rename(old, new) {
        Ok(_) => {
            println!("Renamed {} to {}", old, new);
        }
        Err(_) => {
            println!(
                "Could not rename {}. Does it exist, or does {} already exist?",
                old, new
            );
        }
    }

    Ok(())
}

pub(crate) fn supervised() -> Result<()> {
    let s = SystemdStorage::default();
    s.list().map(|list| {
        list.iter().for_each(|vm| {
            let status = match systemd_active(&vm) {
                Ok(_) => "running",
                Err(_) => "not running",
            };
            println!("{}: {}", vm, status)
        });
    })
}

pub(crate) async fn nc(vm_name: &str, port: u16) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let config = dsh.config(vm_name)?;
    if config.ports.contains_key(&port.to_string()) {
        let (s, mut r) = tokio::sync::mpsc::unbounded_channel();
        let (close_s, close_r) = tokio::sync::mpsc::unbounded_channel();
        let close_r = Arc::new(Mutex::new(close_r));

        let close_s2 = close_s.clone();
        let close_r2 = close_r.clone();

        tokio::spawn(async move {
            let mut buf = [0_u8; 4096];
            while let Ok(size) = tokio::io::stdin().read(&mut buf).await {
                if size > 0 {
                    s.send(buf[..size].to_vec()).unwrap();
                } else {
                    break;
                }

                if close_r2.lock().await.try_recv().is_ok() {
                    return;
                }
            }
            close_s2.send(()).unwrap();
        });

        let mut stream = tokio::net::TcpStream::connect(
            format!("127.0.0.1:{}", port).parse::<std::net::SocketAddr>()?,
        )
        .await?;

        let mut buf = [0_u8; 4096];
        let interest = Interest::WRITABLE.clone();
        let interest = interest.add(Interest::READABLE);
        let interest = interest.add(Interest::ERROR);

        loop {
            let state = stream.ready(interest).await?;

            if state.is_error() {
                close_s.send(())?;
                break;
            }

            if state.is_readable() {
                while let Ok(size) = stream.try_read(&mut buf) {
                    if size > 0 {
                        tokio::io::stdout().write(&buf[..size]).await?;
                    } else {
                        break;
                    }
                }
            }

            if state.is_writable() {
                while let Ok(buf) = r.try_recv() {
                    stream.write(&buf).await?;
                }
            }

            if close_r.lock().await.try_recv().is_ok() {
                break;
            }
        }
    }

    Ok(())
}

pub(crate) fn ssh(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut cmd = Command::new("ssh");
    if cmd
        .args(vec![
            "-p",
            &dsh.config(vm_name)?.machine.ssh_port.to_string(),
            "localhost",
        ])
        .spawn()?
        .wait()?
        .success()
    {
        Ok(())
    } else {
        Err(anyhow!("SSH failed with non-zero status"))
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

pub(crate) fn is_active(vm_name: &str) -> Result<()> {
    match systemd_supervised(vm_name) {
        Ok(_) => match systemd_active(vm_name) {
            Ok(_) => println!("{} is active", vm_name),
            Err(_) => println!("{} is not active", vm_name),
        },
        Err(_) => {
            let running = qemu_active(vm_name);
            println!(
                "{}{}",
                vm_name,
                if running {
                    " is active (pid: ".to_owned() + &qemu_pid(vm_name)?.to_string() + ")"
                } else {
                    " is not active".to_string()
                }
            );
        }
    }

    Ok(())
}

pub(crate) fn qemu_active(vm_name: &str) -> bool {
    qemu_pid(vm_name).map_or_else(
        |_| false,
        |pid| std::fs::metadata(&format!("/proc/{}", pid)).map_or_else(|_| false, |_| true),
    )
}

pub(crate) fn qemu_pid(vm_name: &str) -> Result<u32> {
    let dsh = DirectoryStorageHandler::default();
    Ok(std::fs::read_to_string(dsh.vm_root(vm_name)?.join("pid"))?.parse::<u32>()?)
}

pub(crate) fn systemd_supervised(vm_name: &str) -> Result<()> {
    let s = SystemdStorage::default();
    s.supervised(vm_name)
}

pub(crate) fn systemd_active(vm_name: &str) -> Result<()> {
    let s = SystemdStorage::default();
    systemd(vec!["is-active", &s.service_name(vm_name)?, "-q"])
}

pub(crate) fn systemd(mut command: Vec<&str>) -> Result<()> {
    command.insert(0, "--user");
    match Command::new("/bin/systemctl")
        .args(command)
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

pub(crate) fn reload_systemd() -> Result<()> {
    systemd(vec!["daemon-reload"])
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
