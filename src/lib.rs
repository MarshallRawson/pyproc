use std::default::Default;
use std::collections::HashMap;
use std::io::{Read, Write};

use subprocess::{Popen, PopenConfig, Redirection};

pub struct PyProcBuilder {
    env: HashMap<String, String>,
    python: String,
    import: Option<String>,
}
impl Default for PyProcBuilder {
    fn default() -> Self {
        Self {
            python: "python3".to_string(),
            import: None,
            env: HashMap::new(),
        }
    }
}
impl PyProcBuilder {
    pub fn python(mut self, py: String) -> Self {
        self.python = py;
        self
    }
    pub fn import(mut self, f: Option<String>) -> Self {
        self.import = f;
        self
    }
    pub fn env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }
}



pub struct PyProc {
    proc: Popen,
}
impl PyProc {
    pub fn new(b: &PyProcBuilder) -> subprocess::Result<Self> {
        let argv: Vec<&str> = if let Some(import) = &b.import {
            vec![&b.python, "-i", &import, "-"]
        } else {
            vec![&b.python, "-"]
        };
        let proc = Popen::create(&argv, PopenConfig {
            stdin: Redirection::Pipe,
            stdout: Redirection::Pipe,
            stderr: Redirection::Pipe,
            env: if b.env.len() > 0 {
                Some(b.env.iter().map(|(n, v)| (n.into(), v.into())).collect())
            } else { None },
            ..Default::default()
        })?;
        assert!(proc.stdin.is_some());
        assert!(proc.stdout.is_some());
        assert!(proc.stderr.is_some());
        let mut s = Self { proc };
        //let out = s.read_ret();
        Ok(s)
    }
    fn read_ret(&mut self) -> String {
        let mut out = vec![];
        while out.len() < 5 || std::str::from_utf8(&out[out.len()-5..]) != Ok("\n>>> ") {
            println!("AA");
            let mut o = vec![0_u8; 1];
            self.proc.stdout.as_ref().unwrap().read_exact(&mut o).unwrap();
            out.push(o[0]);
            println!("out: {:?}", std::str::from_utf8(&out));
        }
        println!("BB");
        String::from_utf8_lossy(&out[0..out.len()-5]).to_string()
    }
    pub fn eval(&mut self, pycode: &str) -> Result<String, String> {
        println!("eval");
        write!(&mut self.proc.stdin.as_ref().unwrap(), "{pycode}\n").unwrap();
        let out = self.read_ret();
        if out.len() == 0 {
            let mut err = vec![128; 0];
            let mut d = self.proc.stdout.as_ref().unwrap().read(&mut err).unwrap();
            while d > 0 {
                let mut err2 = vec![128; 0];
                d = self.proc.stdout.as_ref().unwrap().read(&mut err2).unwrap();
                err.extend_from_slice(&err2[0..d]);
            }
            Err(String::from_utf8_lossy(&err).to_string())
        } else {
            Ok(out)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
