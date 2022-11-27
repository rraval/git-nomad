use std::io::{stdout, Stdout, StdoutLock, Write};

/// An abstraction point around a [`Write`] implementation. Allows redirecting output for test
/// assertions.
pub struct OutputStream<Writer: Write, Data> {
    writer: Writer,
    _data: Data,
}

impl<'a> OutputStream<Stdout, StdoutLock<'a>> {
    /// An [`OutputStream`] for stdout, which should be used for all [`crate::workflow::Workflow`] output.
    ///
    /// Implicitly acquires [`StdoutLock`] since the whole application is single threaded.
    pub fn new_stdout() -> Self {
        let writer = stdout();
        let _data = writer.lock();
        Self { writer, _data }
    }
}

#[cfg(test)]
impl OutputStream<Vec<u8>, ()> {
    /// An [`OutputStream`] backed by memory.
    pub fn new_vec() -> Self {
        Self {
            writer: Vec::new(),
            _data: (),
        }
    }

    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.writer.as_slice()).expect("tests should have utf8 output")
    }
}

#[cfg(test)]
impl OutputStream<std::io::Sink, ()> {
    /// An [`OutputStream`] that throws away all writes.
    pub fn new_sink() -> Self {
        Self {
            writer: std::io::sink(),
            _data: (),
        }
    }
}

impl<Writer: Write, Data> Write for OutputStream<Writer, Data> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.writer.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod test {
    use super::OutputStream;
    use std::io::Write;

    /// This test makes no assertions, merely getting a handle and locking it is sufficient.
    #[test]
    fn stdout_stream() {
        OutputStream::new_stdout();
    }

    #[test]
    fn vec_stream() {
        let mut stream = OutputStream::new_vec();
        write!(stream, "foo").unwrap();
        assert_eq!(stream.as_str(), "foo");

        // Flushing should be a no-op
        stream.flush().unwrap();
        assert_eq!(stream.as_str(), "foo");
    }

    #[test]
    fn sink_stream() {
        let mut stream = OutputStream::new_sink();
        write!(stream, "foo").unwrap();
    }
}
