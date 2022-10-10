use std::io::{stdout, Stdout, StdoutLock, Write};

/// An abstraction point around a [`Write`] implementation. Allows redirecting output for test
/// assertions.
pub struct OutputStream<Writer: Write, Data> {
    sink: Writer,
    _data: Data,
}

impl<'a> OutputStream<Stdout, StdoutLock<'a>> {
    /// An [`OutputStream`] for stdout, which should be used for all [`Workflow`] output.
    ///
    /// Implicitly acquires [`StdoutLock`] since the whole application is single threaded.
    pub fn new_stdout() -> Self {
        let sink = stdout();
        let _data = sink.lock();
        Self { sink, _data }
    }
}

#[cfg(test)]
impl OutputStream<Vec<u8>, ()> {
    /// An [`OutputStream`] backed by memory.
    pub fn new_vec() -> Self {
        Self {
            sink: Vec::new(),
            _data: (),
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.sink.as_slice()).expect("tests should have utf8 output")
    }
}

#[cfg(test)]
impl OutputStream<std::io::Sink, ()> {
    /// An [`OutputStream`] that throws away all writes.
    pub fn new_sink() -> Self {
        Self {
            sink: std::io::sink(),
            _data: (),
        }
    }
}

impl<Writer: Write, Data> Write for OutputStream<Writer, Data> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}
