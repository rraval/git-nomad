use std::io::{stdout, Stdout, StdoutLock, Write};

/// An abstraction point around a [`Write`] implementation. Allows redirecting output for test
/// assertions.
pub struct OutputStream<Sink: Write, Data> {
    sink: Sink,
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
}

impl<Sink: Write, Data> Write for OutputStream<Sink, Data> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sink.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}
