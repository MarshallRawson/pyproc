use std::io::Write;

use pyproc::PyProc;

fn main() {
    let mut py_proc = PyProc::new().unwrap();
    println!("{:?}", py_proc.run(
r#"
def foo():
    print('hello from foo!')
"#));
    println!("{:?}", py_proc.eval("foo()"));
    println!("{:?}", py_proc.run("print('hello from python!')"));
    println!("{:?}", py_proc.run("a = 2"));
    println!("{:?}", py_proc.get("a"));
    println!("{:?}", py_proc.eval("1+1"));
    println!("{:?}", py_proc.eval("'abc'"));
}

