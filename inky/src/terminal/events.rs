//! Terminal event types.

/// A terminal event.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Keyboard input.
    Key(KeyEvent),
    /// Mouse input.
    Mouse(MouseEvent),
    /// Terminal was resized.
    Resize {
        /// New terminal width in columns.
        width: u16,
        /// New terminal height in rows.
        height: u16,
    },
    /// Focus gained.
    FocusGained,
    /// Focus lost.
    FocusLost,
    /// Paste event (bracketed paste).
    Paste(String),
}

impl From<crossterm::event::Event> for TerminalEvent {
    fn from(event: crossterm::event::Event) -> Self {
        use crossterm::event::Event;

        match event {
            Event::Key(key) => TerminalEvent::Key(KeyEvent::from(key)),
            Event::Mouse(mouse) => TerminalEvent::Mouse(MouseEvent::from(mouse)),
            Event::Resize(w, h) => TerminalEvent::Resize {
                width: w,
                height: h,
            },
            Event::FocusGained => TerminalEvent::FocusGained,
            Event::FocusLost => TerminalEvent::FocusLost,
            Event::Paste(s) => TerminalEvent::Paste(s),
        }
    }
}

/// Keyboard event.
#[derive(Debug, Clone)]
pub struct KeyEvent {
    /// Key code.
    pub code: KeyCode,
    /// Modifier keys held.
    pub modifiers: KeyModifiers,
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: KeyCode::from(event.code),
            modifiers: KeyModifiers::from(event.modifiers),
        }
    }
}

/// Key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key.
    Char(char),
    /// Backspace key.
    Backspace,
    /// Enter key.
    Enter,
    /// Left arrow.
    Left,
    /// Right arrow.
    Right,
    /// Up arrow.
    Up,
    /// Down arrow.
    Down,
    /// Home key.
    Home,
    /// End key.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Tab key.
    Tab,
    /// Back tab (Shift+Tab).
    BackTab,
    /// Delete key.
    Delete,
    /// Insert key.
    Insert,
    /// Escape key.
    Esc,
    /// Function key F1-F12.
    F(u8),
    /// Null (no key).
    Null,
}

impl From<crossterm::event::KeyCode> for KeyCode {
    fn from(code: crossterm::event::KeyCode) -> Self {
        use crossterm::event::KeyCode as CK;

        match code {
            CK::Char(c) => KeyCode::Char(c),
            CK::Backspace => KeyCode::Backspace,
            CK::Enter => KeyCode::Enter,
            CK::Left => KeyCode::Left,
            CK::Right => KeyCode::Right,
            CK::Up => KeyCode::Up,
            CK::Down => KeyCode::Down,
            CK::Home => KeyCode::Home,
            CK::End => KeyCode::End,
            CK::PageUp => KeyCode::PageUp,
            CK::PageDown => KeyCode::PageDown,
            CK::Tab => KeyCode::Tab,
            CK::BackTab => KeyCode::BackTab,
            CK::Delete => KeyCode::Delete,
            CK::Insert => KeyCode::Insert,
            CK::Esc => KeyCode::Esc,
            CK::F(n) => KeyCode::F(n),
            CK::Null => KeyCode::Null,
            _ => KeyCode::Null,
        }
    }
}

/// Key modifiers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KeyModifiers {
    /// Shift key held.
    pub shift: bool,
    /// Control key held.
    pub ctrl: bool,
    /// Alt key held.
    pub alt: bool,
    /// Super/Meta key held.
    pub super_key: bool,
}

impl KeyModifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        shift: false,
        ctrl: false,
        alt: false,
        super_key: false,
    };

    /// Shift modifier.
    pub const SHIFT: Self = Self {
        shift: true,
        ctrl: false,
        alt: false,
        super_key: false,
    };

    /// Control modifier.
    pub const CTRL: Self = Self {
        shift: false,
        ctrl: true,
        alt: false,
        super_key: false,
    };

    /// Alt modifier.
    pub const ALT: Self = Self {
        shift: false,
        ctrl: false,
        alt: true,
        super_key: false,
    };
}

impl From<crossterm::event::KeyModifiers> for KeyModifiers {
    fn from(mods: crossterm::event::KeyModifiers) -> Self {
        use crossterm::event::KeyModifiers as CKM;

        Self {
            shift: mods.contains(CKM::SHIFT),
            ctrl: mods.contains(CKM::CONTROL),
            alt: mods.contains(CKM::ALT),
            super_key: mods.contains(CKM::SUPER),
        }
    }
}

/// Mouse event.
#[derive(Debug, Clone)]
pub struct MouseEvent {
    /// Mouse button (if applicable).
    pub button: Option<MouseButton>,
    /// Mouse event kind.
    pub kind: MouseEventKind,
    /// X position.
    pub x: u16,
    /// Y position.
    pub y: u16,
    /// Modifiers held.
    pub modifiers: KeyModifiers,
}

impl From<crossterm::event::MouseEvent> for MouseEvent {
    fn from(event: crossterm::event::MouseEvent) -> Self {
        use crossterm::event::MouseEventKind as MEK;

        let (button, kind) = match event.kind {
            MEK::Down(b) => (Some(MouseButton::from(b)), MouseEventKind::Down),
            MEK::Up(b) => (Some(MouseButton::from(b)), MouseEventKind::Up),
            MEK::Drag(b) => (Some(MouseButton::from(b)), MouseEventKind::Drag),
            MEK::Moved => (None, MouseEventKind::Moved),
            MEK::ScrollDown => (None, MouseEventKind::ScrollDown),
            MEK::ScrollUp => (None, MouseEventKind::ScrollUp),
            MEK::ScrollLeft => (None, MouseEventKind::ScrollLeft),
            MEK::ScrollRight => (None, MouseEventKind::ScrollRight),
        };

        Self {
            button,
            kind,
            x: event.column,
            y: event.row,
            modifiers: KeyModifiers::from(event.modifiers),
        }
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button (scroll wheel click).
    Middle,
}

impl From<crossterm::event::MouseButton> for MouseButton {
    fn from(button: crossterm::event::MouseButton) -> Self {
        use crossterm::event::MouseButton as CMB;

        match button {
            CMB::Left => MouseButton::Left,
            CMB::Right => MouseButton::Right,
            CMB::Middle => MouseButton::Middle,
        }
    }
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEventKind {
    /// Mouse button pressed down.
    Down,
    /// Mouse button released.
    Up,
    /// Mouse moved while button held (dragging).
    Drag,
    /// Mouse moved without button held.
    Moved,
    /// Scroll wheel up.
    ScrollUp,
    /// Scroll wheel down.
    ScrollDown,
    /// Scroll wheel left (horizontal scroll).
    ScrollLeft,
    /// Scroll wheel right (horizontal scroll).
    ScrollRight,
}
