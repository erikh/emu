use std::io::Write;
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};

use clap::ArgMatches;

use crate::image::{Imager, QEmuImager};
use crate::launcher::emulators::qemu;
use crate::launcher::emulators::qemu::linux;
use crate::launcher::*;
use crate::network::{BridgeManager, NetworkManager};
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

fn create(vm_name: &str, size: u32) -> Result<()> {
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

fn supervise(vm_name: &str, cdrom: Option<&str>) -> Result<()> {
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
        cdrom: match cdrom {
            Some(x) => Some(String::from(x)),
            None => None,
        },
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
    cdrom: Option<&str>,
    extra_disk: Option<&str>,
    detach: bool,
    headless: bool,
) -> Result<()> {
    let dsh = DirectoryStorageHandler::default();
    if !dsh.valid_filename(vm_name) {
        return Err(anyhow!("invalid VM name"));
    }

    let emu = linux::Emulator {};
    let rc = RuntimeConfig {
        cdrom: match cdrom {
            Some(x) => Some(String::from(x)),
            None => None,
        },
        extra_disk: match extra_disk {
            Some(x) => Some(String::from(x)),
            None => None,
        },
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

fn import(vm_name: &str, from_file: &str, format: &str) -> Result<()> {
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

async fn network_test() -> Result<()> {
    let bm = BridgeManager {};
    let network = bm.create_network("test").await?;
    let interface = bm.create_interface(&network, 1).await?;
    bm.bind(&network, &interface).await?;
    println!("{}", bm.exists_network(&network).await?);
    println!("{}", bm.exists_interface(&interface).await?);
    bm.unbind(&interface).await?;
    bm.delete_interface(&interface).await?;
    bm.delete_network(&network).await?;
    println!("{}", bm.exists_network(&network).await?);
    println!("{}", bm.exists_interface(&interface).await?);

    Ok(println!("{:?}", interface))
}

pub struct Commands {}

impl Commands {
    fn get_clap(&self) -> clap::App<'static, 'static> {
        clap::clap_app!(emu =>
            (version: "0.1.0")
            (author: "Erik Hollensbe <github@hollensbe.org>")
            (about: "Control qemu & more")
            (@subcommand create =>
                (about: "Create vm with a sized image")
                (@arg NAME: +required "Name of VM")
                (@arg SIZE: +required "Size in GB of VM image")
            )
            (@subcommand delete =>
                (about: "Delete existing vm")
                (@arg NAME: +required "Name of VM")
            )
            (@subcommand supervise =>
                (about: "Configure supervision of an already existing VM")
                (@arg cdrom: -c --cdrom +takes_value "ISO of CD-ROM image -- will be embedded into supervision")
                (@arg NAME: +required "Name of VM")
            )
            (@subcommand unsupervise =>
                (about: "Remove supervision of an already existing VM")
                (@arg NAME: +required "Name of VM")
            )
            (@subcommand run =>
                (about: "Just run a pre-created VM; no systemd involved")
                (@arg headless: -e --headless "Run without a video window")
                (@arg detach: -d --detach "Do not wait for qemu to exit")
                (@arg cdrom: -c --cdrom +takes_value "ISO of CD-ROM image")
                (@arg extra_disk: --extra +takes_value "Supply an extra ISO image (useful for windows installations)")
                (@arg NAME: +required "Name of VM")
            )
            (@subcommand shutdown =>
                (about: "Gracefully shutdown a pre-created VM.")
                (@arg NAME: +required "Name of VM")
            )
            (@subcommand qmp =>
                (about: "Issue QMP commands to the guest.")
                (@arg NAME: +required "Name of VM")
                (@arg COMMAND: +required "Command to issue")
                (@arg ARGUMENTS: "Arguments to send for command, JSON literal")
            )
            (@subcommand list =>
                (about: "Yield a list of VMs, one on each line")
            )
            (@subcommand supervised =>
                (about: "Yield a list of supervised VMs, one on each line")
            )
            (@subcommand clone =>
                (about: "Clone one vm to another")
                (@arg FROM: +required "VM to clone from")
                (@arg TO: +required "VM to clone to")
            )
            (@subcommand import =>
                (about: "Import a VM from a VM image file")
                (@arg format: -f --format +takes_value +required "Format of incoming image")
                (@arg NAME: +required "VM to import to")
                (@arg FROM_FILE: +required "VM image to import from")
            )
            (@subcommand config =>
                (about: "Show and manipulate VM configuration")
                (@subcommand show =>
                    (about: "Show the written+inferred configuration for a VM")
                    (@arg NAME: +required "Name of VM")
                )
                (@subcommand set =>
                    (about: "Set a value in the configuration; type-safe")
                    (@arg NAME: +required "Name of VM")
                    (@arg KEY: +required "Name of key to set")
                    (@arg VALUE: +required "Value of key to set")
                )
                (@subcommand port =>
                    (about: "Adjust port mappings")

                    (@subcommand map =>
                        (about: "Add a port mapping from host -> guest")
                        (@arg NAME: +required "Name of VM")
                        (@arg HOSTPORT: +required "Port on localhost to map to guest")
                        (@arg GUESTPORT: +required "Port on guest to expose")
                    )
                    (@subcommand unmap =>
                        (about: "Undo a port mapping")
                        (@arg NAME: +required "Name of VM")
                        (@arg HOSTPORT: +required "Port on localhost to map to guest")
                    )
                )
            )
            // (@subcommand network_test =>
            //     (about: "you have a development build! :) p.s. don't run this")
            // )
        )
    }

    fn show_usage(&self, orig_args: &ArgMatches) -> Result<()> {
        let stderr = std::io::stderr();
        let mut lock = stderr.lock();
        lock.write_all(orig_args.usage().as_bytes())?;
        lock.write_all(b"\n\n")?;
        return Ok(());
    }

    fn evaluate_config_subcommand(&self, orig_args: &ArgMatches) -> Result<()> {
        let (cmd, args) = orig_args.subcommand();
        let args = match args {
            Some(args) => args,
            None => return self.show_usage(orig_args),
        };

        match cmd {
            "show" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                show_config(vm_name)?
            }),
            "set" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                let key = args.value_of("KEY").unwrap();
                let value = args.value_of("VALUE").unwrap();
                config_set(vm_name, key, value)?
            }),
            "port" => {
                let (cmd, portargs) = args.subcommand();
                let args = match portargs {
                    Some(args) => args,
                    None => return self.show_usage(args),
                };

                match cmd {
                    "map" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                        let hostport = args.value_of("HOSTPORT").unwrap_or("");
                        match hostport.parse::<u16>() {
                            Ok(hostport) => {
                                let guestport = args.value_of("GUESTPORT").unwrap_or("");
                                match guestport.parse::<u16>() {
                                    Ok(guestport) => port_map(vm_name, hostport, guestport)?,
                                    Err(e) => return Err(anyhow!(e)),
                                }
                            }
                            Err(e) => return Err(anyhow!(e)),
                        }
                    }),
                    "unmap" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                        let hostport = args.value_of("HOSTPORT").unwrap_or("");
                        match hostport.parse::<u16>() {
                            Ok(hostport) => port_unmap(vm_name, hostport)?,
                            Err(e) => return Err(anyhow!(e)),
                        }
                    }),
                    _ => Ok(()),
                }
            }
            _ => Ok(()),
        }
    }

    pub async fn evaluate(&self) -> Result<()> {
        let app = self.get_clap();
        let matches = app.clone().get_matches();
        let (cmd, args) = matches.subcommand();
        let args = match args {
            Some(args) => args,
            None => {
                let stderr = std::io::stderr();
                let mut lock = stderr.lock();
                app.clone().write_long_help(&mut lock)?;
                lock.write_all(b"\n\n")?;
                return Ok(());
            }
        };

        match cmd {
            "create" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                let size = args.value_of("SIZE").unwrap_or("");
                match size.parse::<u32>() {
                    Ok(u) => create(vm_name, u)?,
                    Err(e) => return Err(anyhow!(e)),
                }
            }),
            "delete" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                delete(vm_name)?
            }),
            "supervise" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                supervise(vm_name, args.value_of("cdrom"))?
            }),
            "unsupervise" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                unsupervise(vm_name)?
            }),
            "run" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                run(
                    vm_name,
                    args.value_of("cdrom"),
                    args.value_of("extra_disk"),
                    args.is_present("detach"),
                    args.is_present("headless"),
                )?
            }),
            "list" => list(),
            "shutdown" => Ok(if let Some(vm_name) = args.value_of("NAME") {
                shutdown(vm_name)?
            }),
            "qmp" => qmp(
                args.value_of("NAME").unwrap(),
                args.value_of("COMMAND").unwrap(),
                args.value_of("ARGUMENTS"),
            ),
            "supervised" => supervised(),
            "config" => self.evaluate_config_subcommand(args),
            "clone" => Ok(if let Some(from) = args.value_of("FROM") {
                if let Some(to) = args.value_of("TO") {
                    clone(from, to)?
                }
            }),
            "import" => import(
                args.value_of("NAME").unwrap(),
                args.value_of("FROM_FILE").unwrap(),
                args.value_of("format").unwrap(),
            ),
            "network_test" => network_test().await,
            _ => Ok(()),
        }
    }
}
