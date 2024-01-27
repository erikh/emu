use super::{
    config_storage::XDGConfigStorage,
    image::{QEmuImageHandler, QEMU_IMG_DEFAULT_FORMAT},
    launcher::QEmuLauncher,
    supervisor::SystemdSupervisor,
    traits::{ConfigStorageHandler, ImageHandler, Launcher, SupervisorHandler},
    vm::VM,
};
use crate::{qmp::client::Client, util::valid_filename};
use anyhow::{anyhow, Result};
use std::{path::PathBuf, process::Command, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    sync::Mutex,
};

#[derive(Debug, Clone)]
pub struct CommandHandler {
    launcher: Arc<Box<dyn Launcher>>,
    config: Arc<Box<dyn ConfigStorageHandler>>,
    image: Arc<Box<dyn ImageHandler>>,
}

impl Default for CommandHandler {
    fn default() -> Self {
        Self {
            launcher: Arc::new(Box::new(QEmuLauncher::default())),
            config: Arc::new(Box::new(XDGConfigStorage::default())),
            image: Arc::new(Box::new(QEmuImageHandler::default())),
        }
    }
}

impl CommandHandler {
    pub fn save_state(&self, vm: &VM) -> Result<()> {
        self.launcher.save_state(vm)
    }

    pub fn load_state(&self, vm: &VM) -> Result<()> {
        self.launcher.load_state(vm)
    }

    pub fn clear_state(&self, vm: &VM) -> Result<()> {
        self.launcher.clear_state(vm)
    }

    pub fn list(&self, running: bool) -> Result<()> {
        if running {
            let mut v = Vec::new();

            for item in self.config.vm_list()? {
                if item.supervisor().is_active(&item).unwrap_or_default() {
                    v.push(item)
                }
            }

            Ok(v)
        } else {
            self.config.vm_list()
        }?
        .iter()
        .for_each(|vm| {
            let supervisor = vm.supervisor();

            let (status, is_running) = if supervisor.supervised() {
                match supervisor.is_active(vm) {
                    Ok(res) => {
                        if res {
                            ("supervised: running".to_string(), true)
                        } else {
                            ("supervised: not running".to_string(), false)
                        }
                    }
                    Err(e) => (
                        format!("supervised: could not determine status: {}", e.to_string()),
                        false,
                    ),
                }
            } else if supervisor.is_active(vm).unwrap_or_default() {
                (format!("pid: {}", supervisor.pidof(vm).unwrap()), true)
            } else {
                ("stopped".to_string(), false)
            };

            if running && is_running || !running {
                println!(
                    "{} ({}) (size: {:.2})",
                    vm.name(),
                    status,
                    byte_unit::Byte::from_u128(self.config.size(vm).unwrap() as u128)
                        .unwrap()
                        .get_appropriate_unit(byte_unit::UnitType::Decimal)
                );
            }
        });

        Ok(())
    }

