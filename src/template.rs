use super::vm::VM;
use anyhow::{anyhow, Result};
use serde::Serialize;
use tinytemplate::TinyTemplate;

const EMU_DEFAULT_PATH: &str = "/bin/emu";

const SYSTEMD_UNIT: &str = "
[Unit]
Description=Virtual Machine: {vm_name}

[Service]
Type=simple
ExecStart={emu_path} run -e {vm_name}
TimeoutStopSec=30
ExecStop={emu_path} shutdown {vm_name}
KillSignal=SIGCONT
FinalKillSignal=SIGKILL

[Install]
WantedBy=default.target
";

#[derive(Serialize)]
pub struct Data {
    vm_name: String,
    emu_path: String,
}

impl Data {
    pub fn new(vm_name: String) -> Self {
        Self {
            vm_name,
            emu_path: match std::env::current_exe() {
                Ok(path) => path.to_str().unwrap().to_string(),
                Err(_) => EMU_DEFAULT_PATH.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Systemd;

impl Systemd {
    pub fn template(&self, vm: &VM) -> Result<String> {
        let mut t = TinyTemplate::new();
        t.add_template("systemd", SYSTEMD_UNIT)?;
        let data = Data::new(vm.name());
        match t.render("systemd", &data) {
            Ok(x) => Ok(x),
            Err(e) => Err(anyhow!(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn test_template() -> Result<()> {
        let out = Systemd::template(&Systemd, &"vm1".to_string().into())?;
        assert!(out.contains("vm1"));

        Ok(())
    }
}
