use std::io::Write;
use std::process::{Command, Stdio};

use crate::error::Error;
use crate::image::{Imager, QEmuImager};
use crate::launcher::{EmulatorLauncher, QemuLauncher};
use crate::network::{BridgeManager, NetworkManager};
use crate::storage::{DirectoryStorageHandler, StorageHandler};
use crate::template::Systemd;

fn list() -> Result<(), Error> {
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

fn supervised() -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();

    let launcher = QemuLauncher::default();
    let t = Systemd::new(Box::new(launcher), dsh);
    match t.list() {
        Ok(list) => {
            for vm in list {
                println!("{}", vm)
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn create(vm_name: &str, size: u32) -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(Error::new("invalid VM name"));
    }

    if dsh.vm_exists(vm_name) {
        return Err(Error::new("vm already exists"));
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

fn delete(vm_name: &str) -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(Error::new("invalid VM name"));
    }

    if !dsh.vm_exists(vm_name) {
        return Err(Error::new("vm doesn't exist"));
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

fn supervise(vm_name: &str, cdrom: Option<&str>) -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(Error::new("invalid VM name"));
    }

    if !dsh.vm_exists(vm_name) {
        return Err(Error::new("vm doesn't exist"));
    }

    let launcher = QemuLauncher::default();
    let t = Systemd::new(Box::new(launcher), dsh);
    if let Err(e) = t.write(vm_name, cdrom) {
        return Err(e);
    }

    reload_systemd()
}

fn unsupervise(vm_name: &str) -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(vm_name) {
        return Err(Error::new("invalid VM name"));
    }

    let launcher = QemuLauncher::default();
    let t = Systemd::new(Box::new(launcher), dsh);
    t.remove(vm_name)?;

    reload_systemd()
}

fn reload_systemd() -> Result<(), Error> {
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
                Err(Error::new(&format!("systemctl exited uncleanly: {}", es)))
            }
        }
        Err(e) => Err(Error::from(e)),
    }
}

fn run(vm_name: &str, cdrom: Option<&str>) -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();
    if !dsh.valid_filename(vm_name) {
        return Err(Error::new("invalid VM name"));
    }

    let launcher = QemuLauncher::default();
    let mut child = launcher.launch_vm(vm_name, cdrom, dsh)?;

    let exit = child.wait();
    match exit {
        Ok(es) => {
            if es.success() {
                Ok(())
            } else {
                Err(Error::new(&format!("qemu exited uncleanly: {}", es)))
            }
        }
        Err(e) => Err(Error::from(e)),
    }
}

fn clone(from: &str, to: &str) -> Result<(), Error> {
    let dsh = DirectoryStorageHandler::default();

    if !dsh.valid_filename(to) {
        return Err(Error::new("invalid VM name"));
    }

    if dsh.vm_exists(to) {
        return Err(Error::new("vm already exists"));
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

async fn network_test() -> Result<(), Error> {
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
                (@arg cdrom: -c --cdrom +takes_value "ISO of CD-ROM image")
                (@arg NAME: +required "Name of VM")
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
            // (@subcommand network_test =>
            //     (about: "you have a development build! :) p.s. don't run this")
            // )
        )
    }

    pub async fn evaluate(&self) -> Result<(), Error> {
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
                    Err(e) => return Err(Error::from(e)),
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
                run(vm_name, args.value_of("cdrom"))?
            }),
            "list" => list(),
            "supervised" => supervised(),
            "clone" => Ok(if let Some(from) = args.value_of("FROM") {
                if let Some(to) = args.value_of("TO") {
                    clone(from, to)?
                }
            }),
            "network_test" => network_test().await,
            _ => Ok(()),
        }
    }
}
