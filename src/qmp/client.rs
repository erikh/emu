use super::messages::{ErrorReturn, Event, GenericReturn, JobInfo, QueryBlock, QueryJobs};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::{
    io::{prelude::*, BufReader},
    os::unix::net::UnixStream,
    path::PathBuf,
};

pub struct Client {
    output: UnixStream,
    input: BufReader<UnixStream>,
}

impl Client {
    pub fn new(us: PathBuf) -> std::io::Result<Self> {
        let stream = UnixStream::connect(us)?;
        return Ok(Self {
            output: stream.try_clone()?,
            input: BufReader::new(stream),
        });
    }

    fn read_input<T>(&mut self) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de> + Default + std::fmt::Debug,
    {
        let mut buf = String::new();
        while let Ok(_) = self.input.read_line(&mut buf) {
            if buf.ends_with("\r\n}\r\n") {
                match serde_json::from_str::<T>(&buf) {
                    Ok(obj) => {
                        return Ok(obj);
                    }
                    Err(e) => {
                        // incoming event, ignore it and retry
                        if let Ok(_) = serde_json::from_str::<Event>(&buf) {
                            buf = String::new();
                        } else if let Ok(e) = serde_json::from_str::<ErrorReturn>(&buf) {
                            // got an error, return it
                            return Err(e.into());
                        } else if let Ok(ret) = serde_json::from_str::<GenericReturn>(&buf) {
                            return ret.into();
                        } else {
                            // return the original error
                            return Err(e.into());
                        }
                    }
                }
            }
        }

        return Err(anyhow!("Read past end of input"));
    }

    fn send_output(&mut self, val: Value) -> Result<()> {
        match self.output.write_all(&val.to_string().as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(e)),
        }
    }

    pub fn handshake(&mut self) -> Result<()> {
        // read_input hangs if the type isn't specified
        self.read_input::<Event>()?;
        Ok(())
    }

    pub fn parsed_reply(&mut self) -> Result<GenericReturn> {
        self.read_input()
    }

    pub fn send_command<T>(&mut self, execute: &str, args: Option<Value>) -> Result<T>
    where
        T: for<'de> serde::Deserialize<'de> + Default + std::fmt::Debug,
    {
        if let Some(args) = args {
            self.send_output(json!({
                "execute": execute,
                "arguments": args,
            }))?;
        } else {
            self.send_output(json!({
                "execute": execute,
            }))?;
        }

        self.read_input()
    }

    pub fn block_devices(&mut self) -> Result<QueryBlock> {
        self.send_command("query-block", None)
    }

    pub fn jobs(&mut self) -> Result<QueryJobs> {
        self.send_command("query-jobs", None)
    }

    pub fn disk_nodes(&mut self) -> Result<Vec<String>> {
        let blocks = self.block_devices()?.result;

        let mut disks = Vec::new();

        for item in blocks {
            if let Some(inserted) = item.inserted {
                if let Some(name) = inserted.node_name {
                    disks.push(name)
                }
            }
        }

        Ok(disks)
    }

    pub fn wait_for_job(&mut self, id: &str) -> Result<JobInfo> {
        loop {
            let res = self.jobs();

            if let Ok(jobs) = res {
                for job in jobs.result {
                    if job.id == id {
                        match job.status.as_str() {
                            "concluded" | "null" => {
                                if let Some(error) = job.error {
                                    self.delete_job(id)?;
                                    return Err(anyhow!(error));
                                } else {
                                    self.delete_job(id)?;
                                    return Ok(job);
                                }
                            }
                            _ => {}
                        }
                        break;
                    }
                }
            } else if let Err(e) = res {
                self.delete_job(id)?;
                return Err(e);
            }

            std::thread::sleep(std::time::Duration::new(0, 200))
        }
    }

    pub fn delete_job(&mut self, id: &str) -> Result<()> {
        loop {
            let mut found = false;
            let res = self.send_command::<QueryJobs>("job-dismiss", Some(json!({"id": id})));
            if let Ok(jobs) = res {
                for job in &jobs.result {
                    if job.id == id {
                        found = true;
                    }
                }
            } else {
                break;
            }

            if !found {
                break;
            }
        }

        Ok(())
    }

    fn cleanup_job(&mut self, res: Result<GenericReturn, anyhow::Error>, id: &str) -> Result<()> {
        if let Err(e) = self.wait_for_job(id) {
            self.delete_job(id)?;
            return Err(e);
        }

        if let Err(e) = res {
            return Err(e);
        }

        Ok(())
    }

    pub fn snapshot_save(&mut self, name: &str) -> Result<()> {
        let disks = self.disk_nodes()?;

        let res = self.send_command::<GenericReturn>(
            "snapshot-save",
            Some(json!({
                "job-id": "snapshot",
                "tag": name,
                "vmstate": disks[0],
                "devices": disks,
            })),
        );

        self.cleanup_job(res, "snapshot")
    }

    pub fn snapshot_load(&mut self, name: &str) -> Result<()> {
        let disks = self.disk_nodes()?;

        let res = self.send_command::<GenericReturn>(
            "snapshot-load",
            Some(json!({
                "job-id": "snapshot",
                "tag": name,
                "vmstate": disks[0],
                "devices": disks,
            })),
        );

        self.cleanup_job(res, "snapshot")
    }

    pub fn snapshot_delete(&mut self, name: &str) -> Result<()> {
        let disks = self.disk_nodes()?;

        let res = self.send_command::<GenericReturn>(
            "snapshot-delete",
            Some(json!({
                "job-id": "snapshot",
                "tag": name,
                "devices": disks,
            })),
        );

        self.cleanup_job(res, "snapshot")
    }
}
