use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Local, NaiveDate};
use crossterm::event::{self, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    prelude::{CrosstermBackend, Line, Span, Stylize, Terminal},
    style::Style,
    symbols,
    widgets::{Block, Borders, Padding, Paragraph, Tabs, Wrap},
    Frame,
};

use anyhow::Result;

use std::path::PathBuf;

use crate::{
    config::Config,
    i18n,
    state::{self, DailyState},
    tagging,
    task::NotCompletedTask,
    theme::{self, theme},
    ui::{tabs::Tab, AppStage, AppUiState, Control, DayRolloverView},
    utils::{
        calculate_total_working_hours, completed_hours, set_task_name_from_previous_tasks,
        suggested_close_time,
    },
};

use ort::Session;

const CONFIG_FILE_NAME: &str = "config.json";
const LOG_FILE_NAME: &str = "sc-time-manager.log";

/// Незакрытая вчерашняя смена: её видно до тех пор, пока пользователь не решит,
/// что с ней делать.
pub struct DayRollover {
    pub date: NaiveDate,
    pub finished_tasks: usize,
    pub finished_hours: f64,
    pub running_task: Option<String>,
    pub running_since: Option<DateTime<Local>>,
    pub close_at: DateTime<Local>,
    pub error: Option<String>,
}

/// Результат последнего действия — показывается строкой внизу экрана.
struct StatusMessage {
    text: String,
    is_error: bool,
}

pub struct App {
    daily_state: DailyState,
    ui_state: AppUiState,
    should_quit: bool,
    state_file_path: PathBuf,
    config_path: PathBuf,
    config: Config,
    pending_day: Option<DayRollover>,
    quit_armed: bool,
    status: Option<StatusMessage>,
    prediction_model_session: Session,
}

/// Что делать по нажатию на вкладке Theme. Вынесено отдельно, чтобы правило
/// стрелок можно было проверить тестами без запуска интерфейса.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ThemeKey {
    Quit,
    PreviousTab,
    NextTab,
    PreviousTheme,
    NextTheme,
    ResetTheme,
    ToggleLanguage,
    Ignored,
}

fn theme_key_action(code: KeyCode) -> ThemeKey {
    match code {
        KeyCode::Esc => ThemeKey::Quit,
        KeyCode::Tab => ThemeKey::NextTab,
        KeyCode::BackTab => ThemeKey::PreviousTab,
        KeyCode::Up => ThemeKey::PreviousTheme,
        KeyCode::Down => ThemeKey::NextTheme,
        KeyCode::Char('r') | KeyCode::Char('R') => ThemeKey::ResetTheme,
        KeyCode::Char('l') | KeyCode::Char('L') => ThemeKey::ToggleLanguage,
        _ => ThemeKey::Ignored,
    }
}

/// Вкладка по Alt+1…Alt+6.
fn tab_from_hotkey(code: KeyCode) -> Option<Tab> {
    match code {
        KeyCode::Char('1') => Some(Tab::Home),
        KeyCode::Char('2') => Some(Tab::Start),
        KeyCode::Char('3') => Some(Tab::Out),
        KeyCode::Char('4') => Some(Tab::End),
        KeyCode::Char('5') => Some(Tab::ClearState),
        KeyCode::Char('6') => Some(Tab::Theme),
        _ => None,
    }
}

fn clear_day_message(outcome: &state::ClearOutcome) -> String {
    let summary = format!(
        "{} · {}: {} ({})",
        i18n::t("Day cleared", "День сброшен"),
        i18n::t("finished tasks", "выполнено задач"),
        outcome.archived_tasks,
        i18n::duration_text(outcome.archived_seconds)
    );

    match &outcome.archive_path {
        Some(path) => format!(
            "{summary} · {}: {}",
            i18n::t("a copy is saved in", "копия сохранена в"),
            path.display()
        ),
        None => summary,
    }
}

/// Время последней записи состояния — приблизительно «когда я последний раз
/// что-то делал в приложении».
fn file_modified_at(path: &Path) -> Option<DateTime<Local>> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;

    Some(DateTime::<Local>::from(modified))
}

fn build_day_rollover(state: &DailyState, state_file_path: &Path) -> DayRollover {
    let running_task = state.current_task.as_ref().map(|task| task.name.to_owned());
    let running_since = state.current_task.as_ref().map(|task| task.dt_start);

    DayRollover {
        date: state.day(),
        finished_tasks: state
            .completed_tasks
            .lock()
            .map(|tasks| tasks.len())
            .unwrap_or(0),
        finished_hours: completed_hours(state),
        running_task,
        running_since,
        close_at: suggested_close_time(
            file_modified_at(state_file_path),
            running_since,
            state.day(),
        ),
        error: None,
    }
}

