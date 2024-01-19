use serde_json::{json, Map, Result, Value};
use std::{
    io::{prelude::*, BufReader},
    os::unix::net::UnixStream,
};

pub struct UnixSocket {
    output: UnixStream,
    input: BufReader<UnixStream>,
}

pub trait Client {
    fn handshake(&mut self) -> Result<Vec<Value>>;
    fn parsed_reply(&mut self) -> Result<Value>;
    fn send_command(&mut self, execute: &str, args: Option<Map<String, Value>>) -> Result<Value>;
}

impl UnixSocket {
    pub fn new(us: UnixStream) -> std::io::Result<Self> {
        let clone = us.try_clone()?;
        return Ok(Self {
            output: us,
            input: BufReader::new(clone),
        });
    }

    fn read_input(&mut self) -> Result<Value> {
        let mut input = String::new();
        let mut tmp = String::new();
        loop {
            match self.input.read_line(&mut tmp) {
                Ok(_) => {}
                Err(e) => return Err(serde_json::Error::io(e)),
            };
            input += &tmp;
            if input.trim_end().ends_with("}") {
                match serde_json::from_str(&input) {
                    Ok(v) => return Ok(v),
                    Err(_) => {}
                }
            }
            tmp = String::new();
        }
    }

    fn send_output(&mut self, val: Value) -> Result<()> {
        match self.output.write_all(&val.to_string().as_bytes()) {
            Ok(_) => Ok(()),
            Err(e) => Err(serde_json::Error::io(e)),
        }
    }
}

impl Client for UnixSocket {
    fn handshake(&mut self) -> Result<Vec<Value>> {
        let value = self.read_input()?;
        match value["QMP"]["capabilities"].as_array() {
            Some(v) => Ok(v.clone()),
            None => Ok(Vec::new()),
        }
    }

    fn parsed_reply(&mut self) -> Result<Value> {
        self.read_input()
    }

    fn send_command(&mut self, execute: &str, args: Option<Map<String, Value>>) -> Result<Value> {
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
}
