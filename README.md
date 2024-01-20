# emu-cli: a small control toolkit for qemu

`emu-cli` installs the `emu` tool, which is a CLI program currently aimed at making x86 VM usage easier for Linux desktop users.

It contains commands to:

-   Manage VMs as a system-wide fleet
    -   List all VMs in one place, along with their run status
    -   Clone and Import VMs from other sources
-   Create, Delete, Start and Stop VMs
    -   ISOs can be attached
    -   You can start VMs with or without graphical screens
    -   `emu` does not have to be running to maintain your VM
-   Supervise VMs with systemd
    -   Uses the user profile (`systemctl --user`)
    -   Knows about which systemd units its maintaining
        -   Deletes them when the VM is deleted
-   Manage settings for VMs
    -   RAM, CPUs, Video & CPU type
    -   Forward Ports to VM networks
-   Define a SSH port that stays with the VM and `emu ssh` to it easily
-   Poke and prod at your VMs with `emu nc`, which opens a TCP socket to the port on the VM
-   Play with qemu QMP commands to control your VM externally

## Requirements

Linux with systemd and qemu. It places things according to the XDG standards, so that means `$HOME/.local` will have the VMs, etc.

To build the software, you will need a working rust environment. I strongly recommend [rustup](https://rustup.rs).

## Stability

emu is still undergoing some UI changes. I do use it somewhat regularly, but there may be additional work that needs to be done for a larger goal that changes small things as they stand now.

emu has bugs, particularly nasty ones at times related to your VMs. I would not use emu in a setting where data integrity mattered much.

## Installation

```
cargo install emu-cli
```

Once installed, you can invoke the software with `emu`.

## Usage

```bash
$ emu create myvm 50 # gigabytes of storage

# start the vm with the cdrom set to the ubuntu iso. Press ^C to terminate the vm.
$ emu run myvm --cdrom ubuntu.iso

# make a copy before doing something dumb
$ emu clone myvm myvm.template
$ emu list
myvm (unsupervised) (size: 6.10 GB)
myvm.template (unsupervised) (size: 6.10 GB)

# supervision in systemd
$ emu supervise myvm
$ emu supervised
myvm: not running
$ systemctl --user start myvm.emu # or enable it, if you'd like. it sticks to your login session.
$ emu list
myvm (supervised: running) (size: 6.10 GB)
myvm.template (unsupervised) (size: 6.10 GB)
$ systemctl --user stop myvm.emu # graceful shutdown
$ emu unsupervise myvm

# run detached and without a screen
$ emu run --detach --headless myvm
$ emu list
myvm (pid: 8675309) (size: 6.10 GB)
myvm.template (unsupervised) (size: 6.10 GB)

# ssh support
$ emu config port map myvm 2222 22
$ emu config set myvm ssh-port 2222
$ emu ssh myvm
myvm$ exit

# nc support
$ emu config port map myvm 8000 80
$ emu nc myvm 8000
GET / HTTP/1.1
Host: localhost
HTTP/1.1 403 Forbidden
Connection: close

# cleanup
$ emu shutdown myvm
$ emu remove myvm
$ emu list
myvm.template (unsupervised) (6.10 GB)
```

### Configuration

Configuration is provided currently by injecting values into a file under
`~/.local/share/emu/<VM>/config`. It is in TOML format.

#### Configuration Values

`[machine]` section:

-   `memory`: integer; memory in megabytes. Default is 16384.
-   `cpus`: integer; count of CPU cores. Default is 8.
-   `vga`: string; name of VGA driver to use with `qemu -vga`. Default is `virtio`.
-   `image_interface`: string; name of interface to use for talking to images with `-drive`. Default `virtio` is recommended.
-   `cpu_type`: string; type of CPU to support. Must be x86 and valid to pass to `qemu -cpu`. Default `host` is recommended.
-   `ssh_port`: integer; port to contact for SSH access; used by `emu ssh`. Default is 2222.

`[ports]` is just a key/value map of host ports, opened on `localhost`, to guest ports, opened on `0.0.0.0`. No other processing is performed.

#### Configuration Example

```toml
[machine]
cpus = 4
memory = 512

[ports]
2222 = 22
```

#### Management Tool

You can control these values with `emu config <subcommand>` sub-commands. `emu config show`, `emu config set`, and `emu config port` can be used to manage these sections.

The commands for `emu config set` are the same as the above `[machine]` section keys, only the underscores (`_`) are replaced with dashes (`-`); so that `ssh_port` is now `ssh-port`.

```bash
$ emu config show myvm
[machine]
cpus = 4
memory = 512

[ports]
2222 = 22

$ emu config port map myvm 2223 23
$ emu config port unmap myvm 2223

$ emu config set myvm ssh-port 2222
$ emu config set myvm cpus 8

$ emu config show myvm
[machine]
cpus = 8
memory = 512
ssh_port = 2222

[ports]
2222 = 22
2223 = 23
```

## License

MIT

## Author

Erik Hollensbe <github@hollensbe.org>
