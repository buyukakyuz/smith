mod commands;
mod diff;

pub use commands::SLASH_COMMANDS;
mod render;
mod terminal;

use crate::config::{ConfigEvent, ConfigEventSender};
use crate::core::augmented_llm::AugmentedLLM;
use crate::core::error::Result;
use crate::tools::ToolType;
use crate::tui::agent_runner::{AgentCommand, AgentRunner};
use crate::tui::events::{AppEvent, terminal_event_loop, tick_loop};
use crate::tui::layout::calculate_layout;
use crate::tui::state::AppState;
use crate::tui::widgets::{ChatWidget, InputAction, InputWidget};
use crossterm::ExecutableCommand;
use crossterm::event::{KeyCode, KeyModifiers};
use crossterm::terminal::{LeaveAlternateScreen, disable_raw_mode};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;
use tokio::sync::mpsc;

use commands::{HELP_TEXT, SlashCommand};
use diff::DiffMetadata;
use render::{render_header, render_model_picker_modal, render_permission_modal, render_status};
use terminal::{restore_terminal, setup_terminal};

pub use terminal::TerminalGuard;

pub struct TuiApp {
    agent_cmd_tx: mpsc::UnboundedSender<AgentCommand>,
    provider_name: String,
    model_name: String,
    state: AppState,
    input_widget: InputWidget<'static>,
    event_rx: mpsc::UnboundedReceiver<AppEvent>,
    event_tx: mpsc::UnboundedSender<AppEvent>,
    config_event_tx: Option<ConfigEventSender>,
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    show_model_picker_on_start: bool,
}

