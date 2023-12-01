use anyhow::Result;
use console::Term;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::{borrow::Cow, io::Write, time::Duration};

pub trait Renderer {
    fn writer<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T>;

    fn are_spinners_visible(&self) -> bool;

    fn spinner<T>(
        &mut self,
        description: impl Into<Cow<'static, str>>,
        func: impl FnOnce() -> Result<T>,
    ) -> Result<T>;
}

pub struct TerminalRenderer(Term);

impl TerminalRenderer {
    pub fn stdout() -> Self {
        Self(Term::buffered_stdout())
    }
}

impl Renderer for TerminalRenderer {
    fn writer<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
        let ret = func(&mut self.0)?;
        self.0.flush()?;
        Ok(ret)
    }

    fn are_spinners_visible(&self) -> bool {
        self.0.is_term()
    }

    fn spinner<T>(
        &mut self,
        description: impl Into<Cow<'static, str>>,
        func: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        let spinner =
            ProgressBar::with_draw_target(None, ProgressDrawTarget::term(self.0.clone(), 10));
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&[" ..", ". .", ".. ", "..."])
                .template("{msg}{spinner} {elapsed}")
                .unwrap(),
        );
        spinner.set_message(description);
        spinner.enable_steady_tick(Duration::from_millis(150));

        let ret = func();
        spinner.finish();

        // The finish call merely redraws the progress bar in its final state. The line needs to be
        // explicitly terminated.
        add_newline_if_spinners_are_visible(self)?;

        ret
    }
}

/// Adds a newline to separate output from spinners, but that's only necessary if spinners are even
/// being displayed.
pub fn add_newline_if_spinners_are_visible(renderer: &mut impl Renderer) -> Result<()> {
    if renderer.are_spinners_visible() {
        renderer.writer(|w| {
            writeln!(w)?;
            Ok(())
        })?;
    }

    Ok(())
}

#[cfg(test)]
pub mod test_terminal {
    use anyhow::Context;

    use crate::renderer::{Renderer, TerminalRenderer};

    #[test]
    fn writer() {
        let mut renderer = TerminalRenderer::stdout();
        renderer
            .writer(|w| write!(w, "").context("write in test"))
            .unwrap();
    }

    #[test]
    fn are_spinners_visible() {
        TerminalRenderer::stdout().are_spinners_visible();
    }

    #[test]
    fn spinner() {
        let mut renderer = TerminalRenderer::stdout();
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

    use super::{add_newline_if_spinners_are_visible, Renderer};

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
        fn writer<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
            func(&mut self.0)
        }

        fn are_spinners_visible(&self) -> bool {
            true
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
        fn writer<T>(&mut self, func: impl FnOnce(&mut dyn Write) -> Result<T>) -> Result<T> {
            func(&mut io::sink())
        }

        fn are_spinners_visible(&self) -> bool {
            false
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
    fn writer() {
        let mut renderer = MemoryRenderer::new();
        renderer
            .writer(|w| writeln!(w, "foo").context("write in test"))
            .unwrap();
        assert_eq!(renderer.as_str(), "foo\n");
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

    #[test]
    fn add_newline() {
        let mut renderer = MemoryRenderer::new();
        add_newline_if_spinners_are_visible(&mut renderer).unwrap();
        assert_eq!(renderer.as_str(), "\n");
    }
}
