//! VTE parser integration for ANSI escape sequence handling

use crate::cell::{CellAttributes, Color};
use vte::{Params, Perform};

/// Actions to perform on the terminal, collected during parsing
#[derive(Debug, Clone)]
pub enum TerminalAction {
    Print(char),
    Bell,
    Backspace,
    Tab,
    LineFeed,
    CarriageReturn,
    CursorUp(usize),
    CursorDown(usize),
    CursorForward(usize),
    CursorBack(usize),
    SetCursor(usize, usize),
    EraseDisplay(u16),
    EraseLine(u16),
    SetTitle(String),
    SetAttributes(CellAttributes),
}

/// Parser performer that collects actions
pub struct ActionCollector {
    pub actions: Vec<TerminalAction>,
    current_attrs: CellAttributes,
}

impl ActionCollector {
    pub fn new(current_attrs: CellAttributes) -> Self {
        Self {
            actions: Vec::new(),
            current_attrs,
        }
    }
}

impl Perform for ActionCollector {
    fn print(&mut self, c: char) {
        self.actions.push(TerminalAction::Print(c));
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.actions.push(TerminalAction::Bell),
            0x08 => self.actions.push(TerminalAction::Backspace),
            0x09 => self.actions.push(TerminalAction::Tab),
            0x0A | 0x0B | 0x0C => self.actions.push(TerminalAction::LineFeed),
            0x0D => self.actions.push(TerminalAction::CarriageReturn),
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: char) {}
    fn put(&mut self, _byte: u8) {}
    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if params.is_empty() {
            return;
        }
        match params[0] {
            b"0" | b"2" => {
                if params.len() > 1 {
                    if let Ok(title) = std::str::from_utf8(params[1]) {
                        self.actions.push(TerminalAction::SetTitle(title.to_string()));
                    }
                }
            }
            _ => {}
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: char) {
        let params: Vec<u16> = params.iter().flat_map(|p| p.iter().copied()).collect();

        match action {
            'A' => self.actions.push(TerminalAction::CursorUp(
                params.first().copied().unwrap_or(1) as usize,
            )),
            'B' => self.actions.push(TerminalAction::CursorDown(
                params.first().copied().unwrap_or(1) as usize,
            )),
            'C' => self.actions.push(TerminalAction::CursorForward(
                params.first().copied().unwrap_or(1) as usize,
            )),
            'D' => self.actions.push(TerminalAction::CursorBack(
                params.first().copied().unwrap_or(1) as usize,
            )),
            'H' | 'f' => {
                let row = params.first().copied().unwrap_or(1).saturating_sub(1) as usize;
                let col = params.get(1).copied().unwrap_or(1).saturating_sub(1) as usize;
                self.actions.push(TerminalAction::SetCursor(row, col));
            }
            'J' => {
                let mode = params.first().copied().unwrap_or(0);
                self.actions.push(TerminalAction::EraseDisplay(mode));
            }
            'K' => {
                let mode = params.first().copied().unwrap_or(0);
                self.actions.push(TerminalAction::EraseLine(mode));
            }
            'm' => {
                self.handle_sgr(&params);
                self.actions
                    .push(TerminalAction::SetAttributes(self.current_attrs));
            }
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

impl ActionCollector {
    fn handle_sgr(&mut self, params: &[u16]) {
        let mut i = 0;

        while i < params.len() {
            match params[i] {
                0 => self.current_attrs = CellAttributes::default(),
                1 => self.current_attrs.bold = true,
                2 => self.current_attrs.dim = true,
                3 => self.current_attrs.italic = true,
                4 => self.current_attrs.underline = true,
                5 | 6 => self.current_attrs.blink = true,
                7 => self.current_attrs.inverse = true,
                8 => self.current_attrs.hidden = true,
                9 => self.current_attrs.strikethrough = true,
                22 => {
                    self.current_attrs.bold = false;
                    self.current_attrs.dim = false;
                }
                23 => self.current_attrs.italic = false,
                24 => self.current_attrs.underline = false,
                25 => self.current_attrs.blink = false,
                27 => self.current_attrs.inverse = false,
                28 => self.current_attrs.hidden = false,
                29 => self.current_attrs.strikethrough = false,
                30..=37 => {
                    self.current_attrs.foreground = Color::Named((params[i] - 30) as u8)
                }
                38 => {
                    if let Some(color) = self.parse_color(&params[i..]) {
                        self.current_attrs.foreground = color;
                        i += if params.get(i + 1) == Some(&2) { 4 } else { 2 };
                    }
                }
                39 => self.current_attrs.foreground = Color::Default,
                40..=47 => {
                    self.current_attrs.background = Color::Named((params[i] - 40) as u8)
                }
                48 => {
                    if let Some(color) = self.parse_color(&params[i..]) {
                        self.current_attrs.background = color;
                        i += if params.get(i + 1) == Some(&2) { 4 } else { 2 };
                    }
                }
                49 => self.current_attrs.background = Color::Default,
                90..=97 => {
                    self.current_attrs.foreground = Color::Named((params[i] - 90 + 8) as u8)
                }
                100..=107 => {
                    self.current_attrs.background = Color::Named((params[i] - 100 + 8) as u8)
                }
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_color(&self, params: &[u16]) -> Option<Color> {
        match params.get(1)? {
            2 => {
                let r = *params.get(2)? as u8;
                let g = *params.get(3)? as u8;
                let b = *params.get(4)? as u8;
                Some(Color::Rgb(r, g, b))
            }
            5 => {
                let idx = *params.get(2)? as u8;
                Some(Color::Indexed(idx))
            }
            _ => None,
        }
    }
}
