//! Form example with input validation.
//!
//! Demonstrates building a form with:
//! - Text input fields with placeholders
//! - Field validation with error messages
//! - Tab navigation between fields
//! - Submit button
//!
//! Controls:
//! - Tab / Shift+Tab: Navigate between fields
//! - Enter: Submit form (when on submit button)
//! - Type to edit focused field
//! - q / Escape: Quit

use inky::prelude::*;

/// Form field definition
struct FormField {
    label: &'static str,
    value: Signal<String>,
    placeholder: &'static str,
    error: Signal<Option<String>>,
    validator: fn(&str) -> Option<String>,
}

impl FormField {
    fn new(
        label: &'static str,
        placeholder: &'static str,
        validator: fn(&str) -> Option<String>,
    ) -> Self {
        Self {
            label,
            value: use_signal(String::new()),
            placeholder,
            error: use_signal(None),
            validator,
        }
    }

    fn validate(&self) -> bool {
        let value = self.value.get();
        let error = (self.validator)(&value);
        self.error.set(error.clone());
        error.is_none()
    }
}

/// Form state
struct FormState {
    fields: Vec<FormField>,
    focused_field: Signal<usize>,
    submitted: Signal<bool>,
    submit_error: Signal<Option<String>>,
}

// Validation functions
fn validate_name(value: &str) -> Option<String> {
    if value.is_empty() {
        Some("Name is required".to_string())
    } else if value.len() < 2 {
        Some("Name must be at least 2 characters".to_string())
    } else {
        None
    }
}

fn validate_email(value: &str) -> Option<String> {
    if value.is_empty() {
        Some("Email is required".to_string())
    } else if !value.contains('@') || !value.contains('.') {
        Some("Please enter a valid email address".to_string())
    } else {
        None
    }
}

fn validate_password(value: &str) -> Option<String> {
    if value.is_empty() {
        Some("Password is required".to_string())
    } else if value.len() < 8 {
        Some("Password must be at least 8 characters".to_string())
    } else if !value.chars().any(|c| c.is_ascii_digit()) {
        Some("Password must contain at least one number".to_string())
    } else {
        None
    }
}

fn validate_confirm(_value: &str) -> Option<String> {
    // This is validated against password in submit
    None
}

impl FormState {
    fn new() -> Self {
        Self {
            fields: vec![
                FormField::new("Name", "Enter your name", validate_name),
                FormField::new("Email", "user@example.com", validate_email),
                FormField::new("Password", "Enter password", validate_password),
                FormField::new("Confirm Password", "Confirm password", validate_confirm),
            ],
            focused_field: use_signal(0),
            submitted: use_signal(false),
            submit_error: use_signal(None),
        }
    }

    fn focus_next(&self) {
        let max = self.fields.len(); // Include submit button
        self.focused_field.update(|i| {
            *i = (*i + 1) % (max + 1);
        });
    }

    fn focus_prev(&self) {
        let max = self.fields.len();
        self.focused_field.update(|i| {
            if *i == 0 {
                *i = max;
            } else {
                *i -= 1;
            }
        });
    }

    fn type_char(&self, c: char) {
        let focused = self.focused_field.get();
        if focused < self.fields.len() {
            self.fields[focused].value.update(|s| s.push(c));
            // Clear error on edit
            self.fields[focused].error.set(None);
        }
    }

    fn backspace(&self) {
        let focused = self.focused_field.get();
        if focused < self.fields.len() {
            self.fields[focused].value.update(|s| {
                s.pop();
            });
        }
    }

    fn submit(&self) -> bool {
        // Validate all fields
        let mut all_valid = true;
        for field in &self.fields {
            if !field.validate() {
                all_valid = false;
            }
        }

        // Check password confirmation
        let password = self.fields[2].value.get();
        let confirm = self.fields[3].value.get();
        if password != confirm {
            self.fields[3]
                .error
                .set(Some("Passwords do not match".to_string()));
            all_valid = false;
        }

        if all_valid {
            self.submitted.set(true);
            self.submit_error.set(None);
            true
        } else {
            self.submit_error
                .set(Some("Please fix the errors above".to_string()));
            false
        }
    }
}