impl App {
    pub fn init(state_file_path: &PathBuf) -> App {
        let config_directory = state_file_path
            .parent()
            .map(|path| path.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let config_path = config_directory.join(CONFIG_FILE_NAME);
        let loaded_config = Config::load(&config_path);
        i18n::set_lang(i18n::Lang::from_code(&loaded_config.config.language));

        let loaded_themes = theme::load_themes(&config_directory);
        let configured_theme = &loaded_config.config.theme.preset;
        let theme_sources = loaded_themes.sources.clone();
        let theme_index = theme::find_preset_index(&loaded_themes.presets, configured_theme);
        let theme_missing_warning = match theme_index {
            None if !configured_theme.trim().is_empty() => Some(format!(
                "{} «{}». {}",
                i18n::t("Theme not found", "Тема не найдена"),
                configured_theme,
                i18n::t("Default is used", "используется Default")
            )),
            _ => None,
        };
        let theme_selected = theme_index.unwrap_or(0);

        if let Some(preset) = loaded_themes.presets.get(theme_selected) {
            theme::apply_colors(&preset.colors);
        }

        let loaded_state = state::DailyState::load(state_file_path);

        let mut warning = [
            loaded_config.warning,
            loaded_themes.warning,
            loaded_state.warning,
            theme_missing_warning,
        ]
        .into_iter()
        .flatten()
        .reduce(|accumulated, next| format!("{accumulated}; {next}"));

        let mut daily_state: DailyState = loaded_state.state;
        let today = Local::now().date_naive();
        let mut pending_day = None;

        if daily_state.is_from_previous_day(today) {
            if daily_state.has_any_data() {
                pending_day = Some(build_day_rollover(&daily_state, state_file_path));
            } else {
                // День пустой — терять нечего, просто переносим его на сегодня.
                daily_state.start_new_day();

                if let Err(message) = daily_state.save(state_file_path) {
                    let mut combined = warning.unwrap_or_default();

                    if !combined.is_empty() {
                        combined.push_str("; ");
                    }

                    combined.push_str(&message);
                    warning = Some(combined);
                }
            }
        }

        let mut ui_state = AppUiState::init(warning, loaded_themes.presets, theme_selected);
        ui_state.theme_sources = theme_sources;

        App {
            ui_state,
            daily_state,
            should_quit: false,
            state_file_path: state_file_path.clone(),
            config_path,
            config: loaded_config.config,
            pending_day,
            quit_armed: false,
            status: None,
            prediction_model_session: tagging::load_model(),
        }
    }
    pub fn update(&mut self) -> Result<()> {
        if event::poll(std::time::Duration::from_millis(250))? {
            if let event::Event::Key(key) = event::read()? {
                if key.kind == event::KeyEventKind::Press {
                    // Успешное сообщение живёт до следующего нажатия, ошибка —
                    // до следующего действия, чтобы её можно было прочитать.
                    if let Some(status) = &self.status {
                        if !status.is_error {
                            self.status = None;
                        }
                    }

                    // Любая клавиша, кроме Esc, отменяет предупреждение о выходе —
                    // в том числе на странице Config и по Alt-хоткеям.
                    if key.code != KeyCode::Esc {
                        self.quit_armed = false;
                    }

                    if self.pending_day.is_some() {
                        self.handle_day_rollover_key(key.code);

                        return Ok(());
                    }

                    if key.modifiers.contains(event::KeyModifiers::ALT) {
                        if let Some(tab) = tab_from_hotkey(key.code) {
                            let opens_config = tab == Tab::Theme;

                            self.ui_state.open_tab(tab);

                            if opens_config {
                                self.reload_themes();
                            }

                            return Ok(());
                        }
                    }

                    if self.ui_state.get_active_tab() == Tab::Theme {
                        self.handle_theme_key(key.code);

                        return Ok(());
                    }

                    match key.code {
                        KeyCode::Esc => self.request_quit(),
                        KeyCode::Tab => self.ui_state.switch_tabs_forward(),
                        KeyCode::BackTab => self.ui_state.switch_tabs_backward(),
                        KeyCode::Down => self.ui_state.switch_control_focus_forwards(),
                        KeyCode::Up => {
                            if self.pick_previous_task_name() {
                                self.sync_history_selection();
                            } else {
                                self.ui_state.switch_control_focus_backwards()
                            }
                        }
                        KeyCode::Enter => self.submit(),
                        KeyCode::Left => {
                            self.with_focused_input(|control| control.move_cursor_left())
                        }
                        KeyCode::Right => {
                            self.with_focused_input(|control| control.move_cursor_right())
                        }
                        KeyCode::Home => {
                            self.with_focused_input(|control| control.move_cursor_home())
                        }
                        KeyCode::End => {
                            self.with_focused_input(|control| control.move_cursor_end())
                        }
                        KeyCode::Delete => {
                            self.with_focused_input(|control| control.delete_forward());
                            self.sync_history_selection();
                        }
                        KeyCode::Backspace => {
                            let word = key.modifiers.contains(KeyModifiers::CONTROL);

                            self.with_focused_input(|control| {
                                if word {
                                    control.delete_word_backward()
                                } else {
                                    control.backspace()
                                }
                            });

                            self.sync_history_selection();
                        }
                        _ => {
                            if let KeyCode::Char(char) = key.code {
                                if key.modifiers.contains(KeyModifiers::CONTROL) {
                                    if char.eq_ignore_ascii_case(&'v') {
                                        self.paste_from_clipboard();
                                    }

                                    return Ok(());
                                }

                                self.with_focused_input(|control| control.insert_char(char));
                                self.sync_history_selection();
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_theme_key(&mut self, code: KeyCode) {
        match theme_key_action(code) {
            ThemeKey::Quit => self.request_quit(),
            ThemeKey::PreviousTab => self.ui_state.switch_tabs_backward(),
            ThemeKey::NextTab => self.ui_state.switch_tabs_forward(),
            ThemeKey::PreviousTheme => self.move_theme_selection(-1),
            ThemeKey::NextTheme => self.move_theme_selection(1),
            ThemeKey::ResetTheme => {
                self.ui_state.select_theme(0);
                self.persist_theme();
            }
            ThemeKey::ToggleLanguage => self.toggle_language(),
            ThemeKey::Ignored => {}
        }
    }

    fn move_theme_selection(&mut self, delta: isize) {
        if self.ui_state.themes.is_empty() {
            return;
        }

        let total = self.ui_state.themes.len() as isize;
        let next = (self.ui_state.theme_selected as isize + delta).rem_euclid(total) as usize;

        self.ui_state.select_theme(next);
        self.persist_theme();
    }

    /// Тема с вкладки Theme: сразу применяется к интерфейсу, а в config.json
    /// уходит только имя темы — сами цвета живут в theme.json.
    fn persist_theme(&mut self) {
        let Some(preset) = self.ui_state.selected_theme() else {
            return;
        };

        theme::apply_colors(&preset.colors);

        self.config.theme.preset = preset.name.clone();

        if let Err(error) = self.config.save(&self.config_path) {
            self.set_status(
                format!(
                    "{}: {error}",
                    i18n::t("Could not save the theme", "Не удалось сохранить тему")
                ),
                true,
            );
        }
    }

    /// Переключение языка интерфейса: применяется сразу и запоминается.
    fn toggle_language(&mut self) {
        let next = i18n::lang().next();
        i18n::set_lang(next);
        self.config.language = next.code().to_string();

        if let Err(error) = self.config.save(&self.config_path) {
            self.set_status(
                format!(
                    "{}: {error}",
                    i18n::t("Could not save settings", "Не удалось сохранить настройки")
                ),
                true,
            );
        }
    }

    /// Стрелка ↑ в поле названия задачи: подставить имя прошлой задачи.
    /// Возвращает true, если подстановка состоялась (тогда фокус не двигаем).
    fn pick_previous_task_name(&mut self) -> bool {
        let Some(control_mutex) = self.ui_state.control_focused.clone() else {
            return false;
        };

        let mut control = control_mutex.lock().unwrap();
        let Control::TaskNameInput(input_state) = &mut *control else {
            return false;
        };

        let completed_tasks = self.daily_state.completed_tasks.lock().unwrap();

        if completed_tasks.is_empty() {
            return false;
        }

        set_task_name_from_previous_tasks(input_state, completed_tasks);

        true
    }

    /// Подсветка в списке выполненных задач повторяет текст поля ввода:
    /// если в поле ровно имя завершённой задачи, её строка и подсвечивается.
    fn sync_history_selection(&mut self) {
        let input_text = {
            let control = self.ui_state.task_name_input.lock().unwrap();

            match &*control {
                Control::TaskNameInput(input) => input.input.to_owned(),
                _ => String::new(),
            }
        };

        let selection = {
            let completed_tasks = self.daily_state.completed_tasks.lock().unwrap();

            crate::utils::find_completed_task_index(&completed_tasks, &input_text)
        };

        self.ui_state.history_selection = selection;
    }

    /// Выполненные задачи за день: имя и длительность для списка на экране.
    fn completed_task_lines(&self) -> Vec<(String, i64)> {
        let completed_tasks = self.daily_state.completed_tasks.lock().unwrap();

        completed_tasks
            .iter()
            .map(|task| {
                (
                    task.name.to_owned(),
                    (task.dt_end - task.dt_start).num_seconds().max(0),
                )
            })
            .collect()
    }

    /// Esc не выходит сразу, если идёт задача: сначала предупреждение,
    /// иначе легко потерять смену и не заметить этого.
    fn request_quit(&mut self) {
        if self.daily_state.current_task.is_some() && !self.quit_armed {
            self.quit_armed = true;

            return;
        }

        self.quit();
    }

    fn handle_day_rollover_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => self.quit(),
            KeyCode::Enter => self.finish_previous_day(),
            KeyCode::Char('c') | KeyCode::Char('C') => self.pending_day = None,
            _ => {}
        }
    }

    /// Закрывает вчерашнюю смену, выгружает её в xlsx и начинает новый день.
    fn finish_previous_day(&mut self) {
        let Some(close_at) = self.pending_day.as_ref().map(|rollover| rollover.close_at) else {
            return;
        };

        if let Err(error) = self.daily_state.close_current_task_at(close_at) {
            if let Some(rollover) = self.pending_day.as_mut() {
                rollover.error = Some(format!(
                    "{}: {error}",
                    i18n::t("Could not close the task", "Не удалось закрыть задачу")
                ));
            }

            return;
        }

        let export_result = self
            .daily_state
            .save_state_as_xlsx(&self.prediction_model_session, &self.config);

        match export_result {
            Ok(file_path) => {
                self.daily_state.start_new_day();

                if let Err(error) = self.daily_state.save(&self.state_file_path) {
                    self.set_status(
                        format!(
                            "{}: {error}",
                            i18n::t(
                                "Could not save the new day",
                                "Не удалось сохранить новый день"
                            )
                        ),
                        true,
                    );
                } else {
                    self.set_status(
                        format!("{}: {file_path}", i18n::t("Saved", "Сохранено")),
                        false,
                    );
                }

                self.pending_day = None;
            }
            Err(error) => {
                if let Some(rollover) = self.pending_day.as_mut() {
                    rollover.error = Some(format!(
                        "{}: {error}",
                        i18n::t("Could not save the xlsx file", "Не удалось сохранить .xlsx")
                    ));
                }
            }
        }
    }

    /// Доступ к состоянию интерфейса — используется предпросмотром экранов.
    pub fn ui_state_mut(&mut self) -> &mut AppUiState {
        &mut self.ui_state
    }

    /// Применяет действие к полю ввода, если фокус сейчас на нём.
    fn with_focused_input(&mut self, action: impl FnOnce(&mut Control)) {
        let Some(control_mutex) = self.ui_state.control_focused.clone() else {
            return;
        };

        let mut control = control_mutex.lock().unwrap();

        if control.is_text_input() {
            action(&mut control);
        }
    }

    fn paste_from_clipboard(&mut self) {
        let Some(text) = crate::clipboard::read_clipboard() else {
            return;
        };

        self.with_focused_input(|control| control.insert_text(&text));
        self.sync_history_selection();
    }

    /// Enter отправляет форму с любого поля: переводить фокус на кнопку
    /// Submit больше не нужно.
    fn submit(&mut self) {
        match self.ui_state.get_active_tab() {
            Tab::Start => {
                let task_name = match self.get_task_name_input() {
                    Ok(task_name) => task_name,
                    Err(message) => {
                        // Пустое имя: показываем ошибку, введённое не теряем.
                        self.set_status(message, true);
                        self.ui_state.refocus_active_tab();

                        return;
                    }
                };

                let end_comment = self.get_end_comment_input();

                match self.execute_start_command(task_name, end_comment) {
                    Ok(message) => {
                        self.set_status(message, false);

                        if let AppStage::Waiting = self.ui_state.stage {
                            self.ui_state.stage = AppStage::Working;
                        }
                    }
                    Err(message) => self.set_status(message, true),
                }
            }
            Tab::End => {
                if !self.daily_state.has_any_data() {
                    self.set_status(
                        i18n::t(
                            "No tasks yet — nothing to export",
                            "Пока нет задач — выгружать нечего",
                        )
                        .to_string(),
                        false,
                    );
                    self.ui_state.refocus_active_tab();

                    return;
                }

                let end_comment = self.get_end_comment_input();

                match self.execute_end_command(end_comment) {
                    Ok(message) => {
                        self.set_status(message, false);
                        self.ui_state.stage = AppStage::Waiting;
                    }
                    Err(message) => self.set_status(message, true),
                }
            }
            Tab::Out => {
                let had_running_task = self.daily_state.current_task.is_some();
                let end_comment = self.get_end_comment_input();

                match self.execute_pause_command(end_comment) {
                    Ok(message) => {
                        self.set_status(message, false);

                        if had_running_task {
                            self.ui_state.stage = AppStage::Paused;
                        }
                    }
                    Err(message) => self.set_status(message, true),
                }
            }
            Tab::ClearState => match self.daily_state.clear_todays_state(&self.state_file_path) {
                Ok(outcome) => {
                    self.set_status(clear_day_message(&outcome), false);
                    self.ui_state.stage = AppStage::Waiting;
                }
                Err(message) => self.set_status(message, true),
            },
            _ => {}
        }

        self.ui_state.clear_inputs_state();
        self.ui_state.refocus_active_tab();
    }

    fn set_status(&mut self, text: String, is_error: bool) {
        self.log_status(&text, is_error);
        self.status = Some(StatusMessage { text, is_error });
    }

    /// Перечитывает папку `themes/`: темы могли добавить или поправить,
    /// пока приложение запущено.
    fn reload_themes(&mut self) {
        let Some(directory) = self.config_path.parent().map(|path| path.to_path_buf()) else {
            return;
        };

        let loaded = theme::load_themes(&directory);
        let warning = self.ui_state.reload_themes(loaded);

        if let Some(warning) = warning {
            self.set_status(warning, false);
        }
    }

    /// Дублирует сообщения в файл рядом с конфигом: в узком окне статус
    /// обрезается, и причину ошибки иначе не увидеть.
    fn log_status(&self, text: &str, is_error: bool) {
        let Some(directory) = self.config_path.parent() else {
            return;
        };

        let level = if is_error { "ERROR" } else { "INFO" };
        let line = format!(
            "{} {level} {text}\n",
            Local::now().format("%Y-%m-%d %H:%M:%S")
        );

        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join(LOG_FILE_NAME))
        {
            let _ = file.write_all(line.as_bytes());
        }
    }

    fn get_task_name_input(&mut self) -> Result<String, String> {
        let task_name_input_lock = self.ui_state.task_name_input.lock();
        let task_name_guard = task_name_input_lock.unwrap();
        if let Control::TaskNameInput(state) = &*task_name_guard {
            let input = state.input.to_owned();

            if input.is_empty() {
                return Err(
                    i18n::t("You should enter new task name!", "Введите название задачи")
                        .to_string(),
                );
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

        if area.width < crate::ui::MIN_WIDTH || area.height < crate::ui::MIN_HEIGHT {
            crate::ui::render_too_small(frame, area);

            return;
        }

        if let Some(rollover) = &self.pending_day {
            let view = DayRolloverView {
                date: rollover.date.format("%d.%m.%Y").to_string(),
                finished_tasks: rollover.finished_tasks,
                finished_hours: rollover.finished_hours,
                running_task: rollover.running_task.as_deref(),
                running_since: rollover
                    .running_since
                    .map(|time| time.format("%H:%M").to_string()),
                close_at: rollover.close_at.format("%H:%M").to_string(),
                error: rollover.error.as_deref(),
            };

            crate::ui::render_day_rollover(frame, area, &view);

            return;
        }

        let current_task = &self.daily_state.current_task;
        let total_working_hours = calculate_total_working_hours(&self.daily_state);
        let completed_tasks = self.completed_task_lines();
        let completed_seconds = self.daily_state.completed_seconds();
        let planned_file =
            crate::export::planned_file_path(&self.config, self.daily_state.start_time)
                .unwrap_or_default();
        let running_seconds = current_task
            .as_ref()
            .map(|task| (Local::now() - task.dt_start).num_seconds())
            .unwrap_or(0);

        frame.render_widget(
            Block::new().style(Style::default().bg(theme().background)),
            area,
        );

        let app_layout = Layout::default()
            .direction(Direction::Vertical)
            // Шапка в две строки, под ней тонкий разделитель.
            .constraints(vec![
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Min(1),
            ])
            .split(area);

        let top_layouts = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(app_layout[0]);

        let left_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Length(1)])
            .split(top_layouts[0]);

        let right_rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Length(1), Constraint::Length(1)])
            .split(top_layouts[1]);

        let main_layouts = Layout::default()
            .direction(Direction::Vertical)
            // Панель вкладок — ровно три строки: рамка, названия, пустая строка.
            .constraints(vec![Constraint::Length(3), Constraint::Min(1)])
            .split(app_layout[2]);

        frame.render_widget(
            Block::new()
                .border_set(symbols::border::THICK)
                .border_style(Style::new().fg(theme().border))
                .borders(Borders::all())
                .padding(Padding::new(1, 1, 1, 1)),
            main_layouts[1],
        );

        // Тонкая линия между шапкой и содержимым.
        frame.render_widget(
            Paragraph::new(Span::styled(
                "─".repeat(area.width as usize),
                Style::new().fg(theme().border).bg(theme().background),
            )),
            app_layout[1],
        );

        let label_style = Style::new().fg(theme().panel_text).bg(theme().panel);
        let value_style = Style::new().fg(theme().accent).bg(theme().panel).bold();

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{} ", i18n::t("Started work at", "День начат:")),
                    label_style,
                ),
                Span::styled(
                    self.daily_state
                        .start_time
                        .format("%d.%m.%Y %H:%M")
                        .to_string(),
                    value_style,
                ),
            ]))
            .style(Style::new().bg(theme().panel))
            .wrap(Wrap { trim: true }),
            left_rows[0],
        );

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("{} ", i18n::t("Worked:", "Отработано:")),
                    label_style,
                ),
                Span::styled(
                    i18n::duration_text((total_working_hours * 3600.0) as i64),
                    value_style,
                ),
            ]))
            .style(Style::new().bg(theme().panel))
            .wrap(Wrap { trim: true }),
            left_rows[1],
        );

        // Метка «Сейчас» с временем задачи, под ней — название задачи.
        let mut current_task_header_spans =
            vec![Span::styled(i18n::t("Current task", "Сейчас"), label_style)];

        if let Some(task) = current_task {
            let elapsed = Local::now() - task.dt_start;

            current_task_header_spans.push(Span::styled(" · ", label_style));
            current_task_header_spans.push(Span::styled(
                i18n::duration_text(elapsed.num_seconds()),
                value_style,
            ));
        }

        frame.render_widget(
            Paragraph::new(Line::from(current_task_header_spans))
                .style(Style::new().bg(theme().panel))
                .wrap(Wrap { trim: true }),
            right_rows[0],
        );

        let task_name = match current_task {
            Some(task) => Span::styled(task.name.to_owned(), value_style),
            None => Span::styled(
                crate::ui::stage_label(&self.ui_state.stage).to_string(),
                label_style,
            ),
        };

        frame.render_widget(
            Paragraph::new(task_name)
                .style(Style::new().bg(theme().panel))
                .wrap(Wrap { trim: true }),
            right_rows[1],
        );
        frame.render_widget(
            Tabs::new(Tab::as_string_vec())
                .select(self.ui_state.get_active_tab_idx())
                .highlight_style(Style::default().fg(theme().accent))
                .block(
                    Block::new()
                        .border_set(symbols::border::Set {
                            bottom_left: symbols::line::NORMAL.vertical_right,
                            bottom_right: symbols::line::NORMAL.vertical_left,
                            ..symbols::border::THICK
                        })
                        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
                        .border_style(Style::new().fg(theme().border))
                        .title(Span::styled(
                            i18n::t("Tabs", "Вкладки"),
                            Style::new().fg(theme().text),
                        )),
                ),
            main_layouts[0],
        );

        match self.ui_state.get_active_tab() {
            Tab::Home => self.ui_state.render_home_tab(frame, main_layouts[1]),
            Tab::Start => self
                .ui_state
                .render_start_tab(frame, main_layouts[1], &completed_tasks),
            Tab::Out => self.ui_state.render_out_tab(frame, main_layouts[1]),
            Tab::End => {
                let view = crate::ui::EndOfDayView {
                    // В файл попадёт и задача, которая закроется вместе с днём.
                    tasks: completed_tasks.len() + usize::from(current_task.is_some()),
                    seconds: completed_seconds + running_seconds,
                    file_path: &planned_file,
                    running_task: current_task.as_ref().map(|task| task.name.as_str()),
                    running_seconds,
                };

                self.ui_state.render_end_tab(frame, main_layouts[1], &view)
            }
            Tab::Theme => self.ui_state.render_theme_tab(frame, main_layouts[1]),
            Tab::ClearState => self.ui_state.render_clear_tab(
                frame,
                main_layouts[1],
                // Предупреждение должно показывать всё, что реально пропадёт,
                // включая задачу, которая идёт прямо сейчас.
                completed_tasks.len() + usize::from(current_task.is_some()),
                completed_seconds + running_seconds,
            ),
        }

        if self.quit_armed {
            crate::ui::render_quit_warning(frame, area);
        } else if let Some(status) = &self.status {
            crate::ui::render_status_bar(frame, area, &status.text, status.is_error);
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

        let previous_task = state.current_task.as_ref().map(|task| {
            (
                task.name.clone(),
                (Local::now() - task.dt_start).num_seconds(),
            )
        });

        state.complete_current_task(previous_task_completion_message)?;

        let new_task = NotCompletedTask::start(new_task_name.clone());

        state.current_task.replace(new_task);

        state.save(state_file_path)?;

        Ok(match previous_task {
            Some((name, seconds)) => format!(
                "{}: «{}» ({}) · {}: «{}»",
                i18n::t("Closed", "Закрыто"),
                name,
                i18n::duration_text(seconds),
                i18n::t("Started", "Начато"),
                new_task_name
            ),
            None => format!("{}: «{}»", i18n::t("Started", "Начато"), new_task_name),
        })
    }

    fn execute_pause_command(
        &mut self,
        previous_task_completion_message: Option<String>,
    ) -> Result<String, String> {
        let state = &mut self.daily_state;
        let state_file_path = &self.state_file_path;

        let closed_task = state.current_task.as_ref().map(|task| {
            (
                task.name.clone(),
                (Local::now() - task.dt_start).num_seconds(),
            )
        });

        let complete_task_result = state.complete_current_task(previous_task_completion_message);

        state.current_task = None;

        complete_task_result?;

        state.save(state_file_path)?;

        Ok(match closed_task {
            Some((name, seconds)) => format!(
                "{}: «{}» ({})",
                i18n::t("Closed", "Закрыто"),
                name,
                i18n::duration_text(seconds)
            ),
            None => i18n::t("Paused", "Пауза").to_string(),
        })
    }

    fn execute_end_command(
        &mut self,
        previous_task_completion_message: Option<String>,
    ) -> Result<String, String> {
        let state = &mut self.daily_state;
        let state_file_path = &self.state_file_path;

        state.complete_current_task(previous_task_completion_message)?;

        // Сохраняем день до выгрузки: если xlsx не получится, данные не пропадут.
        state.save(state_file_path)?;

        match state.save_state_as_xlsx(&self.prediction_model_session, &self.config) {
            Ok(file_path) => {
                // Файл уже сохранён, поэтому день можно начинать заново;
                // если запись нового состояния сорвётся, скажем об этом.
                state.start_new_day();

                if let Err(error) = state.save(state_file_path) {
                    return Err(format!(
                        "{}: {file_path}. {}: {error}",
                        i18n::t("Saved", "Сохранено"),
                        i18n::t("Could not clear the day", "Не удалось очистить день")
                    ));
                }

                Ok(format!("{}: {file_path}", i18n::t("Saved", "Сохранено")))
            }
            Err(error) => Err(format!(
                "{}: {error}. {}",
                i18n::t("Could not save the xlsx file", "Не удалось сохранить .xlsx"),
                i18n::t(
                    "The day data is saved, try again",
                    "Данные дня сохранены, попробуйте ещё раз"
                )
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_key_switches_pages_and_arrows_scroll_the_list() {
        assert_eq!(theme_key_action(KeyCode::Tab), ThemeKey::NextTab);
        assert_eq!(theme_key_action(KeyCode::BackTab), ThemeKey::PreviousTab);
        assert_eq!(theme_key_action(KeyCode::Up), ThemeKey::PreviousTheme);
        assert_eq!(theme_key_action(KeyCode::Down), ThemeKey::NextTheme);
        assert_eq!(theme_key_action(KeyCode::Left), ThemeKey::Ignored);
        assert_eq!(theme_key_action(KeyCode::Right), ThemeKey::Ignored);
    }

    #[test]
    fn vertical_arrows_never_switch_pages() {
        for code in [KeyCode::Up, KeyCode::Down] {
            let action = theme_key_action(code);

            assert_ne!(action, ThemeKey::PreviousTab);
            assert_ne!(action, ThemeKey::NextTab);
        }
    }

    #[test]
    fn letters_are_ignored_on_theme_tab() {
        assert_eq!(theme_key_action(KeyCode::Char('x')), ThemeKey::Ignored);
        assert_eq!(theme_key_action(KeyCode::Enter), ThemeKey::Ignored);
    }

    #[test]
    fn reset_and_quit_keys_keep_working() {
        assert_eq!(theme_key_action(KeyCode::Char('r')), ThemeKey::ResetTheme);
        assert_eq!(theme_key_action(KeyCode::Char('R')), ThemeKey::ResetTheme);
        assert_eq!(theme_key_action(KeyCode::Esc), ThemeKey::Quit);
    }
}
#[cfg(test)]
mod day_rollover_tests {
    use super::*;

    fn scenario_directory(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("rollover")
            .join(name);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn state_json(start: DateTime<Local>, running_task: Option<&str>) -> String {
        let current_task = match running_task {
            Some(name) => format!(r#"{{"name":"{name}","dt_start":"{}"}}"#, start.to_rfc3339()),
            None => "null".to_string(),
        };

        format!(
            r#"{{"current_task":{current_task},"completed_tasks":[],"start_time":"{}","end_time":null}}"#,
            start.to_rfc3339()
        )
    }

    #[test]
    fn stale_state_opens_rollover_screen() {
        let directory = scenario_directory("stale");
        let state_path = directory.join("state.json");
        let yesterday = Local::now() - chrono::Duration::days(1);

        std::fs::write(&state_path, state_json(yesterday, Some("old task"))).unwrap();

        let app = App::init(&state_path);
        let rollover = app.pending_day.as_ref().unwrap();

        assert_eq!(rollover.running_task.as_deref(), Some("old task"));
        assert_eq!(rollover.date, yesterday.date_naive());
        assert_eq!(rollover.close_at.date_naive(), yesterday.date_naive());
    }

    #[test]
    fn rollover_can_be_cancelled_with_c() {
        let directory = scenario_directory("cancel");
        let state_path = directory.join("state.json");
        let yesterday = Local::now() - chrono::Duration::days(1);

        std::fs::write(&state_path, state_json(yesterday, Some("old task"))).unwrap();

        let mut app = App::init(&state_path);
        assert!(app.pending_day.is_some());

        app.handle_day_rollover_key(KeyCode::Char('c'));

        assert!(app.pending_day.is_none());
    }

    #[test]
    fn today_state_has_no_rollover() {
        let directory = scenario_directory("today");
        let state_path = directory.join("state.json");

        std::fs::write(&state_path, state_json(Local::now(), None)).unwrap();

        let app = App::init(&state_path);

        assert!(app.pending_day.is_none());
    }

    #[test]
    fn empty_previous_day_is_rolled_over_silently() {
        let directory = scenario_directory("empty-day");
        let state_path = directory.join("state.json");
        let yesterday = Local::now() - chrono::Duration::days(1);

        std::fs::write(&state_path, state_json(yesterday, None)).unwrap();

        let app = App::init(&state_path);

        assert!(app.pending_day.is_none());
        assert_eq!(app.daily_state.day(), Local::now().date_naive());
    }
}
#[cfg(test)]
mod submit_tests {
    use super::*;

    fn scenario(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("submit")
            .join(name);
        std::fs::create_dir_all(&directory).unwrap();

        directory.join("state.json")
    }

    fn type_task_name(app: &mut App, text: &str) {
        let mut control = app.ui_state_mut().task_name_input.lock().unwrap();

        if let Control::TaskNameInput(input) = &mut *control {
            input.set_input(text.to_string());
        }
    }

    #[test]
    fn enter_on_start_with_empty_name_reports_error() {
        let state_path = scenario("empty-name");
        let _ = std::fs::remove_file(&state_path);

        let mut app = App::init(&state_path);
        app.ui_state_mut().open_tab(Tab::Start);

        app.submit();

        let status = app.status.as_ref().unwrap();

        assert!(status.is_error);
        assert!(app.daily_state.current_task.is_none());
    }

    #[test]
    fn enter_on_start_starts_task_and_reports_success() {
        let state_path = scenario("start");
        let _ = std::fs::remove_file(&state_path);

        let mut app = App::init(&state_path);
        app.ui_state_mut().open_tab(Tab::Start);
        type_task_name(&mut app, "Проверить почту");

        app.submit();

        let status = app.status.as_ref().unwrap();

        assert!(!status.is_error);
        assert_eq!(
            app.daily_state
                .current_task
                .as_ref()
                .map(|task| task.name.as_str()),
            Some("Проверить почту")
        );
    }

    #[test]
    fn end_without_tasks_reports_nothing_to_export() {
        let state_path = scenario("empty-end");
        let _ = std::fs::remove_file(&state_path);

        let mut app = App::init(&state_path);
        app.ui_state_mut().open_tab(Tab::End);

        app.submit();

        let status = app.status.as_ref().unwrap();

        assert!(!status.is_error);
        assert!(status.text.contains("nothing to export"));
        assert!(!app.daily_state.has_any_data());
    }
}
#[cfg(test)]
mod safety_tests {
    use super::*;

    fn scenario(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("safety-app")
            .join(name);
        std::fs::create_dir_all(&directory).unwrap();

        directory.join("state.json")
    }

    #[test]
    fn maps_alt_digits_to_tabs() {
        assert_eq!(tab_from_hotkey(KeyCode::Char('1')), Some(Tab::Home));
        assert_eq!(tab_from_hotkey(KeyCode::Char('5')), Some(Tab::ClearState));
        assert_eq!(tab_from_hotkey(KeyCode::Char('6')), Some(Tab::Theme));
        assert_eq!(tab_from_hotkey(KeyCode::Char('7')), None);
        assert_eq!(tab_from_hotkey(KeyCode::Esc), None);
    }

    #[test]
    fn pause_without_a_task_does_not_switch_to_paused() {
        let state_path = scenario("pause-empty");
        let _ = std::fs::remove_file(&state_path);

        let mut app = App::init(&state_path);
        app.ui_state_mut().open_tab(Tab::Out);

        app.submit();

        assert!(matches!(app.ui_state_mut().stage, AppStage::Waiting));
    }

    #[test]
    fn clear_day_keeps_a_copy_and_reports_it() {
        let state_path = scenario("clear");
        let _ = std::fs::remove_file(&state_path);

        let mut app = App::init(&state_path);
        app.daily_state.current_task = Some(NotCompletedTask::start("work".to_string()));
        app.daily_state.current_task.as_mut().unwrap().dt_start =
            Local::now() - chrono::Duration::minutes(30);

        app.ui_state_mut().open_tab(Tab::ClearState);
        app.ui_state_mut().switch_control_focus_forwards();

        app.submit();

        let status = app.status.as_ref().unwrap();

        assert!(!status.is_error);
        assert!(status.text.contains("Day cleared"));
        assert!(status.text.contains("copy"));
        assert!(!app.daily_state.has_any_data());
    }
}
