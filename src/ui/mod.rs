pub mod control;
pub mod tabs;

use std::sync::{Arc, Mutex};

use itertools::Itertools;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
    Frame,
};

/// Минимальный размер окна, при котором интерфейс ещё читается.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

use crate::{
    config::parse_color,
    i18n::{self, t},
    theme::{self, contrast_text, theme, ThemePreset, ThemeSource},
    utils::centered_rect,
};
use tabs::Tab;

pub use control::{Control, Input};

use self::control::SubmitButton;

pub enum AppStage {
    Waiting,
    Working,
    Paused,
}

/// Подпись состояния для шапки.
pub fn stage_label(stage: &AppStage) -> &'static str {
    match stage {
        AppStage::Waiting => t("No active task", "Нет активной задачи"),
        AppStage::Working => t("Task in progress", "Задача идёт"),
        AppStage::Paused => t("Paused", "На паузе"),
    }
}

pub struct AppUiState {
    submit_btn: Arc<Mutex<Control>>,
    active_tab: usize,
    pub control_focused: Option<Arc<Mutex<Control>>>,
    pub task_name_input: Arc<Mutex<Control>>,
    pub task_end_comment_input: Arc<Mutex<Control>>,
    pub stage: AppStage,
    /// Готовые темы из theme.json и номер выбранной.
    pub themes: Vec<ThemePreset>,
    /// Откуда взялась каждая тема — для строки под списком.
    pub theme_sources: Vec<ThemeSource>,
    pub theme_selected: usize,
    /// Индекс выполненной задачи, которая сейчас подставлена в поле ввода —
    /// по нему подсвечивается строка в списке задач.
    pub history_selection: Option<usize>,
    config_warning: Option<String>,
}

impl AppUiState {
    pub fn init(
        config_warning: Option<String>,
        themes: Vec<ThemePreset>,
        theme_selected: usize,
    ) -> AppUiState {
        AppUiState {
            task_end_comment_input: Arc::new(Mutex::new(Control::EndCommentInput(Input::init()))),
            task_name_input: Arc::new(Mutex::new(Control::TaskNameInput(Input::init()))),
            submit_btn: Arc::new(Mutex::new(Control::SubmitBtn(SubmitButton::init()))),
            active_tab: 0_usize,
            control_focused: None,
            stage: AppStage::Waiting,
            themes,
            theme_sources: Vec::new(),
            theme_selected,
            history_selection: None,
            config_warning,
        }
    }

    /// Открыть конкретную вкладку — нужно для предпросмотра экранов.
    pub fn open_tab(&mut self, tab: Tab) {
        let index = tab.index();

        if index < Tab::COUNT {
            self.active_tab = index;
            self.refocus_active_tab();
        }
    }

    pub fn selected_theme(&self) -> Option<&ThemePreset> {
        self.themes.get(self.theme_selected)
    }

    pub fn select_theme(&mut self, index: usize) {
        if index < self.themes.len() {
            self.theme_selected = index;
        }
    }

    pub fn select_next_theme(&mut self) {
        if self.themes.is_empty() {
            return;
        }

        self.theme_selected = (self.theme_selected + 1) % self.themes.len();
    }

    pub fn select_previous_theme(&mut self) {
        if self.themes.is_empty() {
            return;
        }

        self.theme_selected = (self.theme_selected + self.themes.len() - 1) % self.themes.len();
    }

    /// Переключение вкладки не трогает введённый текст: он остаётся
    /// в поле задачи и в поле комментария, пока не будет отправлен.
    /// Вкладки листаются по кругу.
    pub fn switch_tabs_forward(&mut self) {
        self.active_tab = (self.active_tab + 1) % Tab::COUNT;
        self.refocus_active_tab();
    }

    pub fn switch_tabs_backward(&mut self) {
        self.active_tab = (self.active_tab + Tab::COUNT - 1) % Tab::COUNT;
        self.refocus_active_tab();
    }

    /// Ставит фокус на первый контрол активной вкладки.
    pub fn refocus_active_tab(&mut self) {
        self.task_name_input.lock().unwrap().unset_focus();
        self.task_end_comment_input.lock().unwrap().unset_focus();
        self.submit_btn.lock().unwrap().unset_focus();

        match self.get_active_tab() {
            Tab::Start => {
                self.control_focused = Some(Arc::clone(&self.task_name_input));
                self.task_name_input.lock().unwrap().set_focus();
            }
            Tab::Out | Tab::End => {
                self.control_focused = Some(Arc::clone(&self.task_end_comment_input));
                self.task_end_comment_input.lock().unwrap().set_focus();
            }
            // На сбросе дня кнопка не в фокусе: Enter по привычке не должен
            // удалять день, до кнопки нужно дойти стрелкой.
            Tab::ClearState | Tab::Theme | Tab::Home => {
                self.control_focused = None;
            }
        }
    }

    pub fn is_submit_focused(&self) -> bool {
        match &self.control_focused {
            Some(control) => matches!(&*control.lock().unwrap(), Control::SubmitBtn(_)),
            None => false,
        }
    }

    pub fn switch_control_focus_forwards(&mut self) {
        if self.control_focused.is_none() {
            if let Tab::ClearState = self.get_active_tab() {
                self.control_focused = Some(Arc::clone(&self.submit_btn));
                self.submit_btn.lock().unwrap().set_focus();
            }

            return;
        }

        let active_tab = self.get_active_tab();

        match active_tab {
            Tab::Start => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::TaskNameInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_name_input));