impl TuiApp {
    pub(crate) fn with_event_channels(
        agent: AugmentedLLM,
        event_tx: mpsc::UnboundedSender<AppEvent>,
        event_rx: mpsc::UnboundedReceiver<AppEvent>,
        config_event_tx: Option<ConfigEventSender>,
        show_model_picker_on_start: bool,
    ) -> Result<Self> {
        let terminal = setup_terminal()?;

        let provider_name = agent.llm().name().to_string();
        let model_name = agent.llm().model().to_string();

        let (runner, agent_cmd_tx) = AgentRunner::new(agent, event_tx.clone());
        tokio::spawn(async move {
            runner.run().await;
        });

        Ok(Self {
            agent_cmd_tx,
            provider_name,
            model_name,
            state: AppState::new(),
            input_widget: InputWidget::new(),
            event_rx,
            event_tx,
            config_event_tx,
            terminal,
            show_model_picker_on_start,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let tx1 = self.event_tx.clone();
        let tx2 = self.event_tx.clone();

        tokio::spawn(async move {
            let _ = terminal_event_loop(tx1).await;
        });

        tokio::spawn(async move {
            tick_loop(tx2).await;
        });

        if self.show_model_picker_on_start {
            self.state.show_model_picker();
            self.show_model_picker_on_start = false;
        }

        while !self.state.should_quit {
            let is_processing = self.state.is_processing;
            let elapsed = self.state.elapsed();
            let spinner_frame = self.state.spinner_frame;

            let permission_modal = self.state.permission_modal.as_ref().map(|m| {
                (
                    m.request.clone(),
                    m.selected,
                    m.input_mode,
                    m.feedback_input.clone(),
                )
            });

            let model_picker_modal = self
                .state
                .model_picker_modal
                .as_ref()
                .map(|m| (m.models.clone(), m.selected, m.total_models));

            self.terminal.draw(|f| {
                let layout = calculate_layout(f.area());

                render_header(f, layout.header, &self.provider_name, &self.model_name);

                let messages = self.state.messages_with_streaming();
                let chat_widget = ChatWidget::new(&messages, &mut self.state.scroll, spinner_frame);
                chat_widget.render(layout.chat, f.buffer_mut());

                self.input_widget.render(layout.input, f);

                render_status(f, layout.status, is_processing, elapsed, spinner_frame);

                if let Some((request, selected, input_mode, feedback)) = &permission_modal {
                    render_permission_modal(f, f.area(), request, *selected, *input_mode, feedback);
                }

                if let Some((models, selected, total)) = &model_picker_modal {
                    use crate::tui::state::ModelPickerModal;
                    let modal = ModelPickerModal {
                        models: models.clone(),
                        selected: *selected,
                        total_models: *total,
                    };
                    render_model_picker_modal(f, f.area(), &modal);
                }
            })?;

            if let Some(event) = self.event_rx.recv().await {
                self.handle_event(event)?;
            }
        }

        let _ = self.agent_cmd_tx.send(AgentCommand::Shutdown);

        restore_terminal(&mut self.terminal)?;

        Ok(())
    }

    fn handle_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::Input(key) => {
                self.handle_key_input(key)?;
            }
            AppEvent::Paste(text) => {
                let action = self.input_widget.handle_paste(text);
                self.handle_input_action(action)?;
            }
            AppEvent::Resize(_w, _h) => {}
            AppEvent::MouseScroll(delta) => {
                if delta < 0 {
                    self.state.scroll_up((-delta) as usize);
                } else {
                    self.state.scroll_down(delta as usize);
                }
            }
            AppEvent::Tick => {
                self.state.tick();
            }
            AppEvent::LLMChunk(chunk) => {
                self.state.append_streaming(&chunk);
            }
            AppEvent::LLMComplete(_message) => {
                let text = self.state.finalize_streaming();
                if !text.is_empty() {
                    self.state.add_assistant_message(text);
                }
                self.state.stop_processing();
            }
            AppEvent::LLMError(error) => {
                self.state.finalize_streaming();
                self.state.add_system_message_with_level(
                    format!("Error: {error}"),
                    crate::tui::widgets::MessageLevel::Error,
                );
                self.state.stop_processing();
            }
            AppEvent::ToolStarted { name, input } => {
                self.state.start_tool(&name, input);
            }
            AppEvent::ToolCompleted { name, result } => {
                if (name == ToolType::WriteFile.name() || name == ToolType::UpdateFile.name())
                    && let Some(output) = result.output()
                {
                    if let Some(metadata) = DiffMetadata::extract(output) {
                        let _ = self.event_tx.send(AppEvent::FileDiff {
                            path: metadata.path,
                            old_content: metadata.old_content,
                            new_content: metadata.new_content,
                        });
                    }
                }
                self.state.complete_tool(&name, result);
            }
            AppEvent::ToolFailed { name, error } => {
                self.state.fail_tool(&name, error);
            }
            AppEvent::PermissionRequired {
                request,
                response_tx,
            } => {
                self.state.show_permission_modal(request, response_tx);
            }
            AppEvent::FileDiff {
                path,
                old_content,
                new_content,
            } => {
                self.state.add_file_diff(path, old_content, new_content);
            }
            AppEvent::ModelChanged { provider, model } => {
                self.provider_name = provider.clone();
                self.model_name = model.clone();
                self.state
                    .add_system_message(format!("Switched to {provider}/{model}"));

                if let Some(ref tx) = self.config_event_tx {
                    let _ = tx.send(ConfigEvent::ModelChanged { provider, model });
                }
            }
            AppEvent::ModelSwitchError(error) => {
                self.state.add_system_message_with_level(
                    format!("Failed to switch model: {error}"),
                    crate::tui::widgets::MessageLevel::Error,
                );
            }
        }

        Ok(())
    }

    fn handle_key_input(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.state.has_model_picker() {
                self.state.model_picker_cancel();
                return Ok(());
            }
            if self.state.has_modal() {
                self.state.permission_cancel();
                return Ok(());
            }
            if !self.input_widget.is_empty() {
                self.input_widget.clear();
                return Ok(());
            }
            self.state.quit();
            return Ok(());
        }

