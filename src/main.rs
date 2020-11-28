mod error;
mod image;
mod launcher;
mod storage;

use image::{Imager, QEmuImager};
use storage::{DirectoryStorageHandler, StorageHandler, StorageHandlers};

fn main() {
    let mut handlers = StorageHandlers::new();
    let dsh = Box::new(DirectoryStorageHandler {
        basedir: String::from("/tmp/foo"),
    });

    let imager = QEmuImager::default();
    println!("{:?}", imager.create(dsh.clone(), "quux", 10));

    println!("{:?}", dsh.base_path());
    println!("{:?}", dsh.vm_list());
    println!("{:?}", dsh.vm_root("quux"));
    println!("{:?}", dsh.vm_exists("quux"));
    println!("{:?}", dsh.vm_path("quux", "qemu.qcow2"));
    println!("{:?}", dsh.vm_path_exists("quux", "qemu.qcow2"));
    handlers.register("directory", dsh);
    println!("{:?}", handlers);
}
