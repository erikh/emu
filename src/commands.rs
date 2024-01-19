use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use clap::{Parser, Subcommand};

use crate::image::{Imager, QEmuImager};
use crate::launcher::emulators::qemu;
use crate::launcher::emulators::qemu::linux;
use crate::launcher::*;
use crate::qmp::{Client, UnixSocket};
use crate::storage::{DirectoryStorageHandler, StorageHandler, SystemdStorage};
use crate::template::Systemd;
use anyhow::{anyhow, Result};

fn list() -> Result<()> {
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

fn supervised() -> Result<()> {
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

fn create(vm_name: &str, size: usize) -> Result<()> {
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

fn delete(vm_name: &str) -> Result<()> {
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

fn supervise(vm_name: &str, cdrom: Option<PathBuf>) -> Result<()> {
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

fn unsupervise(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    let s = SystemdStorage::default();
    s.remove(vm_name)?;

    reload_systemd()
}

fn reload_systemd() -> Result<()> {
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

fn shutdown(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    let controller = qemu::EmulatorController::new(dsh);
    controller.shutdown(vm_name)
}

fn run(
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

fn import(vm_name: &str, from_file: PathBuf, format: &str) -> Result<()> {
    let imager = QEmuImager::default();
    imager.import(vm_name, from_file, format)
}

fn clone(from: &str, to: &str) -> Result<()> {
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

fn show_config(vm_name: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    println!("{}", dsh.config(vm_name)?.to_string());
    Ok(())
}

fn config_set(vm_name: &str, key: &str, value: &str) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut config = dsh.config(vm_name)?;
    config.set_machine_value(key, value)?;
    dsh.write_config(vm_name, config)
}

fn port_map(vm_name: &str, hostport: u16, guestport: u16) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut config = dsh.config(vm_name)?;
    config.map_port(hostport, guestport);
    dsh.write_config(vm_name, config)
}

fn port_unmap(vm_name: &str, hostport: u16) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    let mut config = dsh.config(vm_name)?;
    config.unmap_port(hostport);
    dsh.write_config(vm_name, config)
}

fn qmp(vm_name: &str, command: &str, args: Option<&str>) -> Result<()> {
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

#[derive(Debug, Parser, Clone)]
#[command(author, version, about, long_about=None)]
pub struct Commands {
    #[command(subcommand)]
    command: CommandType,
}

#[derive(Debug, Subcommand, Clone)]
enum CommandType {
    /// Create vm with a sized image
    Create {
        /// Name of VM
        name: String,
        /// Size in GB of VM image
        size: usize,
    },
    /// Delete existing vm
    Delete {
        /// Name of VM
        name: String,
    },
    /// Configure supervision of an already existing VM
    Supervise {
        /// ISO of CD-ROM image -- will be embedded into supervision
        #[arg(short = 'c')]
        cdrom: Option<PathBuf>,
        /// Name of VM
        name: String,
    },
    /// Remove supervision of an already existing vm
    Unsupervise {
        /// Name of VM
        name: String,
    },
    /// Just run a pre-created VM; no systemd involved
    Run {
        /// Run without a video window
        #[arg(short = 'e', long, default_value = "false")]
        headless: bool,
        /// Do not wait for qemu to exit
        #[arg(short, long, default_value = "false")]
        detach: bool,
        /// ISO of CD-ROM image
        #[arg(short, long)]
        cdrom: Option<PathBuf>,
        /// Supply an extra ISO image (useful for windows installations)
        #[arg(long = "extra")]
        extra_disk: Option<PathBuf>,
        /// Name of VM
        name: String,
    },
    /// Gracefully shutdown a pre-created VM
    Shutdown {
        /// Name of VM
        name: String,
    },
    /// Issue QMP commands to the guest
    QMP {
        /// Name of VM
        name: String,
        /// Command to issue
        command: String,
        /// Arguments to send for command, JSON literal in single argument
        arguments: Option<String>,
    },
    /// Yield a list of VMs, one on each line
    List,
    /// Yield a list of supervised VMs, one on each line
    Supervised,
    /// Clone one VM to another
    Clone {
        /// VM to clone from
        from: String,
        /// VM to clone to
        to: String,
    },
    /// Import a VM from a VM image file
    Import {
        /// Format of incoming image
        #[arg(short, long, required = true)]
        format: String,
        /// VM to import to
        name: String,
        /// VM image to import from
        from_file: PathBuf,
    },
    /// Show and manipulate VM configuration
    #[command(subcommand)]
    Config(ConfigSubcommand),
}

#[derive(Debug, Subcommand, Clone)]
enum ConfigSubcommand {
    /// Show the written+inferred configuration for a VM
    Show {
        /// Name of VM
        name: String,
    },
    /// Set a value int the configuration; type-safe
    Set {
        /// Name of VM
        name: String,
        /// Name of key to set
        key: String,
        /// Value of key to set
        value: String,
    },
    /// Adjust port mappings
    #[command(subcommand)]
    Port(ConfigPortSubcommand),
}

#[derive(Debug, Subcommand, Clone)]
enum ConfigPortSubcommand {
    /// Add a port mapping from localhost:<HOSTPORT> -> <GUEST IP>:<GUESTPORT>
    Map {
        /// Name of VM
        name: String,
        /// Port on localhost to map to guest remote IP
        hostport: u16,
        /// Port on guest to expose
        guestport: u16,
    },
    /// Undo a port mapping
    Unmap {
        /// Name of VM
        name: String,
        /// Port on localhost mapped to guest
        hostport: u16,
    },
}

impl Commands {
    pub fn evaluate() -> Result<()> {
        let args = Self::parse();

        match args.command {
            CommandType::Config(sub) => match sub {
                ConfigSubcommand::Set { name, key, value } => config_set(&name, &key, &value),
                ConfigSubcommand::Show { name } => show_config(&name),
                ConfigSubcommand::Port(sub) => match sub {
                    ConfigPortSubcommand::Map {
                        name,
                        hostport,
                        guestport,
                    } => port_map(&name, hostport, guestport),
                    ConfigPortSubcommand::Unmap { name, hostport } => port_unmap(&name, hostport),
                },
            },
            CommandType::Create { name, size } => create(&name, size),
            CommandType::Delete { name } => delete(&name),
            CommandType::Supervise { cdrom, name } => supervise(&name, cdrom),
            CommandType::Unsupervise { name } => unsupervise(&name),
            CommandType::Run {
                headless,
                detach,
                cdrom,
                extra_disk,
                name,
            } => run(&name, cdrom, extra_disk, detach, headless),
            CommandType::List => list(),
            CommandType::Shutdown { name } => shutdown(&name),
            CommandType::QMP {
                name,
                command,
                arguments,
            } => qmp(&name, &command, arguments.as_deref()),
            CommandType::Supervised => supervised(),
            CommandType::Clone { from, to } => clone(&from, &to),
            CommandType::Import {
                format,
                name,
                from_file,
            } => import(&name, from_file, &format),
        }
    }
}
