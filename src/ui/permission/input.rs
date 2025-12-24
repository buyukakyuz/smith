use crossterm::ExecutableCommand;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use std::io::{self, Write, stdout};

use crate::permission::types::{PermissionResponse, PermissionType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Selection {
    AllowOnce,
    AllowSession,
    Feedback,
}

impl Selection {
    pub const fn index(self) -> usize {
        match self {
            Self::AllowOnce => 0,
            Self::AllowSession => 1,
            Self::Feedback => 2,
        }
    }

    pub const fn prev(self) -> Self {
        match self {
            Self::AllowOnce | Self::AllowSession => Self::AllowOnce,
            Self::Feedback => Self::AllowSession,
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::AllowOnce => Self::AllowSession,
            Self::AllowSession | Self::Feedback => Self::Feedback,
        }
    }

    pub fn into_response(self) -> PermissionResponse {
        match self {
            Self::AllowOnce => PermissionResponse::AllowOnce,
            Self::AllowSession => PermissionResponse::AllowSession,
            Self::Feedback => get_feedback_input(),
        }
    }
}

pub enum KeyAction {
    Navigate(Selection),
    Confirm(PermissionResponse),
    None,
}

pub fn handle_key(key: KeyEvent, current: Selection) -> KeyAction {
    match (key.code, key.modifiers) {
        (KeyCode::Up | KeyCode::Char('k'), _) => KeyAction::Navigate(current.prev()),
        (KeyCode::Down | KeyCode::Char('j'), _) => KeyAction::Navigate(current.next()),

        (KeyCode::Char('1'), _) => KeyAction::Confirm(PermissionResponse::AllowOnce),
        (KeyCode::Char('2'), _) | (KeyCode::Tab, KeyModifiers::SHIFT) => {
            KeyAction::Confirm(PermissionResponse::AllowSession)
        }
        (KeyCode::Char('3'), _) => KeyAction::Confirm(get_feedback_input()),

        (KeyCode::Enter, _) => KeyAction::Confirm(current.into_response()),

        (KeyCode::Esc, _) => KeyAction::Confirm(cancelled_response()),

        _ => KeyAction::None,
    }
}

pub const fn get_action_verb(op: PermissionType) -> &'static str {
    match op {
        PermissionType::FileWrite => "create",
        PermissionType::FileRead => "read",
        PermissionType::FileDelete => "delete",
        PermissionType::CommandExecute => "execute",
        PermissionType::NetworkAccess => "connect to",
        PermissionType::SystemModification => "modify",
    }
}

fn get_feedback_input() -> PermissionResponse {
    let _ = disable_raw_mode();
    let _ = stdout().execute(LeaveAlternateScreen);

    println!("\nTell the model what to do instead:");
    print!("> ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) if !input.trim().is_empty() => {
            PermissionResponse::TellModelDifferently(input.trim().to_string())
        }
        _ => cancelled_response(),
    }
}

fn cancelled_response() -> PermissionResponse {
    PermissionResponse::TellModelDifferently(
        "User cancelled this operation. Please ask what to do instead.".to_string(),
    )
}
