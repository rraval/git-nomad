use anyhow::Result;
use console::Term;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{borrow::Cow, io::Write, time::Duration};

pub trait Renderer {
    fn out<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T>;

    fn err<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T>;

    fn spinner<T>(
        &mut self,
        description: impl Into<Cow<'static, str>>,
        func: impl FnOnce() -> Result<T>,
    ) -> Result<T>;
}

pub struct TerminalRenderer {
    stdout: Term,
    stderr: Term,
}

impl TerminalRenderer {
    pub fn new() -> Self {
        Self {
            stdout: Term::buffered_stdout(),
            stderr: Term::stderr(),
        }
    }
}

impl Renderer for TerminalRenderer {
    fn out<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
        let ret = func(&mut self.stdout)?;
        self.stdout.flush()?;
        Ok(ret)
    }

    fn err<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
        let ret = func(&mut self.stderr)?;
        self.stdout.flush()?;
        Ok(ret)
    }

    fn spinner<T>(
        &mut self,
        description: impl Into<Cow<'static, str>>,
        func: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let spinner =
            ProgressBar::with_draw_target(None, ProgressDrawTarget::term(self.stderr.clone(), 10));
        spinner.set_style(ProgressStyle::default_spinner());
        spinner.set_message(description);
        spinner.enable_steady_tick(Duration::from_millis(150));

        let ret = func();
        spinner.finish_and_clear();

        ret
    }
}

#[cfg(test)]
pub mod test_terminal {
    use anyhow::Context;

    use crate::renderer::{Renderer, TerminalRenderer};

    #[test]
    fn out() {
        let mut renderer = TerminalRenderer::new();
        renderer
            .out(|w| write!(w, "").context("write in test"))
            .unwrap();
    }

    #[test]
    fn err() {
        let mut renderer = TerminalRenderer::new();
        renderer
            .err(|w| write!(w, "").context("write in test"))
            .unwrap();
    }

    #[test]
    fn spinner() {
        let mut renderer = TerminalRenderer::new();
        let mut func_called = false;
        renderer
            .spinner("Spinning", || {
                func_called = true;
                Ok(())
            })
            .unwrap();
        assert!(func_called);
    }
}

#[cfg(test)]
pub mod test {
    use std::io::Write;
    use std::{borrow::Cow, io};

    use anyhow::{Context, Result};

    use super::Renderer;

    pub struct MemoryRenderer(Vec<u8>);

    impl MemoryRenderer {
        pub fn new() -> Self {
            Self(Vec::new())
        }

        pub fn as_str(&self) -> &str {
            std::str::from_utf8(self.0.as_slice()).expect("tests should have utf8 output")
        }
    }

    impl Renderer for MemoryRenderer {
        fn out<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
            func(&mut self.0)
        }

        fn err<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
            func(&mut self.0)
        }

        fn spinner<T>(
            &mut self,
            description: impl Into<Cow<'static, str>>,
            func: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            writeln!(self.0, "{}...", description.into())?;
            func()
        }
    }

    pub struct NoRenderer;

    impl Renderer for NoRenderer {
        fn out<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
            func(&mut io::sink())
        }

        fn err<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
            func(&mut io::sink())
        }

        fn spinner<T>(
            &mut self,
            _description: impl Into<Cow<'static, str>>,
            func: impl FnOnce() -> Result<T>,
        ) -> Result<T> {
            func()
        }
    }

    #[test]
    fn out_and_err_writes_to_same_buffer() {
        let mut renderer = MemoryRenderer::new();
        renderer
            .out(|w| writeln!(w, "out").context("write in test"))
            .unwrap();
        renderer
            .err(|w| writeln!(w, "err").context("write in test"))
            .unwrap();
        assert_eq!(renderer.as_str(), "out\nerr\n");
    }

    #[test]
    fn spinner() {
        let mut renderer = MemoryRenderer::new();
        let mut func_called = false;
        renderer
            .spinner("Spinning", || {
                func_called = true;
                Ok(())
            })
            .unwrap();

        assert_eq!(renderer.as_str(), "Spinning...\n");
        assert!(func_called);
    }
}