    pub fn rename(&self, old: &VM, new: &VM) -> Result<()> {
        match self.config.rename(old, new) {
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

    pub fn supervised(&self) -> Result<()> {
        for item in self.config.vm_list()? {
            if item.supervisor().supervised() {
                let status = if item.supervisor().is_active(&item).unwrap_or_default() {
                    "running"
                } else {
                    "not running"
                };
                println!("{}: {}", item, status)
            }
        }

        Ok(())
    }

    pub async fn nc(&self, vm: &VM, port: u16) -> Result<()> {
        let config = vm.config();

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

    pub fn ssh(&self, vm: &VM, args: Option<Vec<String>>) -> Result<()> {
        let mut cmd = Command::new("ssh");
        let port = vm.config().machine.ssh_port.to_string();
        let mut all_args = vec!["-p", &port, "localhost"];

        let args = args.unwrap_or_default();
        all_args.append(&mut args.iter().map(String::as_str).collect());

        if cmd.args(all_args).spawn()?.wait()?.success() {
            Ok(())
        } else {
            Err(anyhow!("SSH failed with non-zero status"))
        }
    }

    pub fn create(&self, vm: &VM, size: usize, append: bool) -> Result<()> {
        if !append {
            if self.config.vm_exists(vm) {
                return Err(anyhow!("vm already exists"));
            }

            if !valid_filename(&vm.name()) {
                return Err(anyhow!("filename contains invalid characters"));
            }

            std::fs::create_dir_all(self.config.vm_root(vm))?;
        }

        self.image.create(self.config.vm_root(vm), size)
    }

    pub fn list_disks(&self, vm: &VM) -> Result<()> {
        if !self.config.vm_exists(vm) {
            return Err(anyhow!("vm doesn't exist"));
        }

        for disk in self.config.disk_list(vm)? {
            let disk = disk
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .trim_start_matches("qemu-")
                .trim_end_matches(QEMU_IMG_DEFAULT_FORMAT)
                .trim_end_matches(".");
            println!("{}", disk);
        }

        Ok(())
    }

    pub fn delete(&self, vm: &VM, disk: Option<String>) -> Result<()> {
        if !self.config.vm_exists(vm) {
            return Err(anyhow!("vm doesn't exist"));
        }

        let root = self.config.vm_root(vm);
        if let Some(disk) = disk {
            std::fs::remove_file(root.join(format!("qemu-{}.{}", disk, QEMU_IMG_DEFAULT_FORMAT)))?;
        } else {
            std::fs::remove_dir_all(root)?;
            if let Err(_) = self.unsupervise(vm) {
                println!("Could not remove systemd unit; assuming it was never installed")
            }
        }

        Ok(())
    }

    pub fn supervise(&self, vm: &VM) -> Result<()> {
        if !self.config.vm_exists(vm) {
            return Err(anyhow!("vm doesn't exist"));
        }

        let supervisor = SystemdSupervisor::default();

        supervisor.storage().create(vm)?;
        supervisor.reload()
    }

    pub fn unsupervise(&self, vm: &VM) -> Result<()> {
        let supervisor = vm.supervisor();
        supervisor.storage().remove(vm)?;
        supervisor.reload()
    }

    pub fn is_active(&self, vm: &VM) -> Result<()> {
        if vm.supervisor().is_active(&vm).unwrap_or_default() {
            println!("{} is active", vm);
        } else {
            println!("{} is not active", vm);
        }

        Ok(())
    }

    pub fn shutdown(&self, vm: &VM, nowait: bool) -> Result<()> {
        if nowait {
            self.launcher.shutdown_immediately(vm)
        } else {
            if let Ok(status) = self.launcher.shutdown_wait(vm) {
                println!(
                    "qemu exited with {} status",
                    status.code().unwrap_or_default()
                );
            }

            Ok(())
        }
    }

    pub fn run(&self, vm: &VM, detach: bool) -> Result<()> {
        if detach {
            self.launcher.launch_detached(vm)
        } else {
            match self.launcher.launch_attached(vm) {
                Ok(status) => {
                    if status.success() {
                        Ok(())
                    } else {
                        Err(anyhow!("qemu exited uncleanly: {}", status))
                    }
                }
                Err(e) => Err(e),
            }
        }
    }

    pub fn import(&self, vm: &VM, from_file: PathBuf, format: String) -> Result<()> {
        if !self.config.vm_exists(vm) {
            std::fs::create_dir_all(self.config.vm_root(vm))?;
        }

        self.image.import(
            self.config.vm_root(vm).join(from_file.file_name().unwrap()),
            from_file,
            format,
        )
    }

    pub fn clone(&self, from: &VM, to: &VM) -> Result<()> {
        if self.config.vm_exists(to) {
            return Err(anyhow!("vm already exists"));
        }

        std::fs::create_dir_all(self.config.vm_root(to))?;
        for img in self.config.disk_list(from)? {
            self.image.clone_image(
                img.clone(),
                self.config.vm_root(to).join(img.file_name().unwrap()),
            )?;
        }

        Ok(())
    }

    pub fn config_copy(&self, from: &VM, to: &VM) -> Result<()> {
        if !self.config.vm_exists(from) {
            println!("VM {} does not exist", from);
            return Ok(());
        }

        let mut to = to.clone();

        to.set_config(from.config());
        self.config.write_config(to)
    }

    pub fn show_config(&self, vm: &VM) -> Result<()> {
        if !self.config.vm_exists(vm) {
            println!("VM {} does not exist", vm);
            return Ok(());
        }
        println!("{}", vm.config().to_string());
        Ok(())
    }

    pub fn config_set(&self, vm: &VM, key: String, value: String) -> Result<()> {
        let mut vm = vm.clone();
        let mut config = vm.config();
        config.set_machine_value(&key, &value)?;
        vm.set_config(config);
        match self.config.write_config(vm.clone()) {
            Ok(_) => {}
            Err(_) => {
                println!("VM {} does not exist", vm);
            }
        }

        Ok(())
    }

    pub fn port_map(&self, vm: &VM, hostport: u16, guestport: u16) -> Result<()> {
        let mut vm = vm.clone();
        let mut config = vm.config();
        config.map_port(hostport, guestport);
        vm.set_config(config);
        self.config.write_config(vm)
    }

    pub fn port_unmap(&self, vm: &VM, hostport: u16) -> Result<()> {
        let mut vm = vm.clone();
        let mut config = vm.config();
        config.unmap_port(hostport);
        vm.set_config(config);
        self.config.write_config(vm)
    }

    pub fn qmp(&self, vm: &VM, command: &str, args: Option<&str>) -> Result<()> {
        let mut us = Client::new(self.config.monitor_path(vm))?;
        us.handshake()?;
        // this command hangs if the type isn't provided (for some reason)
        us.send_command::<serde_json::Value>("qmp_capabilities", None)?;
        let val = match args {
            Some(args) => {
                us.send_command::<serde_json::Value>(command, Some(serde_json::from_str(args)?))?
            }
            None => us.send_command::<serde_json::Value>(command, None)?,
        };

        println!("{}", serde_json::to_string_pretty(&val)?);
        Ok(())
    }
}