                            state.unset_focus();
                            self.task_name_input.lock().unwrap().set_focus();
                        }
                    }
                }
            }
            Tab::End | Tab::Out => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub fn switch_control_focus_backwards(&mut self) {
        let active_tab = self.get_active_tab();

        match active_tab {
            Tab::Start => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::TaskNameInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_name_input));

                            state.unset_focus();
                            self.task_name_input.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                    }
                }
            }
            Tab::End | Tab::Out => {
                let input_focused_cell_option = self.control_focused.take();

                if let Some(input_focused_cell) = input_focused_cell_option {
                    let mut input_focused = input_focused_cell.lock().unwrap();

                    match &mut *input_focused {
                        Control::EndCommentInput(state) => {
                            self.control_focused = Some(Arc::clone(&self.submit_btn));

                            state.unset_focus();
                            self.submit_btn.lock().unwrap().set_focus();
                        }
                        Control::SubmitBtn(state) => {
                            self.control_focused = Some(Arc::clone(&self.task_end_comment_input));

                            state.unset_focus();
                            self.task_end_comment_input.lock().unwrap().set_focus();
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    pub fn clear_inputs_state(&mut self) {
        self.task_name_input.lock().unwrap().clear_input();
        self.task_end_comment_input.lock().unwrap().clear_input();
        self.submit_btn.lock().unwrap().unset_focus();
        self.history_selection = None;
    }

    pub fn get_active_tab(&self) -> Tab {
        let current_active_idx = self.active_tab;

        if current_active_idx == Tab::Home as usize {
            Tab::Home
        } else if current_active_idx == Tab::Start as usize {
            Tab::Start
        } else if current_active_idx == Tab::Out as usize {
            Tab::Out
        } else if current_active_idx == Tab::End as usize {
            Tab::End
        } else if current_active_idx == Tab::Theme as usize {
            Tab::Theme
        } else if current_active_idx == Tab::ClearState as usize {
            Tab::ClearState
        } else {
            Tab::Home
        }
    }

    pub fn get_active_tab_idx(&self) -> usize {
        self.active_tab
    }

    pub fn render_home_tab(&self, frame: &mut Frame, area: Rect) {
        let mut lines: Vec<Line> = vec![Line::from(vec![Span::styled(
            t("Welcome to the time manager tool!", "Трекер рабочего дня"),
            Style::new().fg(theme().accent).bold(),
        )])];

        lines.push(Line::from(""));

        lines.push(Line::from(vec![Span::styled(
            t(
                "Tracks time per task and exports the day to .xlsx",
                "Считаю время по задачам, в конце дня выгружаю в .xlsx",
            ),
            Style::default().fg(theme().muted),
        )]));

        lines.push(Line::from(""));

        lines.push(Line::from(vec![Span::styled(
            t("Here are some useful keys:", "Клавиши:"),
            Style::default().fg(theme().text).bold(),
        )]));

        // Две строки подсказок: навигация отдельно, редактирование отдельно —
        // так русские подписи не переносятся и не отрываются от чипов.
        let navigation = [
            ("Tab", t("Tabs", "Вкладки")),
            ("Alt+1…6", t("Tab by number", "Вкладка по номеру")),
            ("Esc", t("Quit", "Выход")),
        ];
        let editing = [
            ("↑/↓", t("Focus", "Фокус")),
            ("←/→", t("Cursor", "Курсор")),
            ("Enter", t("Submit", "Отправить")),
        ];

        for (index, keys) in [navigation, editing].into_iter().enumerate() {
            if index > 0 {
                lines.push(Line::from(""));
            }

            let spans = keys
                .iter()
                .flat_map(|(key, desc)| {
                    let key = Span::styled(format!(" {} ", key), theme().key_binding.key);
                    let desc = Span::styled(format!(" {} ", desc), theme().key_binding.description);
                    [key, desc]
                })
                .collect_vec();

            lines.push(
                Line::from(spans)
                    .centered()
                    .style(Style::new().fg(theme().text).bg(theme().background)),
            );
        }

        if let Some(warning) = &self.config_warning {
            lines.push(Line::from(vec![Span::styled(
                warning.to_owned(),
                Style::new().fg(theme().error).bold(),
            )]));
        }

        let inner_width = area.width.saturating_sub(4);
        let height = (rendered_text_height(&lines, inner_width, area.height) + 4).min(area.height);

        let text = Paragraph::new(Text::from(lines))
            .alignment(Alignment::Center)
            .wrap(Wrap::default())
            .block(
                Block::new()
                    .borders(Borders::empty())
                    .padding(Padding::new(2, 2, 2, 2)),
            );

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .flex(ratatui::layout::Flex::Center)
            .constraints(vec![Constraint::Length(height)])
            .split(area);

        frame.render_widget(text, rows[0])
    }

    pub fn render_start_tab(
        &self,
        frame: &mut Frame,
        area: Rect,
        completed_tasks: &[(String, i64)],
    ) {
        let inner_area = centered_rect(area, 92, 78);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(61), Constraint::Percentage(39)])
            .split(inner_area);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(37),
                Constraint::Percentage(38),
                Constraint::Percentage(25),
            ])
            .split(columns[0]);

        self.task_name_input
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[0], Tab::Start);

        self.task_end_comment_input.lock().unwrap().render(
            frame,
            area_vertical_layouts[1],
            Tab::Start,
        );
        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[2], Tab::Start);

        render_completed_tasks_list(frame, columns[1], completed_tasks, self.history_selection);
    }

    pub fn render_out_tab(&self, frame: &mut Frame, area: Rect) {
        let inner_area = centered_rect(area, 90, 70);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(20),
                Constraint::Percentage(50),
                Constraint::Percentage(30),
            ])
            .split(inner_area);

        let text = Text::from(t("Pause the day?", "Приостановить день?"));
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center)
            .fg(theme().text);
        frame.render_widget(paragraph, area_vertical_layouts[0]);

        let task_end_comment_rect = centered_rect(area_vertical_layouts[1], 100, 80);

        self.task_end_comment_input
            .lock()
            .unwrap()
            .render(frame, task_end_comment_rect, Tab::Out);

        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[2], Tab::Out)
    }

    pub fn render_end_tab(&self, frame: &mut Frame, area: Rect, view: &EndOfDayView) {
        let inner_area = centered_rect(area, 90, 70);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(6),
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(inner_area);

        let mut lines = vec![
            Line::from(Span::styled(
                t("Close the day?", "Закрыть день?"),
                Style::new().fg(theme().accent).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "{}: {} · {}",
                    t("Tasks", "Задач"),
                    view.tasks,
                    crate::i18n::duration_text(view.seconds)
                ),
                Style::new().fg(theme().text),
            )),
        ];

        if let Some(running_task) = view.running_task {
            lines.push(Line::from(Span::styled(
                format!(
                    "{}: {running_task} · {}",
                    t(
                        "Current task closes with the day",
                        "Текущая задача закроется вместе с днём"
                    ),
                    crate::i18n::duration_text(view.running_seconds)
                ),
                Style::new().fg(theme().text),
            )));
        }

        lines.push(Line::from(vec![
            Span::styled(
                format!("{}: ", t("File", "Файл")),
                Style::new().fg(theme().muted),
            ),
            Span::styled(view.file_path.to_owned(), Style::new().fg(theme().text)),
        ]));

        frame.render_widget(
            Paragraph::new(Text::from(lines))
                .alignment(Alignment::Center)
                .wrap(Wrap::default()),
            rows[0],
        );

        self.task_end_comment_input
            .lock()
            .unwrap()
            .render(frame, rows[1], Tab::End);
        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, rows[2], Tab::End);
    }

    pub fn render_clear_tab(&self, frame: &mut Frame, area: Rect, tasks: usize, seconds: i64) {
        let inner_area = centered_rect(area, 90, 70);

        let area_vertical_layouts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(5),
                Constraint::Length(1),
                Constraint::Length(3),
            ])
            .split(inner_area);

        let lines = vec![
            Line::from(Span::styled(
                t("Clear the day?", "Сбросить день?"),
                Style::new().fg(theme().error).bold(),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!(
                    "{}: {} · {}",
                    t("Tasks", "Задач"),
                    tasks,
                    crate::i18n::duration_text(seconds)
                ),
                Style::new().fg(theme().text),
            )),
            Line::from(Span::styled(
                t(
                    "A copy of the day stays next to state.json",
                    "Копия дня сохранится рядом со state.json",
                ),
                Style::new().fg(theme().muted),
            )),
        ];

        frame.render_widget(
            Paragraph::new(Text::from(lines)).alignment(Alignment::Center),
            area_vertical_layouts[0],
        );

        if !self.is_submit_focused() {
            frame.render_widget(
                Paragraph::new(t(
                    "Press Down to reach the button",
                    "Нажми ↓, чтобы дойти до кнопки",
                ))
                .alignment(Alignment::Center)
                .style(Style::new().fg(theme().muted)),
                area_vertical_layouts[1],
            );
        }

        self.submit_btn
            .lock()
            .unwrap()
            .render(frame, area_vertical_layouts[2], Tab::ClearState);
    }

    pub fn render_theme_tab(&self, frame: &mut Frame, area: Rect) {
        let inner_area = centered_width(area, theme_panel_width(area), 88);

        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(2),
                Constraint::Min(6),
                Constraint::Length(2),
            ])
            .split(inner_area);

        let header = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                t("Config", "Конфиг"),
                Style::new().fg(theme().accent).bold(),
            )),
            Line::from(vec![
                Span::styled(
                    t(
                        "Applied at once and remembered",
                        "Применяется сразу и запоминается",
                    ),
                    Style::new().fg(theme().muted),
                ),
                Span::styled(
                    format!(" · {}", self.theme_source_label()),
                    Style::new().fg(theme().text),
                ),
            ]),
        ]))
        .block(
            Block::new()
                .borders(Borders::empty())
                .padding(Padding::new(1, 1, 0, 0)),
        );

        frame.render_widget(header, rows[0]);

        // Слева список тем, справа блок языка, подсказки — полосой снизу.
        let body = rows[1];
        let list_width = 42_u16.min(body.width.saturating_sub(24)).max(28);
        let language_width = body.width.saturating_sub(list_width + 2).clamp(22, 32);

        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Length(list_width),
                Constraint::Min(1),
                Constraint::Length(language_width),
            ])
            .split(body);

        let list_height = (self.themes.len() as u16 + 2).min(body.height).max(3);
        let list_area = Rect::new(columns[0].x, columns[0].y, columns[0].width, list_height);

        self.render_theme_list(frame, list_area);

        let language_area = Rect::new(
            columns[2].x,
            columns[2].y,
            columns[2].width,
            3.min(body.height),
        );

        self.render_language_block(frame, language_area);

        if body.height > language_area.height {
            let hint_area = Rect::new(
                columns[2].x,
                language_area.y + language_area.height,
                columns[2].width,
                1,
            );

            frame.render_widget(
                Paragraph::new(Line::from(hint_spans(&[(
                    "L",
                    t("switch language", "сменить язык"),
                )])))
                .block(
                    Block::new()
                        .borders(Borders::empty())
                        .padding(Padding::new(1, 0, 0, 0)),
                ),
                hint_area,
            );
        }

        let hint_line = hint_spans(&[
            ("↑/↓", t("theme", "выбор темы")),
            ("R", t("default theme", "тема по умолчанию")),
        ]);

        frame.render_widget(
            Paragraph::new(Line::from(hint_line)).block(
                Block::new()
                    .borders(Borders::empty())
                    .padding(Padding::new(1, 1, 0, 0)),
            ),
            rows[2],
        );
    }

    /// Откуда взята выбранная тема: из бинарника или из файла в `themes/`.
    fn theme_source_label(&self) -> String {
        match self.theme_sources.get(self.theme_selected) {
            Some(ThemeSource::Builtin) => t("built-in theme", "встроенная тема").to_string(),
            Some(ThemeSource::File(path)) => {
                let file_name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string());

                format!("{}/{}", crate::theme::THEMES_DIRECTORY, file_name)
            }
            None => String::new(),
        }
    }

    /// Перечитывает папку тем, сохраняя выбранную тему по имени.
    pub fn reload_themes(&mut self, loaded: theme::LoadedThemes) -> Option<String> {
        let current_name = self
            .themes
            .get(self.theme_selected)
            .map(|preset| preset.name.clone());

        self.themes = loaded.presets;
        self.theme_sources = loaded.sources;

        self.theme_selected = current_name
            .and_then(|name| theme::find_preset_index(&self.themes, &name))
            .unwrap_or_else(|| self.theme_selected.min(self.themes.len().saturating_sub(1)));

        loaded.warning
    }

    /// Блок выбора языка: оба варианта рядом, текущий — акцентом.
    fn render_language_block(&self, frame: &mut Frame, area: Rect) {
        let current_language = i18n::lang();
        let mut spans = vec![];

        for (index, language) in [i18n::Lang::En, i18n::Lang::Ru].iter().enumerate() {
            if index > 0 {
                spans.push(Span::styled(" · ", Style::new().fg(theme().muted)));
            }

            let style = if *language == current_language {
                Style::new().fg(theme().accent).bold()
            } else {
                Style::new().fg(theme().muted)
            };

            spans.push(Span::styled(language.name(), style));
        }

        let paragraph = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Center)
            .block(
                Block::new()
                    .borders(Borders::ALL)
                    .border_style(Style::new().fg(theme().border))
                    .title(block_title(t("Language", "Язык"))),
            );

        frame.render_widget(paragraph, area);
    }
    /// Список готовых тем: имя и цветовая полоска. Выбранная тема
    /// подсвечивается фоном, без символа-маркера, чтобы строки не прыгали.
    fn render_theme_list(&self, frame: &mut Frame, area: Rect) {
        let visible_rows = area.height.saturating_sub(2) as usize;
        let selected_row = self.theme_selected;
        let offset = if visible_rows > 0 && selected_row >= visible_rows {
            selected_row + 1 - visible_rows
        } else {
            0
        };

        let mut lines: Vec<Line> = vec![];

        for index in offset..self.themes.len().min(offset + visible_rows.max(1)) {
            let preset = &self.themes[index];
            let is_selected = index == self.theme_selected;
            let name_style = if is_selected {
                Style::new()
                    .fg(theme().accent_text)
                    .bg(theme().accent)
                    .bold()
            } else {
                Style::new().fg(theme().text)
            };

            let mut spans = vec![Span::styled(
                format!(" {:<18} ", truncate(&preset.name, 18)),
                name_style,
            )];

            for (label, value) in [
                ("bg", &preset.colors.background),
                ("pn", &preset.colors.panel),
                ("ac", &preset.colors.accent),
                ("tx", &preset.colors.text),
            ] {
                let color = parse_color(value, Color::Reset);

                spans.push(Span::styled(
                    format!(" {label} "),
                    Style::new().fg(contrast_text(color)).bg(color),
                ));
                spans.push(Span::raw(" "));
            }

            lines.push(Line::from(spans));
        }

        let paragraph = Paragraph::new(Text::from(lines)).block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(theme().border))
                .title(block_title(format!(
                    "{}: {}",
                    t("Themes", "Темы"),
                    self.themes.len()
                ))),
        );

        frame.render_widget(paragraph, area);
    }
}

