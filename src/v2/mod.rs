pub mod command;
pub mod command_handler;
pub mod config_storage;
pub mod image;
pub mod launcher;
pub mod supervisor;
pub mod template;
pub mod traits;
pub mod vm;

use self::{
    command::{CommandType, Commands, ConfigPortSubcommand, ConfigSubcommand},
    command_handler::CommandHandler,
};
use anyhow::Result;
use clap::Parser;

pub async fn evaluate() -> Result<()> {
    let handler = CommandHandler::default();
    let args = Commands::parse();

    match args.command {
        CommandType::Config(sub) => match sub {
            ConfigSubcommand::Set { name, key, value } => {
                handler.config_set(&name.into(), key, value)
            }
            ConfigSubcommand::Copy { from, to } => handler.config_copy(&from.into(), &to.into()),
            ConfigSubcommand::Show { name } => handler.show_config(&name.into()),
            ConfigSubcommand::Port(sub) => match sub {
                ConfigPortSubcommand::Map {
                    name,
                    hostport,
                    guestport,
                } => handler.port_map(&name.into(), hostport, guestport),
                ConfigPortSubcommand::Unmap { name, hostport } => {
                    handler.port_unmap(&name.into(), hostport)
                }
            },
        },
        CommandType::ListDisks { name } => handler.list_disks(&name.into()),
        CommandType::NC { name, port } => handler.nc(&name.into(), port).await,
        CommandType::SSH { name, args } => handler.ssh(&name.into(), args),
        CommandType::Create { append, name, size } => handler.create(&name.into(), size, append),
        CommandType::Rename { old, new } => handler.rename(&old.into(), &new.into()),
        CommandType::Delete { name, disk } => handler.delete(&name.into(), disk),
        CommandType::Supervise { cdrom, name } => {
            let mut vm: vm::VM = name.into();
            if let Some(cdrom) = cdrom {
                vm.set_cdrom(cdrom)
            }
            handler.supervise(&vm)
        }
        CommandType::Unsupervise { name } => handler.unsupervise(&name.into()),
        CommandType::Run {
            headless,
            detach,
            cdrom,
            extra_disk,
            name,
        } => {
            let mut vm: vm::VM = name.into();
            vm.set_headless(headless);
            if let Some(cdrom) = cdrom {
                vm.set_cdrom(cdrom);
            }
            if let Some(extra_disk) = extra_disk {
                vm.set_extra_disk(extra_disk)
            }

            handler.run(&vm, detach)
        }
        CommandType::List { running } => handler.list(running),
        CommandType::Shutdown { name } => handler.shutdown(&name.into()),
        CommandType::QMP {
            name,
            command,
            arguments,
        } => handler.qmp(&name.into(), &command, arguments.as_deref()),
        CommandType::Supervised => handler.supervised(),
        CommandType::Clone { from, to } => handler.clone(&from.into(), &to.into()),
        CommandType::Import {
            format,
            name,
            from_file,
        } => handler.import(&name.into(), from_file, format),
        CommandType::IsActive { name } => handler.is_active(&name.into()),
    }
}