/// Build a form field row
fn build_field_row(field: &FormField, focused: bool, is_password: bool) -> BoxNode {
    let value = field.value.get();
    let error = field.error.get();

    // Determine display value
    let display_value = if value.is_empty() {
        field.placeholder.to_string()
    } else if is_password {
        "*".repeat(value.len())
    } else {
        value.clone()
    };

    let value_color = if value.is_empty() {
        Color::BrightBlack
    } else {
        Color::White
    };

    let border_style = if focused {
        BorderStyle::Bold
    } else {
        BorderStyle::Single
    };

    let mut row = BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .margin_xy(0.0, 1.0);

    // Label
    row = row.child(TextNode::new(field.label).color(if focused {
        Color::BrightCyan
    } else {
        Color::White
    }));

    // Input field
    let input = BoxNode::new()
        .width(40)
        .height(3)
        .border(border_style)
        .padding_xy(1.0, 0.0)
        .flex_direction(FlexDirection::Column)
        .justify_content(JustifyContent::Center)
        .child(
            TextNode::new(format!(
                "{}{}",
                display_value,
                if focused { "▎" } else { "" }
            ))
            .color(value_color),
        );

    row = row.child(input);

    // Error message
    if let Some(err) = error {
        row = row.child(TextNode::new(format!("⚠ {}", err)).color(Color::BrightRed));
    }

    row
}

/// Build submit button
fn build_submit_button(focused: bool) -> BoxNode {
    let border = if focused {
        BorderStyle::Bold
    } else {
        BorderStyle::Single
    };

    let color = if focused {
        Color::BrightGreen
    } else {
        Color::Green
    };

    BoxNode::new()
        .width(20)
        .height(3)
        .border(border)
        .justify_content(JustifyContent::Center)
        .align_items(AlignItems::Center)
        .child(TextNode::new("Submit").bold().color(color))
}

/// Build success message
fn build_success_message(name: &str, email: &str) -> BoxNode {
    BoxNode::new()
        .flex_direction(FlexDirection::Column)
        .align_items(AlignItems::Center)
        .padding(2)
        .border(BorderStyle::Rounded)
        .child(
            TextNode::new("✓ Registration Successful!")
                .bold()
                .color(Color::BrightGreen),
        )
        .child(TextNode::new(""))
        .child(TextNode::new(format!("Welcome, {}!", name)).color(Color::White))
        .child(TextNode::new(format!("Confirmation sent to: {}", email)).color(Color::BrightBlack))
        .child(TextNode::new(""))
        .child(TextNode::new("Press any key to exit").color(Color::BrightBlack))
}

fn main() -> Result<()> {
    let state = FormState::new();

    App::new()
        .state(state)
        .alt_screen(true)
        .render(|ctx| {
            let focused = ctx.state.focused_field.get();
            let submitted = ctx.state.submitted.get();
            let submit_error = ctx.state.submit_error.get();

            if submitted {
                // Show success message
                let name = ctx.state.fields[0].value.get();
                let email = ctx.state.fields[1].value.get();

                return BoxNode::new()
                    .width(ctx.width())
                    .height(ctx.height())
                    .justify_content(JustifyContent::Center)
                    .align_items(AlignItems::Center)
                    .child(build_success_message(&name, &email))
                    .into();
            }

            // Build form
            let mut form = BoxNode::new()
                .flex_direction(FlexDirection::Column)
                .padding(2)
                .border(BorderStyle::Rounded);

            // Title
            form = form.child(
                TextNode::new("Registration Form")
                    .bold()
                    .color(Color::BrightCyan),
            );
            form = form.child(TextNode::new(""));

            // Fields
            form = form.child(build_field_row(&ctx.state.fields[0], focused == 0, false));
            form = form.child(build_field_row(&ctx.state.fields[1], focused == 1, false));
            form = form.child(build_field_row(&ctx.state.fields[2], focused == 2, true));
            form = form.child(build_field_row(&ctx.state.fields[3], focused == 3, true));

            // Submit error
            if let Some(err) = submit_error {
                form = form.child(TextNode::new(format!("✗ {}", err)).color(Color::BrightRed));
                form = form.child(TextNode::new(""));
            }

            // Submit button
            form = form.child(build_submit_button(focused == 4));

            // Instructions
            form = form.child(TextNode::new(""));
            form = form.child(
                TextNode::new("Tab: next field | Shift+Tab: previous | Enter: submit | Esc: quit")
                    .color(Color::BrightBlack),
            );

            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .child(form)
                .into()
        })
        .on_key(|state, key| {
            // If already submitted, any key exits
            if state.submitted.get() {
                return true;
            }

            match key.code {
                KeyCode::Tab => {
                    state.focus_next();
                }
                KeyCode::BackTab => {
                    state.focus_prev();
                }
                KeyCode::Enter => {
                    let focused = state.focused_field.get();
                    if focused == state.fields.len() {
                        // On submit button
                        state.submit();
                    } else {
                        // Move to next field
                        state.focus_next();
                    }
                }
                KeyCode::Char(c) => {
                    state.type_char(c);
                }
                KeyCode::Backspace => {
                    state.backspace();
                }
                KeyCode::Esc => {
                    return true;
                }
                _ => {}
            }
            false
        })
        .run()?;

    Ok(())
}