/// Данные для экрана «вчерашняя смена не закрыта».
pub struct DayRolloverView<'a> {
    pub date: String,
    pub finished_tasks: usize,
    pub finished_hours: f64,
    pub running_task: Option<&'a str>,
    pub running_since: Option<String>,
    pub close_at: String,
    pub error: Option<&'a str>,
}

/// Экран, который показывается при запуске, если в state.json остался
/// вчерашний день: предлагает выгрузить его и начать новый.
pub fn render_day_rollover(frame: &mut Frame, area: Rect, view: &DayRolloverView) {
    let mut lines: Vec<Line> = vec![
        Line::from(Span::styled(
            t("Yesterday's day is not closed", "Вчерашний день не закрыт"),
            Style::new().fg(theme().error).bold(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}: {}", t("Date", "Дата"), view.date),
            Style::new().fg(theme().text),
        )),
        Line::from(Span::styled(
            format!(
                "{}: {} ({})",
                t("Finished tasks", "Выполнено задач"),
                view.finished_tasks,
                i18n::duration_text((view.finished_hours * 3600.0) as i64)
            ),
            Style::new().fg(theme().text),
        )),
    ];

    if let Some(task) = view.running_task {
        let since = view
            .running_since
            .clone()
            .unwrap_or_else(|| "?".to_string());

        lines.push(Line::from(Span::styled(
            format!(
                "{}: {task} {} {since}",
                t("Running", "Идёт"),
                t("since", "с")
            ),
            Style::new().fg(theme().text),
        )));
        lines.push(Line::from(Span::styled(
            format!(
                "{} {} ({}).",
                t("The task will be closed at", "Задача закроется в"),
                view.close_at,
                t("by last activity", "по последней активности")
            ),
            Style::new().fg(theme().muted),
        )));
    }

    lines.push(Line::from(""));

    let hints = [
        (
            "Enter",
            t(
                "save yesterday to xlsx and start a new day",
                "сохранить вчерашний день в xlsx и начать новый",
            ),
        ),
        (
            "C",
            t(
                "keep it open and continue",
                "оставить открытым и продолжить",
            ),
        ),
        ("Esc", t("quit without changes", "выйти без изменений")),
    ];

    for (key, description) in hints {
        lines.push(Line::from(vec![
            Span::styled(format!(" {key:^5} "), theme().key_binding.key),
            Span::raw(" "),
            Span::styled(description, Style::new().fg(theme().text)),
        ]));
    }

    if let Some(error) = view.error {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            error.to_owned(),
            Style::new().fg(theme().error).bold(),
        )));
    }

    // Карточка по содержимому и по центру экрана, но не больше самого экрана.
    // Высоту считаем по строкам после переноса: длинные подсказки и текст
    // ошибки на узком окне занимают по две строки. Плюс две рамки и по строке
    // отступа сверху и снизу.
    let block_width = (area.width * 70 / 100).max(40).min(area.width);
    // Ширина текста внутри блока: две рамки и отступы по два символа слева и справа.
    let inner_width = block_width.saturating_sub(6);
    let wrapped_lines = rendered_text_height(&lines, inner_width, area.height);
    let block_height = (wrapped_lines + 4).min(area.height);

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap::default())
        .block(
            Block::new()
                .borders(Borders::ALL)
                .border_style(Style::new().fg(theme().border))
                .title(block_title(t("New day", "Новый день")))
                .padding(Padding::new(2, 2, 1, 1)),
        );

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .flex(ratatui::layout::Flex::Center)
        .constraints(vec![Constraint::Length(block_height)])
        .split(area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .flex(ratatui::layout::Flex::Center)
        .constraints(vec![Constraint::Length(block_width)])
        .split(rows[0]);

    frame.render_widget(paragraph, columns[0]);
}

