use super::{image::QEMU_IMG_DEFAULT_FORMAT, traits::ConfigStorageHandler, vm::VM};
use crate::util::path_exists;
use anyhow::{anyhow, Result};
use std::{path::PathBuf, rc::Rc};

#[derive(Debug, Clone)]
pub struct XDGConfigStorage {
    base: PathBuf,
}

impl XDGConfigStorage {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }
}

impl Default for XDGConfigStorage {
    fn default() -> Self {
        let dir = dirs::data_dir().unwrap_or(dirs::home_dir().unwrap());
        let root = dir.join("emu");

        std::fs::create_dir_all(root.clone()).unwrap_or(());

        Self { base: root }
    }
}

impl ConfigStorageHandler for XDGConfigStorage {
    fn create(&self, vm: &VM) -> Result<()> {
        Ok(std::fs::create_dir_all(self.vm_root(vm))?)
    }

    fn rename(&self, old: &VM, new: &VM) -> Result<()> {
        Ok(std::fs::rename(self.vm_root(old), self.vm_root(new))?)
    }

    fn vm_root(&self, vm: &VM) -> PathBuf {
        self.base_path().join(vm.name())
    }

    fn running_vms(&self) -> Result<Vec<VM>> {
        let mut ret = Vec::new();

        for vm in self.vm_list()? {
            if vm.supervisor().is_active(&vm)? {
                ret.push(vm);
            }
        }

        Ok(ret)
    }

    fn vm_list(&self) -> Result<Vec<VM>> {
        match std::fs::read_dir(self.base_path()) {
            Ok(rd) => {
                let mut ret = Vec::new();
                for dir in rd {
                    match dir {
                        Ok(dir) => match dir.file_name().into_string() {
                            Ok(s) => ret.push(VM::new(s, Rc::new(Box::new(self.clone())))),
                            Err(_) => {
                                return Err(anyhow!(
                                "could not iterate base directory; some vm filenames are invalid"
                            ))
                            }
                        },
                        Err(e) => return Err(anyhow!("could not iterate directory: {}", e)),
                    }
                }

                Ok(ret)
            }
            Err(e) => Err(anyhow!(e)),
        }
    }

    fn vm_path(&self, vm: &VM, filename: &str) -> PathBuf {
        self.vm_root(vm).join(filename)
    }

    fn vm_path_exists(&self, vm: &VM, filename: &str) -> bool {
        path_exists(self.vm_path(vm, filename))
    }

    fn pidfile(&self, vm: &VM) -> PathBuf {
        self.vm_path(vm, "pid")
    }

    fn base_path(&self) -> PathBuf {
        self.base.clone()
    }

    fn vm_exists(&self, vm: &VM) -> bool {
        path_exists(self.vm_root(vm))
    }

    fn delete(&self, vm: &VM, disk: Option<String>) -> Result<()> {
        if !self.vm_exists(vm) {
            return Err(anyhow!("vm doesn't exist"));
        }

        let root = self.vm_root(vm);
        if let Some(disk) = disk {
            std::fs::remove_file(root.join(format!("qemu-{}.{}", disk, QEMU_IMG_DEFAULT_FORMAT)))?;
        } else {
            std::fs::remove_dir_all(root)?;
        }

        Ok(())
    }

    fn disk_list(&self, vm: &VM) -> Result<Vec<PathBuf>> {
        if !self.vm_exists(vm) {
            return Err(anyhow!("vm does not exist"));
        }

        let mut v = Vec::new();

        let dir = std::fs::read_dir(self.vm_root(vm))?;
        for item in dir.flatten() {
            if item
                .path()
                .to_str()
                .unwrap()
                .ends_with(&format!(".{}", QEMU_IMG_DEFAULT_FORMAT))
            {
                v.push(item.path());
            }
        }

        v.sort();

        Ok(v)
    }

    fn config_path(&self, vm: &VM) -> PathBuf {
        self.vm_path(vm, "config")
    }

    fn monitor_path(&self, vm: &VM) -> PathBuf {
        self.vm_path(vm, "mon")
    }

    fn write_config(&self, vm: VM) -> Result<()> {
        vm.config().to_file(self.config_path(&vm))
    }

    fn size(&self, vm: &VM) -> Result<usize> {
        let dir = std::fs::read_dir(self.vm_root(vm))?;
        let mut total = 0;
        let mut items = Vec::new();
        let mut dirs = vec![dir];
        while let Some(dir) = dirs.pop() {
            for item in dir.flatten() {
                let meta = item.metadata()?;
                if meta.is_file() {
                    items.push(item);
                }
            }
        }

        for item in items {
            let meta = item.metadata()?;
            total += meta.len() as usize;
        }

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::VM;
    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn test_xdg_storage() -> Result<()> {
        let base = tempdir()?;
        let base_path = base.path().to_path_buf();
        let storage = XDGConfigStorage::new(base_path.clone());

        let vm1: VM = "vm1".to_string().into();
        let vm2: VM = "vm2".to_string().into();
        let vm3: VM = "vm3".to_string().into();

        assert_eq!(storage.vm_list()?, vec![]);
        storage.create(&vm1)?;
        storage.create(&vm2)?;
        assert_eq!(storage.vm_list()?, vec![vm1.clone(), vm2.clone()]);
        storage.rename(&vm2, &vm3)?;
        assert_eq!(storage.vm_list()?, vec![vm1.clone(), vm3.clone()]);
        storage.create(&vm2)?;
        assert_eq!(
            storage.vm_list()?,
            vec![vm1.clone(), vm3.clone(), vm2.clone()]
        );

        assert_eq!(storage.vm_root(&vm1), base_path.join(vm1.name()));
        assert_eq!(storage.vm_root(&vm2), base_path.join(vm2.name()));

        assert!(storage.vm_exists(&vm1));
        storage.delete(&vm1, None)?;
        assert!(!storage.vm_exists(&vm1));
        storage.create(&vm1)?;

        assert!(storage.size(&vm1)? == 0);
        assert_eq!(
            storage.vm_path(&vm1, "config"),
            base_path.join("vm1/config")
        );
        assert!(!storage.vm_path_exists(&vm1, "config"));
        assert_eq!(storage.config_path(&vm1), base_path.join("vm1/config"));
        assert_eq!(storage.pidfile(&vm1), base_path.join("vm1/pid"));
        assert_eq!(storage.monitor_path(&vm1), base_path.join("vm1/mon"));
        assert_eq!(storage.disk_list(&vm1)?.len(), 0);
        assert_eq!(storage.running_vms()?.len(), 0);
        storage.write_config(vm1.clone())?;
        assert!(storage.vm_path_exists(&vm1, "config"));

        base.close()?;
        Ok(())
    }
}
