use anyhow::Result;
#[cfg(test)]
use std::collections::VecDeque;
use std::io::{self, Write};

pub trait Prompt {
    fn input(&mut self, message: &str) -> Result<String>;
    #[allow(dead_code)]
    fn confirm(&mut self, message: &str, default: bool) -> Result<bool>;
}

pub struct StdioPrompt;

impl Prompt for StdioPrompt {
    fn input(&mut self, message: &str) -> Result<String> {
        print!("{message} ");
        io::stdout().flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        Ok(buf.trim().to_string())
    }

    fn confirm(&mut self, message: &str, default: bool) -> Result<bool> {
        let hint = if default { "[Y/n]" } else { "[y/N]" };
        let response = self.input(&format!("{message} {hint}"))?;
        if response.is_empty() {
            return Ok(default);
        }
        let normalized = response.to_lowercase();
        Ok(matches!(normalized.as_str(), "y" | "yes"))
    }
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptedResponse {
    Input(String),
    Confirm(bool),
}

#[cfg(test)]
pub struct ScriptedPrompt {
    responses: VecDeque<ScriptedResponse>,
}

#[cfg(test)]
impl ScriptedPrompt {
    pub fn new(responses: Vec<ScriptedResponse>) -> Self {
        Self {
            responses: responses.into(),
        }
    }
}

#[cfg(test)]
impl Prompt for ScriptedPrompt {
    fn input(&mut self, _message: &str) -> Result<String> {
        match self.responses.pop_front() {
            Some(ScriptedResponse::Input(value)) => Ok(value),
            Some(ScriptedResponse::Confirm(value)) => Ok(if value { "y" } else { "n" }.to_string()),
            None => Ok(String::new()),
        }
    }

    fn confirm(&mut self, _message: &str, default: bool) -> Result<bool> {
        match self.responses.pop_front() {
            Some(ScriptedResponse::Confirm(value)) => Ok(value),
            Some(ScriptedResponse::Input(value)) => {
                let normalized = value.to_lowercase();
                Ok(matches!(normalized.as_str(), "y" | "yes"))
            }
            None => Ok(default),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scripted_prompt_confirm_uses_response() {
        let mut prompt = ScriptedPrompt::new(vec![ScriptedResponse::Confirm(true)]);
        assert!(prompt.confirm("Confirm?", false).unwrap());
    }
}