/// Сколько строк займёт текст после переноса: рисуем его во временном буфере
/// и смотрим последнюю занятую строку — так ширина считается ровно так же,
/// как при настоящей отрисовке.
fn rendered_text_height(lines: &[Line], width: u16, max_height: u16) -> u16 {
    let area = Rect::new(0, 0, width.max(1), max_height.max(1));
    let mut buffer = ratatui::buffer::Buffer::empty(area);

    let paragraph = Paragraph::new(Text::from(lines.to_vec())).wrap(Wrap::default());
    ratatui::widgets::Widget::render(paragraph, area, &mut buffer);

    let width = area.width as usize;
    let used_rows = buffer
        .content
        .iter()
        .rposition(|cell| cell.symbol() != " ")
        .map(|index| index / width + 1)
        .unwrap_or(1);

    used_rows as u16
}

/// Плашки «клавиша + описание» для строк подсказок.
fn hint_spans(hints: &[(&str, &str)]) -> Vec<Span<'static>> {
    hints
        .iter()
        .flat_map(|(key, description)| {
            [
                Span::styled(format!(" {} ", key), theme().key_binding.key),
                Span::styled(
                    format!(" {} ", description),
                    theme().key_binding.description,
                ),
            ]
        })
        .collect_vec()
}
/// Полоса внизу экрана: «задача идёт, Esc нажми ещё раз».
pub fn render_quit_warning(frame: &mut Frame, area: Rect) {
    render_bar(
        frame,
        area,
        t(
            "The task is still running. Press Esc again to quit, any other key to stay.",
            "Задача всё ещё идёт. Esc ещё раз — выйти, любая другая клавиша — остаться.",
        ),
        theme().error,
        theme().background,
    );
}

