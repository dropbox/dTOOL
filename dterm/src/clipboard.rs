use anyhow::Result;

pub trait ClipboardProvider {
    fn set_text(&mut self, text: &str) -> Result<()>;
}

pub struct SystemClipboard {
    inner: arboard::Clipboard,
}

impl SystemClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self {
            inner: arboard::Clipboard::new()?,
        })
    }
}

impl ClipboardProvider for SystemClipboard {
    fn set_text(&mut self, text: &str) -> Result<()> {
        self.inner.set_text(text.to_string())?;
        Ok(())
    }
}

pub struct NoopClipboard;

impl ClipboardProvider for NoopClipboard {
    fn set_text(&mut self, _text: &str) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[derive(Default)]
pub struct TestClipboard {
    pub last_text: Option<String>,
    pub fail: bool,
}

#[cfg(test)]
impl ClipboardProvider for TestClipboard {
    fn set_text(&mut self, text: &str) -> Result<()> {
        if self.fail {
            anyhow::bail!("clipboard error");
        }
        self.last_text = Some(text.to_string());
        Ok(())
    }
}
