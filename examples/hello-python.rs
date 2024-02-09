use std::ffi::CString;

extern crate libc;

use libc::{c_char, c_int, FILE, fopen, fclose, pid_t};

use fork::{fork, Fork};
use pipe::{PipeReader, PipeWriter};

type PyObjPtr = *const i8;
#[repr(i32)]
#[derive(Clone, Debug)]
enum PyGILState {
    PyGILStateLocked,
    PyGILStateUnlocked,
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
    fn PyRun_String(code: *const c_char, start: c_int, globals: PyObjPtr, locals: PyObjPtr) -> PyObjPtr;
    fn PyRun_File(fp: *mut FILE, fname: *const c_char, globals: PyObjPtr, locals: PyObjPtr) -> PyObjPtr;
    fn Py_Finalize();
}
fn pygil_ensure() -> PyGILState { unsafe { PyGILState_Ensure() } }
fn pygil_release(gil: PyGILState) { unsafe { PyGILState_Release(gil) } }
fn py_initialize() { unsafe { Py_Initialize(); }}
fn pydict_new() -> PyObj {
    PyObj::new(unsafe { PyDict_New() })
}
fn pyrun_string(s: &str, state: &PyState) -> PyObj {
    let cs = CString::new(s).unwrap();
    PyObj::new( unsafe {
        PyRun_String(cs.as_ptr() as *const i8, PY_FILE_INPUT, state.globals.p, state.locals.p)
    })
}
fn pyrun_file(f: &str, state: &PyState) -> std::io::Result<PyObj> {
    let path = CString::new(f).unwrap();
    let r = CString::new("r").unwrap();
    let file_ptr = unsafe { fopen(path.as_ptr() as *const i8, r.as_ptr() as *const i8) };
    if file_ptr.is_null() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("failed to open {f}")));
    }
    let res = PyObj::new( unsafe {
        PyRun_File(file_ptr, path.as_ptr() as *const i8, state.globals.p, state.locals.p)
    });
    unsafe { fclose(file_ptr); }
    Ok(res)
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
    state: &'a PyGil<'a>,
}
impl<'a> PyState<'a> {
    fn new(state: &'a PyGil) -> Self {
        Self {
            globals: pydict_new(),
            locals: pydict_new(),
            state: state,
        }
    }
}
struct PyGil<'a> {
    py: &'a Py,
    gil: PyGILState
}
impl<'a> PyGil<'a> {
    fn new(py: &'a Py) -> Self {
        let gil = pygil_ensure();
        Self { py: py, gil: gil }
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

pub struct PyProc {
    read: PipeReader,
    write: PipeWriter,
    child: pid_t,
}
impl PyProc {
    pub fn new(files: Vec<String>) -> Result<Self, i32> {
        let (mut parent_read, mut child_write) = pipe::pipe();
        let (mut child_read, mut parent_write) = pipe::pipe();
        match fork() {
            Ok(Fork::Child) => {
                Self::main(child_read, child_write, files);
                std::process::exit(0);
                Err(0)
            },
            Ok(Fork::Parent(child)) => {
                Ok(Self {read: parent_read, write: parent_write, child: child })
            }
            Err(e) => { return Err(e); },
        }
    }
    fn main(r: PipeReader, w: PipeWriter, files: Vec<String>) {
        let py = Py::new();
        let gil = PyGil::new(&py);
        let state = PyState::new(&gil);
        for f in files {
            pyrun_file(&f, &state).unwrap();
        }
        // read string from r
        // eval with pyrun_string
        // stringify 
//        let ret = pyrun_string(r#"print('Hello from Python!')
//a = 1
//a += 1
//print(a)
//def foo(): return "bar"
//foo()
//        "#, &state);
    }
}


fn main() {
    
    
}