/// Полоса внизу экрана с результатом последнего действия.
pub fn render_status_bar(frame: &mut Frame, area: Rect, message: &str, is_error: bool) {
    let background = if is_error {
        theme().error
    } else {
        theme().accent
    };

    let foreground = if is_error {
        theme().background
    } else {
        theme().accent_text
    };

    render_bar(frame, area, message, background, foreground);
}

/// Длинные сообщения (пути, ошибки) обрезаются, чтобы не рвать полосу.
fn fit_message(message: &str, width: usize) -> String {
    let available = width.saturating_sub(2);

    if message.chars().count() <= available {
        return message.to_string();
    }

    // Сохраняем начало и конец: причина ошибки обычно в хвосте.
    let head_length = available.saturating_sub(1) / 2;
    let tail_length = available.saturating_sub(1) - head_length;
    let head: String = message.chars().take(head_length).collect();
    let tail: String = message
        .chars()
        .skip(message.chars().count() - tail_length)
        .collect();

    format!("{head}…{tail}")
}

fn render_bar(frame: &mut Frame, area: Rect, message: &str, background: Color, foreground: Color) {
    if area.height == 0 {
        return;
    }

    let bar = Rect::new(area.x, area.y + area.height - 1, area.width, 1);
    let fitted = fit_message(message, area.width as usize);

    // Сначала заливаем строку пробелами: иначе под полосой остаются символы
    // рамки карточки и по краям видны обрубки.
    frame.render_widget(
        Paragraph::new(" ".repeat(bar.width as usize)).style(Style::new().bg(background)),
        bar,
    );
    frame.render_widget(
        Paragraph::new(format!(" {fitted} "))
            .alignment(Alignment::Center)
            .style(Style::new().fg(foreground).bg(background).bold()),
        bar,
    );
}
/// Заголовок блока: в разрыве рамки обычный цвет темы читается лучше,
/// чем цвет рамки.
fn block_title(text: impl Into<String>) -> Span<'static> {
    Span::styled(text.into(), Style::new().fg(theme().text))
}

/// Данные для экрана «Конец дня».
pub struct EndOfDayView<'a> {
    pub tasks: usize,
    pub seconds: i64,
    pub file_path: &'a str,
    pub running_task: Option<&'a str>,
    pub running_seconds: i64,
}

/// Ширина панели темы: растёт вместе с окном, но не до бесконечности.
fn theme_panel_width(area: Rect) -> u16 {
    ((area.width as u32 * 60 / 100) as u16).clamp(66, 90)
}

/// Экран-заглушка, если окно слишком маленькое для интерфейса.
pub fn render_too_small(frame: &mut Frame, area: Rect) {
    let message = format!(
        "{} {}×{}",
        t(
            "Window is too small — make it at least",
            "Окно слишком маленькое — нужно минимум"
        ),
        MIN_WIDTH,
        MIN_HEIGHT
    );

    let paragraph = Paragraph::new(message)
        .alignment(Alignment::Center)
        .style(Style::new().fg(theme().error).bold())
        .wrap(Wrap::default());

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Min(0),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    frame.render_widget(paragraph, rows[1]);
}

