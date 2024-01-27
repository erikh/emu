use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser, Clone)]
#[command(author, version, about, long_about=None)]
pub struct Commands {
    #[command(subcommand)]
    pub command: CommandType,
}

#[derive(Debug, Subcommand, Clone)]
pub enum CommandType {
    /// Create vm with a sized image, or append storage to an existing VM
    Create {
        /// Append this VM image to an existing VM?
        #[arg(short, long, default_value = "false")]
        append: bool,
        /// Name of VM
        name: String,
        /// Size in GB of VM image
        size: usize,
    },
    /// Delete existing vm
    Delete {
        /// Name of VM
        name: String,
        /// (Optional) ID of disk to remove
        disk: Option<String>,
    },
    /// Rename VM to new name
    Rename {
        /// Original name of VM
        old: String,
        /// New name of VM
        new: String,
    },
    /// List all disks a VM has available
    ListDisks {
        /// Name of VM
        name: String,
    },
    /// Open standard input to a port on the VM
    NC {
        /// Name of VM
        name: String,
        /// Port of VM
        port: u16,
    },
    /// Uses ssh_port configuration variable to SSH into the host
    SSH {
        /// Name of VM
        name: String,
        /// Arguments to pass to `ssh` program (use -- to stop CLI processing)
        args: Option<Vec<String>>,
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
        /// Do not wait for the VM to terminate
        #[arg(short, long, default_value = "false")]
        nowait: bool,
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
    List {
        /// List only currently running VMs
        #[arg(short, long, default_value = "false")]
        running: bool,
    },
    /// Yield a list of supervised VMs, one on each line
    Supervised,
    /// Clone one VM to another
    Clone {
        /// Copy configuration as well
        #[arg(short, long, default_value = "false")]
        config: bool,
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
    /// Show and manipulate VM snapshots
    #[command(subcommand)]
    Snapshot(SnapshotSubcommand),
    /// Is this VM currently active?
    IsActive {
        /// Name of VM
        name: String,
    },
    /// Quickly save a snapshot of the named VM
    Save {
        /// Name of VM
        name: String,
    },
    /// Load the quick save of the named VM
    Load {
        /// Name of VM
        name: String,
    },
    /// Delete the quick save of the named VM
    ClearState {
        /// Name of VM
        name: String,
    },
    /// Hard Reset a VM
    Reset {
        /// Name of VM
        name: String,
    },
    /// Shutdown and re-launch the VM. Does not work with supervisors.
    Restart {
        /// Name of VM
        name: String,
    },
}

#[derive(Debug, Subcommand, Clone)]
pub enum ConfigSubcommand {
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
    /// Copy settings from one VM to another
    Copy {
        /// Name of VM to copy settings from
        from: String,
        /// Name of VM to copy settings to
        to: String,
    },
    /// Adjust port mappings
    #[command(subcommand)]
    Port(ConfigPortSubcommand),
}

#[derive(Debug, Subcommand, Clone)]
pub enum SnapshotSubcommand {
    /// Save a snapshot for a VM
    Save {
        /// Name of VM
        name: String,
        /// Name of snapshot to take (must not already exist)
        snapshot_name: String,
    },
    /// Delete an existing snapshot from a VM
    Delete {
        /// Name of VM
        name: String,
        /// Name of snapshot to take (must not already exist)
        snapshot_name: String,
    },
    /// Load an existing snapshot from a VM into the current state
    Load {
        /// Name of VM
        name: String,
        /// Name of snapshot to take (must not already exist)
        snapshot_name: String,
    },
}

#[derive(Debug, Subcommand, Clone)]
pub enum ConfigPortSubcommand {
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
