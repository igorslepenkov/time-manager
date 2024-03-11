use std::borrow::BorrowMut;

use crossterm::event::{self, KeyCode};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Stylize, Terminal},
    style::Style,
    symbols,
    widgets::{Block, Borders, Padding, Paragraph, Tabs, Wrap},
    Frame,
};

use anyhow::Result;

use std::path::PathBuf;

use crate::{
    state::{self, DailyState},
    task::NotCompletedTask,
    ui::{tabs::Tab, AppStage, AppUiState, Control},
    utils::{calculate_total_working_hours, set_task_name_from_previous_tasks},
};

pub struct App {
    daily_state: DailyState,
    ui_state: AppUiState,
    should_quit: bool,
    state_file_path: PathBuf,
}

impl App {
    pub fn init(state_file_path: &PathBuf) -> App {
        let daily_state: DailyState = state::DailyState::init(state_file_path).unwrap();

        App {
            ui_state: AppUiState::init(),
            daily_state,
            should_quit: false,
            state_file_path: state_file_path.clone(),
        }
    }

    pub fn update(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(250))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    match key.code {
                        KeyCode::Esc => self.quit(),
                        KeyCode::Right => self.ui_state.switch_tabs_forward(),
                        KeyCode::Left => self.ui_state.switch_tabs_backward(),
                        KeyCode::Down => self.ui_state.switch_control_focus_forwards(),
                        // KeyCode::Up => self.ui_state.switch_control_focus_backwards(),
                        KeyCode::Up => {
                            let active_control = &self.ui_state.control_focused;

                            if let Some(ref control_mutex) = active_control {
                                let mut control = control_mutex.lock().unwrap();

                                let completed_tasks =
                                    self.daily_state.completed_tasks.lock().unwrap();

                                if !completed_tasks.is_empty() {
                                    if let Control::TaskNameInput(ref mut input_state) = *control {
                                        set_task_name_from_previous_tasks(
                                            input_state,
                                            completed_tasks,
                                        );

                                        return Ok(());
                                    }
                                }
                            }

                            self.ui_state.switch_control_focus_backwards()
                        }
                        KeyCode::Enter => self.submit(),
                        KeyCode::Backspace => {
                            if let Some(control_focused_mutex) = &self.ui_state.control_focused {
                                let input = &mut *control_focused_mutex.lock().unwrap();
                                match input {
                                    Control::TaskNameInput(control)
                                    | Control::EndCommentInput(control) => {
                                        control.borrow_mut().remove_last_char_from_input()
                                    }
                                    _ => {}
                                }
                            }
                        }
                        _ => {
                            if let KeyCode::Char(char) = key.code {
                                if let Some(control_focused_mutex) = &self.ui_state.control_focused
                                {
                                    let input = &mut *control_focused_mutex.lock().unwrap();
                                    match input {
                                        Control::TaskNameInput(control)
                                        | Control::EndCommentInput(control) => {
                                            control.borrow_mut().add_char_to_input(char)
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn submit(&mut self) {
        let ui_state = &mut self.ui_state;

        let control_focused_option = &ui_state.control_focused;

        if let Some(control_arc) = control_focused_option {
            let control = control_arc.lock().unwrap();
            let active_tab = ui_state.get_active_tab();

            if let Control::SubmitBtn(_) = *control {
                match active_tab {
                    Tab::Start => {
                        drop(control);

                        let task_name = self.get_task_name_input().unwrap();
                        let end_comment = self.get_end_comment_input();
                        let _ = self.execute_start_command(task_name, end_comment);

                        let current_stage = &mut self.ui_state.stage;
                        if let AppStage::Waiting = current_stage {
                            *current_stage = AppStage::Working
                        }
                    }
                    Tab::End => {
                        drop(control);

                        let end_comment = self.get_end_comment_input();
                        let _ = self.execute_end_command(end_comment);

                        let current_stage = &mut self.ui_state.stage;
                        if let AppStage::Working = current_stage {
                            *current_stage = AppStage::Waiting
                        }
                    }
                    Tab::Out => {
                        drop(control);

                        let end_comment = self.get_end_comment_input();
                        let _ = self.execute_pause_command(end_comment);

                        let current_stage = &mut self.ui_state.stage;
                        if let AppStage::Working = current_stage {
                            *current_stage = AppStage::Paused
                        }
                    }
                    Tab::ClearState => {
                        let _ = self.daily_state.clear_todays_state(&self.state_file_path);

                        ui_state.stage = AppStage::Waiting;
                    }
                    _ => {}
                }
            }
        }

        self.ui_state.clear_inputs_state();
    }

    fn get_task_name_input(&mut self) -> Result<String, String> {
        let task_name_input_lock = self.ui_state.task_name_input.lock();
        let task_name_guard = task_name_input_lock.unwrap();
        if let Control::TaskNameInput(state) = &*task_name_guard {
            let input = state.input.to_owned();

            if input.is_empty() {
                return Err("You should enter new task name!".to_string());
            }

            Ok(input)
        } else {
            unreachable!()
        }
    }

    fn get_end_comment_input(&mut self) -> Option<String> {
        let previous_task_comment_input_lock = self.ui_state.task_end_comment_input.lock();
        let end_comment_guard = previous_task_comment_input_lock.unwrap();
        let end_comment: Option<String> =
            if let Control::EndCommentInput(state) = &*end_comment_guard {
                let input = &state.input;

                if input.is_empty() {
                    None
                } else {
                    Some(input.to_owned())
                }
            } else {
                unreachable!()
            };

        drop(end_comment_guard);

        end_comment
    }

    pub fn ui(&self, frame: &mut Frame) {
        let area = frame.size();
        let current_task = &self.daily_state.current_task;
        let total_working_hours = calculate_total_working_hours(&self.daily_state);

        let app_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(15), Constraint::Percentage(85)])
            .split(area);

        let top_layouts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(app_layout[0]);

        let top_left_internal_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(top_layouts[0]);

        let top_right_internal_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(top_layouts[1]);

        let main_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(19), Constraint::Percentage(81)])
            .split(app_layout[1]);

        let main_top_sections = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
            .split(main_layouts[0]);

        frame.render_widget(
            Block::new()
                .border_set(symbols::border::THICK)
                .borders(Borders::all())
                .padding(Padding::new(1, 1, 1, 1)),
            main_layouts[1],
        );

        frame.render_widget(
            Paragraph::new(format!(
                "Started work at {}",
                &self.daily_state.start_time.format("%d/%m/%Y %H:%M")
            ))
            .wrap(Wrap { trim: true })
            .white()
            .on_blue(),
            top_left_internal_layouts[0],
        );

        frame.render_widget(
            Paragraph::new(format!(
                "You've been working for {0:.2} hours already",
                total_working_hours
            ))
            .wrap(Wrap { trim: true })
            .white()
            .on_blue(),
            top_right_internal_layouts[0],
        );

        frame.render_widget(
            Paragraph::new("Current task: ")
                .white()
                .on_blue()
                .wrap(Wrap { trim: true }),
            top_left_internal_layouts[1],
        );

        let current_task_name = if let Some(task) = current_task {
            task.name.to_owned()
        } else {
            match self.ui_state.stage {
                AppStage::Working => {
                    unreachable!()
                }
                AppStage::Waiting => AppStage::Waiting.to_string(),
                AppStage::Paused => AppStage::Paused.to_string(),
            }
        };

        frame.render_widget(
            Paragraph::new(current_task_name)
                .wrap(Wrap { trim: true })
                .white()
                .on_blue(),
            top_right_internal_layouts[1],
        );

        frame.render_widget(
            Tabs::new(self.ui_state.tabs.to_owned())
                .select(self.ui_state.get_active_tab_idx())
                .highlight_style(Style::default().yellow())
                .block(
                    Block::new()
                        .border_set(symbols::border::Set {
                            bottom_left: symbols::line::NORMAL.vertical_right,
                            bottom_right: symbols::line::NORMAL.vertical_left,
                            ..symbols::border::THICK
                        })
                        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                        .title("Tabs"),
                ),
            main_top_sections[0],
        );

        match self.ui_state.get_active_tab() {
            Tab::Home => self.ui_state.render_home_tab(frame, main_layouts[1]),
            Tab::Start => self.ui_state.render_start_tab(frame, main_layouts[1]),
            Tab::Out => self.ui_state.render_out_tab(frame, main_layouts[1]),
            Tab::End => self.ui_state.render_end_tab(frame, main_layouts[1]),
            Tab::ClearState => self.ui_state.render_clear_tab(frame, main_layouts[1]),
        }
    }

    pub fn run(mut self) -> Result<()> {
        let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stderr()))?;

        loop {
            // application render
            terminal.draw(|frame| {
                self.ui(frame);
            })?;

            // application update
            self.update()?;

            // application exit
            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    pub fn quit(&mut self) {
        self.should_quit = true;
    }

    fn execute_start_command(
        &mut self,
        new_task_name: String,
        previous_task_completion_message: Option<String>,
    ) -> Result<String, String> {
        let state = &mut self.daily_state;
        let state_file_path = &self.state_file_path;

        state.complete_current_task(previous_task_completion_message)?;

        let new_task = NotCompletedTask::start(new_task_name.to_string());

        state.current_task.replace(new_task);

        let _ = state.save(state_file_path);

        Ok(format!("Started new task. Current task: {}", new_task_name))
    }

    fn execute_pause_command(
        &mut self,
        previous_task_completion_message: Option<String>,
    ) -> Result<String, String> {
        let state = &mut self.daily_state;
        let state_file_path = &self.state_file_path;

        let complete_task_result = state.complete_current_task(previous_task_completion_message);

        state.current_task = None;

        complete_task_result?;

        let _ = state.save(state_file_path);

        Ok("Track is paused. Out of keyboard".to_string())
    }

    fn execute_end_command(
        &mut self,
        previous_task_completion_message: Option<String>,
    ) -> Result<String, String> {
        let state = &mut self.daily_state;
        let state_file_path = &self.state_file_path;

        state.complete_current_task(previous_task_completion_message)?;

        let save_result = state.save_state_as_xlsx();

        match save_result {
            Ok(file_path) => {
                let _ = state.clear_todays_state(state_file_path);

                Ok(format!("Work is ended. Generated log file {}", file_path))
            }
            Err(_) => Err("Could not save workbook".to_string()),
        }
    }
}