/// Прямоугольник фиксированной ширины по центру — для списка тем, которому
/// не нужна вся ширина окна.
fn centered_width(area: Rect, width: u16, percent_y: u16) -> Rect {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let middle = rows[1];
    let width = width.min(middle.width);
    let x = middle.x + (middle.width - width) / 2;

    Rect::new(x, middle.y, width, middle.height)
}
/// Обрезает длинное имя темы, чтобы строка списка не разъезжалась.
fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>()
        + "…"
}

/// Список выполненных задач на вкладке Start: подсвечивает ту задачу, которая
/// сейчас подставлена в поле ввода, и держит подсветку в видимой области.
fn render_completed_tasks_list(
    frame: &mut Frame,
    area: Rect,
    completed_tasks: &[(String, i64)],
    selected: Option<usize>,
) {
    let block = Block::new()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(theme().border))
        .title(block_title(format!(
            "{}: {}",
            t("Completed tasks", "Выполнено задач"),
            completed_tasks.len()
        )));

    if completed_tasks.is_empty() {
        let paragraph = Paragraph::new(t("Nothing finished yet", "Пока ничего не выполнено"))
            .alignment(Alignment::Center)
            .style(Style::new().fg(theme().muted))
            .block(block);

        frame.render_widget(paragraph, area);

        return;
    }

    let inner_width = area.width.saturating_sub(2) as usize;
    // Колонка времени фиксированной ширины: точка‑разделитель стоит на одном
    // месте во всех строках, время — от неё до правого края.
    let durations = completed_tasks
        .iter()
        .map(|(_, seconds)| crate::i18n::duration_text(*seconds))
        .collect::<Vec<String>>();
    let duration_width = durations
        .iter()
        .map(|duration| duration.chars().count())
        .max()
        .unwrap_or(0);
    let name_width = inner_width.saturating_sub(duration_width + 3);

    let items: Vec<ListItem> = completed_tasks
        .iter()
        .zip(durations)
        .map(|((name, _), duration)| {
            let fitted_name = format!("{:<width$}", truncate(name, name_width), width = name_width);
            let fitted_duration = format!("{duration:<width$}", width = duration_width);

            ListItem::new(Line::from(vec![
                Span::styled(fitted_name, Style::new().fg(theme().text)),
                Span::styled(" · ", Style::new().fg(theme().text)),
                Span::styled(fitted_duration, Style::new().fg(theme().text)),
            ]))
        })
        .collect();

    // Без highlight_symbol: подсветка только фоном, иначе строки прыгают вправо.
    let list = List::new(items).block(block).highlight_style(
        Style::new()
            .fg(theme().accent_text)
            .bg(theme().accent)
            .bold(),
    );

    let mut state = ListState::default();
    state.select(selected);

    if let Some(selected) = selected {
        let visible_rows = area.height.saturating_sub(2) as usize;

        if visible_rows > 0 && selected >= visible_rows {
            *state.offset_mut() = selected + 1 - visible_rows;
        }
    }

    frame.render_stateful_widget(list, area, &mut state);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::builtin_presets;
    use ratatui::{backend::TestBackend, Terminal};

    fn test_state() -> AppUiState {
        AppUiState::init(None, builtin_presets(), 0)
    }

    fn render_to_lines(width: u16, height: u16, draw: impl Fn(&mut Frame)) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame)).unwrap();

        let buffer = terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                let mut line = String::new();

                for x in 0..buffer.area.width {
                    line.push_str(buffer.get(x, y).symbol());
                }

                line
            })
            .collect()
    }

    fn type_into_task_name(state: &AppUiState, text: &str) {
        let mut control = state.task_name_input.lock().unwrap();

        if let Control::TaskNameInput(input) = &mut *control {
            for char in text.chars() {
                input.insert_char(char);
            }
        } else {
            panic!("task name control expected");
        }
    }

    fn task_name(state: &AppUiState) -> String {
        let control = state.task_name_input.lock().unwrap();

        if let Control::TaskNameInput(input) = &*control {
            input.input.to_owned()
        } else {
            panic!("task name control expected");
        }
    }

    #[test]
    fn switching_tabs_keeps_typed_task_name() {
        let mut state = test_state();

        state.switch_tabs_forward();
        assert_eq!(state.get_active_tab(), Tab::Start);

        type_into_task_name(&state, "самая классная задача");

        state.switch_tabs_forward();
        state.switch_tabs_forward();
        state.switch_tabs_backward();

        assert_eq!(state.get_active_tab(), Tab::Out);
        assert_eq!(task_name(&state), "самая классная задача");
    }

    #[test]
    fn tab_switching_wraps_around() {
        let mut state = test_state();
        let total = Tab::COUNT;

        for _ in 0..total {
            state.switch_tabs_forward();
        }
        assert_eq!(state.get_active_tab(), Tab::Home);

        // Шаг назад с первой вкладки попадает на последнюю в списке — Config.
        state.switch_tabs_backward();
        assert_eq!(state.get_active_tab(), Tab::Theme);
    }

    #[test]
    fn tab_switch_moves_focus_to_tab_control() {
        let mut state = test_state();

        state.switch_tabs_forward();

        assert_eq!(state.get_active_tab(), Tab::Start);
        assert!(!state.is_submit_focused());

        state.switch_tabs_forward();
        state.switch_control_focus_forwards();

        assert!(state.is_submit_focused());
    }

    #[test]
    fn home_tab_has_no_focused_control() {
        let mut state = test_state();

        state.switch_tabs_forward();
        state.switch_tabs_backward();

        assert_eq!(state.get_active_tab(), Tab::Home);
        assert!(state.control_focused.is_none());
    }

    #[test]
    fn theme_tab_is_reachable_and_has_no_focused_control() {
        let mut state = test_state();
        state.open_tab(Tab::Theme);

        assert_eq!(state.get_active_tab(), Tab::Theme);
        assert!(state.control_focused.is_none());
    }

    #[test]
    fn theme_selection_wraps_around() {
        let mut state = test_state();
        let total = state.themes.len();

        state.select_previous_theme();
        assert_eq!(state.theme_selected, total - 1);

        state.select_next_theme();
        assert_eq!(state.theme_selected, 0);
    }

    #[test]
    fn theme_tab_lists_presets_without_marker() {
        let mut state = test_state();
        state.open_tab(Tab::Theme);

        let screen = render_to_lines(120, 40, |frame| state.render_theme_tab(frame, frame.size()))
            .join("\n");

        assert!(screen.contains("Themes:"));
        assert!(screen.contains("Dracula"));
        assert!(screen.contains("Gruvbox Dark"));
        assert!(!screen.contains("> Default"));
    }

    #[test]
    fn start_tab_lists_completed_tasks_without_marker() {
        let mut state = test_state();
        state.switch_tabs_forward();
        state.history_selection = Some(1);

        let tasks = vec![
            ("first task".to_string(), 25 * 60),
            ("second task".to_string(), 40 * 60),
            ("third task".to_string(), 5 * 60),
        ];

        let screen = render_to_lines(120, 34, |frame| {
            state.render_start_tab(frame, frame.size(), &tasks)
        })
        .join("\n");

        assert!(screen.contains("Completed tasks: 3"));
        assert!(screen.contains("second task"));
        assert!(screen.contains("40min"));
        assert!(!screen.contains("> second task"));
    }

    #[test]
    fn start_tab_shows_placeholder_without_tasks() {
        let mut state = test_state();
        state.switch_tabs_forward();

        let screen = render_to_lines(120, 34, |frame| {
            state.render_start_tab(frame, frame.size(), &[])
        })
        .join("\n");

        assert!(screen.contains("Completed tasks: 0"));
        assert!(screen.contains("Nothing finished yet"));
    }

    #[test]
    fn truncates_long_theme_names() {
        assert_eq!(truncate("Nord", 10), "Nord");
        assert_eq!(truncate("Catppuccin Mocha", 10), "Catppucci…");
    }
}
#[cfg(test)]
mod status_bar_tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn status_bar_fills_the_whole_row() {
        let mut terminal = Terminal::new(TestBackend::new(40, 4)).unwrap();

