use super::{AppEvent, POLL_TIMEOUT, SCROLL_DELTA, TICK_INTERVAL};
use crate::core::error::Result;
use crossterm::event::{self, Event as CrosstermEvent, MouseEventKind};
use tokio::sync::mpsc::UnboundedSender;

pub async fn terminal_event_loop(tx: UnboundedSender<AppEvent>) -> Result<()> {
    loop {
        if event::poll(POLL_TIMEOUT)? {
            let app_event = match event::read()? {
                CrosstermEvent::Key(key) => Some(AppEvent::Input(key)),
                CrosstermEvent::Paste(text) => Some(AppEvent::Paste(text)),
                CrosstermEvent::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                CrosstermEvent::Mouse(mouse) => match mouse.kind {
                    MouseEventKind::ScrollUp => Some(AppEvent::MouseScroll(-SCROLL_DELTA)),
                    MouseEventKind::ScrollDown => Some(AppEvent::MouseScroll(SCROLL_DELTA)),
                    _ => None,
                },
                _ => None,
            };

            if let Some(event) = app_event
                && tx.send(event).is_err()
            {
                break;
            }
        }
    }
    Ok(())
}

pub async fn tick_loop(tx: UnboundedSender<AppEvent>) {
    let mut interval = tokio::time::interval(TICK_INTERVAL);
    loop {
        interval.tick().await;
        if tx.send(AppEvent::Tick).is_err() {
            break;
        }
    }
}
