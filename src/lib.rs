use std::ffi::{CStr, CString};
use std::io::{Read, Write};
use std::os::fd::FromRawFd;
use std::fs::File;

extern crate libc;

use libc::{c_char, c_int, FILE, fopen, fclose, pid_t, pipe, close, kill, SIGKILL};
use std::os::fd::RawFd;

use fork::{fork, Fork};

type PyObjPtr = *const c_char;
#[repr(i32)]
#[derive(Clone, Debug)]
enum PyGILState {
    _PyGILStateLocked,
    _PyGILStateUnlocked,
}

const PY_SINGLE_INPUT: c_int = 256;
const PY_FILE_INPUT: c_int = 257;
const PY_EVAL_INPUT: c_int = 258;
extern "C" {
    fn Py_Initialize();
    fn Py_DecRef(o: PyObjPtr);
    fn Py_IncRef(o: PyObjPtr);
    fn PyGILState_Ensure() -> PyGILState;
    fn PyGILState_Release(s: PyGILState);
    fn PyDict_New() -> PyObjPtr;
    fn PyDict_GetItemString(d: PyObjPtr, key: *const c_char) -> PyObjPtr;
    fn PyRun_String(code: *const c_char, start: c_int, globals: PyObjPtr, locals: PyObjPtr) -> PyObjPtr;
    fn PyRun_File(fp: *mut FILE, fname: *const c_char, globals: PyObjPtr, locals: PyObjPtr) -> PyObjPtr;
    fn PyObject_Str(obj: PyObjPtr) -> PyObjPtr;
    fn PyObject_Repr(obj: PyObjPtr) -> PyObjPtr;
    fn PyUnicode_AsUTF8(s: PyObjPtr) -> *const c_char;
    fn Py_IsNone(s: PyObjPtr) -> c_int;
    fn Py_Finalize();
}
fn pygil_ensure() -> PyGILState { unsafe { PyGILState_Ensure() } }
fn pygil_release(gil: PyGILState) { unsafe { PyGILState_Release(gil) } }
fn py_initialize() { unsafe { Py_Initialize(); }}
fn pydict_new() -> PyObj {
    PyObj::new(unsafe { PyDict_New() })
}
fn pydict_getitemstring(d: &PyObj, key: &str) -> PyObj {
    let k = CString::new(key).unwrap();
    PyObj::new(unsafe { PyDict_GetItemString(d.p, k.as_ptr()) })
}
fn pyrun_string(s: &str, state: &PyState) -> PyObj {
    let cs = CString::new(s).unwrap();
    PyObj::new( unsafe {
        PyRun_String(cs.as_ptr(), PY_FILE_INPUT, state.globals.p, state.locals.p)
    })
}
fn pyrun_file(f: &str, state: &PyState) -> std::io::Result<PyObj> {
    let path = CString::new(f).unwrap();
    let r = CString::new("r").unwrap();
    let file_ptr = unsafe { fopen(path.as_ptr(), r.as_ptr()) };
    if file_ptr.is_null() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("failed to open {f}")));
    }
    let res = PyObj::new( unsafe {
        PyRun_File(file_ptr, path.as_ptr(), state.globals.p, state.locals.p)
    });
    unsafe { fclose(file_ptr); }
    Ok(res)
}
fn pyobject_str(obj: &PyObj) -> Result<PyObj, String> {
    let r = unsafe { PyObject_Repr(obj.p) };
    if r.is_null() {
        Err("Failed to str object".to_string())
    } else {
        Ok(PyObj::new(r))
    }
}
fn pyunicode_asutf8(s: &PyObj) -> Result<String, String> {
    let c_str = unsafe { PyUnicode_AsUTF8(s.p) };
    if c_str.is_null() {
        Err("Failed to convert to UTF8 String".to_string())
    } else {
        let s = unsafe { CStr::from_ptr(c_str) }.to_str().unwrap();
        Ok(format!("{}", s))
    }
}
fn py_finalize() { unsafe { Py_Finalize(); }}

struct PyObj {
    p: PyObjPtr
}
impl PyObj {
    fn new(p: PyObjPtr) -> Self {
        Self { p }
    }
}
impl Drop for PyObj {
    fn drop(&mut self) -> () { unsafe {
        Py_DecRef(self.p);
    }
}}
impl Clone for PyObj {
    fn clone(&self) -> Self { unsafe {
        Py_IncRef(self.p);
        Self { p: self.p }
    }
}}