        terminal
            .draw(|frame| render_status_bar(frame, frame.size(), "Saved: report.xlsx", false))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let last_row = buffer.area.height - 1;
        let left = buffer.get(0, last_row);
        let right = buffer.get(buffer.area.width - 1, last_row);

        assert_eq!(left.bg, right.bg);
        assert_ne!(left.bg, Color::Reset);

        let text: String = (0..buffer.area.width)
            .map(|x| buffer.get(x, last_row).symbol())
            .collect();

        assert!(text.contains("Saved: report.xlsx"));
    }

    #[test]
    fn error_and_success_bars_differ_in_background() {
        let mut success = Terminal::new(TestBackend::new(40, 4)).unwrap();
        success
            .draw(|frame| render_status_bar(frame, frame.size(), "ok", false))
            .unwrap();
        let success_color = success.backend().buffer().get(0, 3).bg;

        let mut error = Terminal::new(TestBackend::new(40, 4)).unwrap();
        error
            .draw(|frame| render_status_bar(frame, frame.size(), "fail", true))
            .unwrap();
        let error_color = error.backend().buffer().get(0, 3).bg;

        assert_ne!(success_color, error_color);
    }
}
#[cfg(test)]
mod layout_tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_to_text(width: u16, height: u16, draw: impl Fn(&mut Frame)) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| draw(frame)).unwrap();

        let buffer = terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn too_small_window_shows_a_hint_instead_of_the_ui() {
        let screen = render_to_text(70, 18, |frame| render_too_small(frame, frame.size()));

        assert!(screen.contains("Window is too small"));
        assert!(screen.contains("80×24"));
    }

    #[test]
    fn theme_panel_grows_with_the_window_but_has_limits() {
        let narrow = Rect::new(0, 0, 60, 30);
        let normal = Rect::new(0, 0, 120, 30);
        let wide = Rect::new(0, 0, 220, 40);

        assert_eq!(theme_panel_width(narrow), 66);
        assert_eq!(theme_panel_width(normal), 72);
        assert_eq!(theme_panel_width(wide), 90);
    }
}
#[cfg(test)]
mod clear_tab_tests {
    use super::*;
    use crate::theme::builtin_presets;
    use ratatui::{backend::TestBackend, Terminal};

    fn render_clear_screen(tasks: usize, seconds: i64) -> String {
        let state = AppUiState::init(None, builtin_presets(), 0);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();

        terminal
            .draw(|frame| state.render_clear_tab(frame, frame.size(), tasks, seconds))
            .unwrap();

        let buffer = terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn clear_tab_shows_counts_and_keeps_button_out_of_focus() {
        let screen = render_clear_screen(3, 3 * 3600 + 15 * 60);

        assert!(screen.contains("Tasks: 3"));
        assert!(screen.contains("3h 15min"));
        assert!(screen.contains("A copy of the day"));
        assert!(screen.contains("Press Down"));
    }
}
#[cfg(test)]
mod end_tab_tests {
    use super::*;
    use crate::theme::builtin_presets;
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn end_tab_shows_totals_running_task_and_file() {
        let state = AppUiState::init(None, builtin_presets(), 0);
        let view = EndOfDayView {
            tasks: 4,
            seconds: 3 * 3600 + 40 * 60,
            file_path: r"D:\work\13.11.2026.xlsx",
            running_task: Some("Починить сборку"),
            running_seconds: 25 * 60,
        };

        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal
            .draw(|frame| state.render_end_tab(frame, frame.size(), &view))
            .unwrap();

        let buffer = terminal.backend().buffer();
        let screen = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n");

        assert!(screen.contains("Close the day?"));
        assert!(screen.contains("Tasks: 4"));
        assert!(screen.contains("3h 40min"));
        assert!(screen.contains("Починить сборку"));
        assert!(screen.contains("D:\\work\\13.11.2026.xlsx"));
    }
}
#[cfg(test)]
mod rollover_tests {
    use super::*;
    use crate::theme::builtin_presets;
    use ratatui::{backend::TestBackend, Terminal};

