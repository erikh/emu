# emu: a small control toolkit for qemu

`emu` contains commands to:

- start and shutdown vms
- supervise vms with userspace systemd (`systemctl --user`)
- configure vm parameters & forward ports
  - by use of a INI configuration file
- clone (stopped) vm images

## Requirements

The non-systemd parts probably would work on a mac with few changes. Until then
this is linux-only. You will also need to be a part of your system's `kvm`
group.

## Stability

Since emu is a really new project I'll keep this brief: do not depend on this
software.

I don't really know what I'm trying to do with this software yet. Until then,
perhaps you'll find it interesting or entertaining to play with.

## Installation

Install [rustup](https://rustup.rs) to get a cargo environment, and install it
from source:

```
cargo install --git https://code.hollensbe.org/erikh/emu
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
myvm
myvm.template

# supervision in systemd

$ emu supervise myvm
$ emu supervised
myvm
$ systemctl --user start myvm.emu # or enable it, if you'd like. it sticks to your login session.
$ systemctl --user stop myvm.emu # graceful shutdown
$ emu unsupervise myvm
$ emu remove myvm
$ emu list
myvm.template
```

### Configuration

Configuration is provided currently by injecting values into a file under
`~/.local/share/emu/<VM>/config`. It has this format (but more values, RTFS):

```ini
[machine]
cpus = 4 # actually cores
memory = 512 # megabytes

[ports]
2222 = 22 # host -> guest map
```

You can control these values with `emu config <subcommand>` sub-commands.

```bash
$ emu config show myvm
[machine]
cpus = 4
memory = 512

[ports]
2222 = 22

$ emu config port map myvm 2223 23
$ emu config port unmap myvm 2223

$ emu config set myvm cpus 8
```

## Author

Erik Hollensbe <github@hollensbe.org>