struct PyState<'a> {
    globals: PyObj,
    locals: PyObj,
    _state: &'a PyGil<'a>,
}
impl<'a> PyState<'a> {
    fn new(state: &'a PyGil) -> Self {
        Self {
            globals: pydict_new(),
            locals: pydict_new(),
            _state: state,
        }
    }
}
struct PyGil<'a> {
    _py: &'a Py,
    gil: PyGILState
}
impl<'a> PyGil<'a> {
    fn new(py: &'a Py) -> Self {
        let gil = pygil_ensure();
        Self { _py: py, gil: gil }
    }
}
impl Drop for PyGil<'_> {
    fn drop(&mut self) {
        pygil_release(self.gil.clone());
    }
}
struct Py {
}
impl Py {
    fn new() -> Self {
        py_initialize();
        Self {}
    }
}
impl Drop for Py {
    fn drop(&mut self) {
        py_finalize();
    }
}

struct Child(pid_t);
impl Drop for Child {
    fn drop(&mut self) {
        unsafe { kill(self.0, SIGKILL); }
    }
}

const U64_SIZE: u64 = 8;
#[repr(u8)]
enum Mode {
    Run = 0,
    Get = 1,
}

pub struct PyProc {
    r: File,
    w: File,
    _child: Child,
}
impl PyProc {
    pub fn new(/*files: Vec<String>*/) -> Result<Self, i32> {
        // parent_read, child_write
        let mut fd1: [RawFd; 2] = [0; 2];
        unsafe { pipe(&mut fd1[0] as *mut RawFd); }
        // child_read, parent_write
        let mut fd2: [RawFd; 2] = [0; 2];
        unsafe { pipe(&mut fd2[0] as *mut RawFd); }
        match fork() {
            Ok(Fork::Child) => {
                unsafe { close(fd2[1]); }
                unsafe { close(fd1[0]); }
                match Self::main(
                    unsafe { File::from_raw_fd(fd2[0])/*child_read*/},
                    unsafe { File::from_raw_fd(fd1[1])/*child_write*/},
                    //files,
                ) {
                    Ok(_) => std::process::exit(0),
                    Err(_) => std::process::exit(1),
                }
            },
            Ok(Fork::Parent(child)) => {
                unsafe { close(fd2[0]); }
                unsafe { close(fd1[1]); }
                Ok(Self {
                    r: unsafe { File::from_raw_fd(fd1[0])/*parent_read*/},
                    w: unsafe { File::from_raw_fd(fd2[1])/*parent_write*/},
                    _child: Child(child),
                })
            }
            Err(e) => { return Err(e); },
        }
    }
    fn main(mut r: File, mut w: File, /*files: Vec<String>*/) -> std::io::Result<()> {
        let py = Py::new();
        let gil = PyGil::new(&py);
        let state = PyState::new(&gil);
        //for f in files {
        //    pyrun_file(&f, &state).unwrap();
        //}
        loop {
            // read
            let len = {
                let mut len_buf = [0; U64_SIZE as usize];
                r.read_exact(&mut len_buf)?;
                u64::from_be_bytes(len_buf)
            };
            let input = {
                let mut input = vec![0; len as usize];
                r.read_exact(&mut input)?;
                String::from_utf8(input).unwrap()
            };
            let mode: Mode = {
                let mut m = [0_u8];
                r.read_exact(&mut m[..])?;
                match m[0] {
                    0 => Mode::Run,
                    1 => Mode::Get,
                    _ => panic!(),
                }
            };
            let result = match mode {
                Mode::Run => {
                    // eval with pyrun_string
                    let result = pyrun_string(&input, &state);
                    // stringify
                    let result = pyobject_str(&result).unwrap();
                    result
                },
                Mode::Get => {
                    pydict_getitemstring(&state.locals, &input)
                },
            };
            let result = pyobject_str(&result).unwrap();
            let result = pyunicode_asutf8(&result).unwrap();
            // write
            let s = result.as_bytes();
            let len = s.len() as u64;
            w.write(&len.to_be_bytes()[..])?;
            w.write(s)?;
        }
    }
    fn transaction(&mut self, s: &str, mode: Mode) -> std::io::Result<String> {
        let s = s.as_bytes();
        let len = s.len() as u64;
        self.w.write(&len.to_be_bytes()[..])?;
        self.w.write(s)?;
        self.w.write(&[mode as u8][..])?;
        let len = {
            let mut len_buf = [0; U64_SIZE as usize];
            self.r.read_exact(&mut len_buf)?;
            u64::from_be_bytes(len_buf)
        };
        let output = {
            let mut output = vec![0; len as usize];
            self.r.read_exact(&mut output)?;
            String::from_utf8(output).unwrap()
        };
        Ok(output)
    }
    pub fn run(&mut self, s: &str) -> std::io::Result<()> {
        let _ = self.transaction(&format!("{s}\n"), Mode::Run)?;
        Ok(())
    }
    pub fn get(&mut self, s: &str) -> std::io::Result<String> {
        Ok(self.transaction(s, Mode::Get)?)
    }
    pub fn eval(&mut self, s: &str) -> std::io::Result<String> {
        let _ = self.run(&format!("_ = ({s})\n"))?;
        Ok(self.get("_")?)
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