    fn render(view: &DayRolloverView, width: u16, height: u16) -> String {
        crate::theme::apply_colors(&builtin_presets()[0].colors);
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();

        terminal
            .draw(|frame| render_day_rollover(frame, frame.size(), view))
            .unwrap();

        let buffer = terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    fn long_error_view(error: &str) -> DayRolloverView<'_> {
        DayRolloverView {
            date: "12.11.2026".to_string(),
            finished_tasks: 4,
            finished_hours: 5.25,
            running_task: Some("Починить сборку релиза"),
            running_since: Some("18:40".to_string()),
            close_at: "20:15".to_string(),
            error: Some(error),
        }
    }

    #[test]
    fn error_text_is_not_clipped_on_the_smallest_window() {
        let error = "Не удалось сохранить xlsx: нет доступа к папке выгрузки, проверь права и свободное место";
        let screen = render(&long_error_view(error), 80, 24);

        assert!(screen.contains("not closed"));
        assert!(screen.contains("свободное место"), "экран: {screen}");
    }

    #[test]
    fn finished_day_text_fits_without_error_line() {
        let view = DayRolloverView {
            date: "12.11.2026".to_string(),
            finished_tasks: 4,
            finished_hours: 5.25,
            running_task: None,
            running_since: None,
            close_at: "20:15".to_string(),
            error: None,
        };
        let screen = render(&view, 80, 24);

        assert!(screen.contains("Finished tasks"));
        assert!(screen.contains("Date: 12.11.2026"));
    }
}
#[cfg(test)]
mod theme_source_tests {
    use super::*;
    use crate::theme::{builtin_presets, LoadedThemes, ThemePreset, ThemeSource};
    use ratatui::{backend::TestBackend, Terminal};

    fn custom(name: &str) -> ThemePreset {
        ThemePreset {
            name: name.to_string(),
            colors: Default::default(),
        }
    }

    fn render(state: &AppUiState) -> String {
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal
            .draw(|frame| state.render_theme_tab(frame, frame.size()))
            .unwrap();

        let buffer = terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn shows_where_the_selected_theme_comes_from() {
        let mut state = AppUiState::init(None, builtin_presets(), 0);
        state.theme_sources = vec![ThemeSource::Builtin; state.themes.len()];
        let builtin_screen = render(&state);

        assert!(builtin_screen.contains("built-in"));

        state.themes = vec![custom("Default"), custom("Моя тема")];
        state.theme_sources = vec![
            ThemeSource::Builtin,
            ThemeSource::File(std::path::PathBuf::from("themes/mine.json")),
        ];
        state.theme_selected = 1;

        let custom_screen = render(&state);

        assert!(custom_screen.contains("themes/mine.json"));
    }

    #[test]
    fn reload_keeps_the_selected_theme_by_name() {
        let mut state = AppUiState::init(None, vec![custom("A"), custom("B")], 3);
        state.theme_selected = 1;

        let loaded = LoadedThemes {
            presets: vec![custom("A"), custom("B"), custom("C")],
            sources: vec![ThemeSource::Builtin; 3],
            warning: None,
        };

        let warning = state.reload_themes(loaded);

        assert!(warning.is_none());
        assert_eq!(state.themes.len(), 3);
        assert_eq!(state.themes[state.theme_selected].name, "B");
    }
}
#[cfg(test)]
mod small_window_tests {
    use super::*;
    use crate::theme::builtin_presets;
    use ratatui::{backend::TestBackend, Terminal};

    fn render(state: &AppUiState, draw: impl FnOnce(&AppUiState, &mut Frame)) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();

        terminal.draw(|frame| draw(state, frame)).unwrap();

        let buffer = terminal.backend().buffer();

        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.get(x, y).symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<String>>()
            .join("\n")
    }

    #[test]
    fn main_pages_stay_usable_on_80_by_24() {
        let state = AppUiState::init(None, builtin_presets(), 0);

        let home = render(&state, |state, frame| {
            state.render_home_tab(frame, frame.size())
        });
        assert!(home.contains("Welcome"));
        assert!(home.contains("Tabs"));

        let start = render(&state, |state, frame| {
            state.render_start_tab(frame, frame.size(), &[("Задача".to_string(), 25 * 60)])
        });
        assert!(start.contains("New task description"));
        assert!(start.contains("Completed tasks"));

        let end = render(&state, |state, frame| {
            state.render_end_tab(
                frame,
                frame.size(),
                &EndOfDayView {
                    tasks: 4,
                    seconds: 3 * 3600,
                    file_path: r"D:\work\13.11.2026.xlsx",
                    running_task: Some("Задача"),
                    running_seconds: 600,
                },
            )
        });
        assert!(end.contains("Close the day?"));
        assert!(end.contains("Tasks: 4"));

        let theme = render(&state, |state, frame| {
            state.render_theme_tab(frame, frame.size())
        });
        assert!(theme.contains("Config"));
        assert!(theme.contains("Language"));

        let clear = render(&state, |state, frame| {
            state.render_clear_tab(frame, frame.size(), 4, 3 * 3600)
        });
        assert!(clear.contains("Clear the day?"));
        assert!(clear.contains("Tasks: 4"));
    }
}