        if key.code == KeyCode::Char('d') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if self.input_widget.is_empty() && !self.state.has_modal() {
                self.state.quit();
            }
            return Ok(());
        }

        if self.state.has_model_picker() {
            return self.handle_model_picker_input(key);
        }

        if self.state.has_modal() {
            return self.handle_modal_input(key);
        }

        if key.code == KeyCode::Char('l') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.state.clear_messages();
            return Ok(());
        }

        match key.code {
            KeyCode::PageUp => {
                self.state.scroll_up(10);
                return Ok(());
            }
            KeyCode::PageDown => {
                self.state.scroll_down(10);
                return Ok(());
            }
            KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.scroll_to_top();
                return Ok(());
            }
            KeyCode::End if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state.scroll_to_bottom();
                return Ok(());
            }
            _ => {}
        }

        let action = self.input_widget.handle_key(key);
        self.handle_input_action(action)?;

        Ok(())
    }

    fn handle_modal_input(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        if self.state.permission_in_input_mode() {
            match key.code {
                KeyCode::Enter => {
                    if let Some(feedback) = self.state.permission_confirm() {
                        self.state.add_system_message(format!("Denied: {feedback}"));
                    }
                }
                KeyCode::Esc => {
                    self.state.permission_select_prev();
                }
                KeyCode::Backspace => {
                    self.state.permission_input_backspace();
                }
                KeyCode::Char(c) => {
                    self.state.permission_input_char(c);
                }
                KeyCode::Up | KeyCode::Down => {
                    if key.code == KeyCode::Up {
                        self.state.permission_select_prev();
                    }
                }
                _ => {}
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.permission_select_prev();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.permission_select_next();
            }
            KeyCode::Char('1') => {
                if let Some(modal) = &mut self.state.permission_modal {
                    modal.selected = 0;
                    modal.input_mode = false;
                }
                self.state.permission_confirm();
            }
            KeyCode::Char('2') => {
                if let Some(modal) = &mut self.state.permission_modal {
                    modal.selected = 1;
                    modal.input_mode = false;
                }
                self.state.permission_confirm();
            }
            KeyCode::Char('3') | KeyCode::Char('n') => {
                if let Some(modal) = &mut self.state.permission_modal {
                    modal.selected = 2;
                    modal.input_mode = true;
                }
            }
            KeyCode::Enter => {
                if let Some(feedback) = self.state.permission_confirm() {
                    self.state.add_system_message(format!("Denied: {feedback}"));
                }
            }
            KeyCode::Esc => {
                self.state.permission_cancel();
            }
            KeyCode::Char('y') => {
                if let Some(modal) = &mut self.state.permission_modal {
                    modal.selected = 0;
                    modal.input_mode = false;
                }
                self.state.permission_confirm();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_model_picker_input(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.state.model_picker_select_prev();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.state.model_picker_select_next();
            }
            KeyCode::Enter => {
                if let Some(model_name) = self.state.model_picker_confirm() {
                    let _ = self
                        .agent_cmd_tx
                        .send(AgentCommand::SwitchModel { model_name });
                }
            }
            KeyCode::Esc => {
                self.state.model_picker_cancel();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_input_action(&mut self, action: InputAction) -> Result<()> {
        match action {
            InputAction::Continue => {}

            InputAction::Submit(text) => {
                if text.starts_with('/') {
                    self.handle_slash_command(&text);
                } else {
                    self.state.add_to_history(text.clone());
                    self.state.add_user_message(text.clone());
                    self.state.start_processing();

                    let _ = self
                        .agent_cmd_tx
                        .send(AgentCommand::Run { user_message: text });
                }
            }

            InputAction::HistoryPrev => {
                if let Some(text) = self.state.history_prev() {
                    self.input_widget.set_text(&text);
                }
            }

            InputAction::HistoryNext => {
                if let Some(text) = self.state.history_next() {
                    self.input_widget.set_text(&text);
                } else {
                    self.input_widget.clear();
                }
            }

            InputAction::Clear => {}
        }

        Ok(())
    }

    fn handle_slash_command(&mut self, command: &str) {
        match SlashCommand::parse(command) {
            SlashCommand::Help => {
                self.state.add_system_message(HELP_TEXT.to_string());
            }
            SlashCommand::Exit => {
                self.state.quit();
            }
            SlashCommand::Clear => {
                self.state.clear_messages();
                self.state
                    .add_system_message("Chat history cleared.".to_string());
            }
            SlashCommand::Model => {
                self.state.show_model_picker();
            }
            SlashCommand::NotImplemented(cmd) => {
                self.state
                    .add_system_message(format!("Command '{cmd}' is not yet implemented."));
            }
            SlashCommand::Unknown(cmd) => {
                self.state.add_system_message(format!(
                    "Unknown command: {cmd}. Type /help for available commands."
                ));
            }
        }
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        use crossterm::event::DisableBracketedPaste;
        let _ = self.terminal.backend_mut().execute(DisableBracketedPaste);
        let _ = disable_raw_mode();
        let _ = self.terminal.backend_mut().execute(LeaveAlternateScreen);
    }
}
