mod error;
mod image;
mod launcher;
mod storage;
mod template;

use error::Error;
use image::{Imager, QEmuImager};
use launcher::{EmulatorLauncher, QemuLauncher};
use storage::{DirectoryStorageHandler, StorageHandler};

fn main() -> Result<(), Error> {
    let dsh = DirectoryStorageHandler {
        basedir: "/tmp/foo",
    };

    let imager = QEmuImager::default();
    println!("{:?}", imager.create(dsh, "quux", 10));

    println!("{:?}", dsh.base_path());
    println!("{:?}", dsh.vm_list());
    println!("{:?}", dsh.vm_root("quux"));
    println!("{:?}", dsh.vm_exists("quux"));
    println!("{:?}", dsh.vm_path("quux", "qemu.qcow2"));
    println!("{:?}", dsh.vm_path_exists("quux", "qemu.qcow2"));

    let launcher = QemuLauncher::default();
    println!("{:?}", launcher.emulator_path());
    println!("{:?}", launcher.emulator_args("quux", Some("foo.iso"), dsh));
    println!(
        "{:?}",
        launcher.emulator_args(
            "quux",
            Some("/home/erikh/vm-images/isos/ubuntu-20.04.1-live-server-amd64.iso"),
            dsh,
        )
    );
    println!("{:?}", launcher.emulator_args("quux", None, dsh));

    // let mut child = launcher.launch_vm(
    //     "quux",
    //     Some("/home/erikh/vm-images/isos/ubuntu-20.04.1-live-server-amd64.iso"),
    //     dsh,
    // )?;
    //
    // println!("{:?}", child);
    // let exit = child.wait()?;
    // println!("{:?}", exit);

    let t = template::Systemd::new(Box::new(launcher), dsh);

    t.write(
        "quux",
        Some("/home/erikh/vm-images/isos/ubuntu-20.04.1-live-server-amd64.iso"),
    )
}
