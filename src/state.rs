use std::fs::File;
use std::io::prelude::*;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::config::Config;
use crate::export;
use crate::i18n::t;
use crate::tagging;
use crate::task::{CompletedTask, NotCompletedTask};

use chrono::prelude::*;
use ort::Session;
use serde::{Deserialize, Serialize};

/// Что осталось от сброшенного дня.
pub struct ClearOutcome {
    pub archived_tasks: usize,
    pub archived_seconds: i64,
    pub archive_path: Option<PathBuf>,
}

/// Результат загрузки дня вместе с предупреждением, если файл пришлось чинить.
pub struct LoadedState {
    pub state: DailyState,
    pub warning: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct DailyState {
    pub current_task: Option<NotCompletedTask>,
    pub completed_tasks: Mutex<Vec<CompletedTask>>,
    pub start_time: DateTime<Local>,
    pub end_time: Option<DateTime<Local>>,
}

impl DailyState {
    /// День, к которому относится состояние.
    pub fn day(&self) -> NaiveDate {
        self.start_time.date_naive()
    }

    pub fn is_from_previous_day(&self, today: NaiveDate) -> bool {
        let task_from_previous_day = self
            .current_task
            .as_ref()
            .map(|task| task.dt_start.date_naive() < today)
            .unwrap_or(false);

        self.day() < today || task_from_previous_day
    }

    pub fn has_finished_tasks(&self) -> bool {
        self.completed_tasks
            .lock()
            .map(|tasks| !tasks.is_empty())
            .unwrap_or(false)
    }

    /// Есть ли что терять при сбросе дня.
    pub fn has_any_data(&self) -> bool {
        self.current_task.is_some() || self.has_finished_tasks()
    }

    /// Завершает открытую задачу указанным временем, не теряя сделанное.
    pub fn close_current_task_at(&mut self, dt_end: DateTime<Local>) -> Result<(), String> {
        let Some(task) = self.current_task.take() else {
            return Ok(());
        };

        let dt_end = if dt_end < task.dt_start {
            task.dt_start
        } else {
            dt_end
        };

        let completed_task = task.complete_task_at(dt_end, None);
        let mut tasks = self
            .completed_tasks
            .lock()
            .map_err(|error| error.to_string())?;

        tasks.push(completed_task);

        Ok(())
    }

    /// Начинает день с чистого листа: ни задач, ни открытой смены.
    pub fn start_new_day(&mut self) {
        self.current_task = None;
        self.completed_tasks = Mutex::new(vec![]);
        self.start_time = Local::now();
        self.end_time = None;
    }

    pub fn save(&self, file_path: &PathBuf) -> Result<String, String> {
        let json = serde_json::to_string(self).map_err(|error| {
            format!(
                "{}: {error}",
                t(
                    "Could not save the day data",
                    "Не удалось сохранить данные дня"
                )
            )
        })?;

        write_file_atomically(file_path, &json)?;

        Ok("State saved".to_owned())
    }

    /// Сколько секунд набежало по завершённым задачам.
    pub fn completed_seconds(&self) -> i64 {
        self.completed_tasks
            .lock()
            .map(|tasks| {
                tasks
                    .iter()
                    .map(|task| (task.dt_end - task.dt_start).num_seconds().max(0))
                    .sum()
            })
            .unwrap_or(0)
    }

    /// Сбрасывает день, но сначала сохраняет его копию рядом: случайный Enter
    /// не должен уничтожать данные навсегда.
    pub fn clear_todays_state(&mut self, file_path: &PathBuf) -> Result<ClearOutcome, String> {
        let archived_tasks = self
            .completed_tasks
            .lock()
            .map(|tasks| tasks.len())
            .unwrap_or(0);
        let archived_seconds = self.completed_seconds();
        let archive_path = self.archive_day(file_path)?;

        self.start_new_day();
        self.save(file_path)?;

        Ok(ClearOutcome {
            archived_tasks,
            archived_seconds,
            archive_path,
        })
    }

    /// Копия дня рядом с основным файлом: `state.cleared-13.11.2026.json`.
    fn archive_day(&self, file_path: &PathBuf) -> Result<Option<PathBuf>, String> {
        if !file_path.exists() {
            return Ok(None);
        }

        let date = self.start_time.format("%d.%m.%Y").to_string();
        let archive_path = file_path.with_file_name(format!("state.cleared-{date}.json"));

        std::fs::copy(file_path, &archive_path).map_err(|error| {
            format!(
                "{} {}: {error}",
                t(
                    "Could not save the day copy",
                    "Не удалось сохранить копию дня"
                ),
                archive_path.display()
            )
        })?;

        Ok(Some(archive_path))
    }

    fn new_state(file_path: &PathBuf) -> Result<DailyState, String> {
        let state = DailyState {
            current_task: None,
            completed_tasks: Default::default(),
            start_time: Local::now(),
            end_time: None,
        };

        state.save(file_path)?;

        Ok(state)
    }

    /// Загружает день: свежий, сохранённый или восстановленный после порчи файла.
    /// Если файл испорчен, он копируется в `.broken` и возвращается предупреждение.
    pub fn load(file_path: &PathBuf) -> LoadedState {
        let raw = match std::fs::read_to_string(file_path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(recovered) = DailyState::recover_from_temporary(file_path) {
                    return recovered;
                }

                return match DailyState::new_state(file_path) {
                    Ok(state) => LoadedState {
                        state,
                        warning: None,
                    },
                    Err(message) => LoadedState {
                        state: DailyState::fresh(),
                        warning: Some(message),
                    },
                };
            }
            Err(error) => {
                return LoadedState {
                    state: DailyState::fresh(),
                    warning: Some(format!(
                        "{} {}: {}. {}",
                        t("Cannot read", "Не удалось прочитать"),
                        file_path.display(),
                        error,
                        t("starting a new day", "начат новый день")
                    )),
                };
            }
        };

        match serde_json::from_str::<DailyState>(crate::config::strip_bom(&raw)) {
            Ok(state) => LoadedState {
                state,
                warning: None,
            },
            Err(error) => {
                if let Some(recovered) = DailyState::recover_from_temporary(file_path) {
                    return recovered;
                }

                let backup_path = file_path.with_extension("json.broken");
                let state = DailyState::fresh();
                let backup_result = std::fs::copy(file_path, &backup_path);

                match backup_result {
                    Ok(_) => {
                        let _ = state.save(file_path);

                        LoadedState {
                            state,
                            warning: Some(format!(
                                "{} ({}): {}. {}: {}",
                                t("state.json is damaged", "state.json повреждён"),
                                file_path.display(),
                                error,
                                t("a copy is saved in", "копия сохранена в"),
                                backup_path.display()
                            )),
                        }
                    }
                    Err(copy_error) => LoadedState {
                        state,
                        warning: Some(format!(
                            "{} ({}): {}. {}: {copy_error}",
                            t("state.json is damaged", "state.json повреждён"),
                            file_path.display(),
                            error,
                            t("the file is left untouched", "файл оставлен без изменений")
                        )),
                    },
                }
            }
        }
    }

    /// Если процесс убили посреди записи, рядом остаётся `.tmp` с целым JSON —
    /// пробуем поднять день из него.
    fn recover_from_temporary(file_path: &PathBuf) -> Option<LoadedState> {
        let directory = file_path.parent()?;
        let base_name = file_path.file_name()?.to_string_lossy().to_string();
        let prefix = format!("{base_name}.");

        let mut candidates = std::fs::read_dir(directory)
            .ok()?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .map(|name| {
                        let name = name.to_string_lossy();

                        name.starts_with(&prefix) && name.ends_with(".tmp")
                    })
                    .unwrap_or(false)
            })
            .collect::<Vec<PathBuf>>();

        candidates.sort_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|metadata| metadata.modified())
                .ok()
        });

        for candidate in candidates.into_iter().rev() {
            let Ok(text) = std::fs::read_to_string(&candidate) else {
                continue;
            };

            let Ok(state) = serde_json::from_str::<DailyState>(crate::config::strip_bom(&text))
            else {
                continue;
            };

            let _ = state.save(file_path);
            let _ = std::fs::remove_file(&candidate);

            return Some(LoadedState {
                state,
                warning: Some(format!(
                    "{}: {}",
                    t(
                        "The day was restored from the temporary file",
                        "День восстановлен из временного файла"
                    ),
                    candidate.display()
                )),
            });
        }

        None
    }

    fn fresh() -> DailyState {
        DailyState {
            current_task: None,
            completed_tasks: Default::default(),
            start_time: Local::now(),
            end_time: None,
        }
    }
    /// Собирает xlsx через модуль экспорта: здесь только сбор задач и тегов.
    pub fn save_state_as_xlsx(
        &mut self,
        model: &Session,
        config: &Config,
    ) -> Result<String, String> {
        let completed_tasks = self
            .completed_tasks
            .lock()
            .map_err(|error| error.to_string())?
            .to_vec();

        let task_tags = tagging::predict_task_tags(model, &completed_tasks);

        export::write_workbook(config, self.start_time, &completed_tasks, &task_tags)
    }
    pub fn complete_current_task(
        &mut self,
        task_completion_message: Option<String>,
    ) -> Result<(), String> {
        let task_to_complite_option = self.current_task.take();

        if task_to_complite_option.is_none() {
            return Ok(());
        }

        let task_to_complite = task_to_complite_option.unwrap();
        let mut complited_task = task_to_complite.complete_task(task_completion_message);

        let tasks = self.completed_tasks.lock();

        match tasks {
            Ok(mut tasks) => {
                let mut new_tasks_state = tasks.to_vec();

                if let Some(message) = &complited_task.end_comment {
                    complited_task.name =
                        [complited_task.name.to_owned(), message.to_owned()].join(". ");
                }

                new_tasks_state.push(complited_task);

                *tasks = new_tasks_state;

                Ok(())
            }
            Err(err) => Err(err.to_string()),
        }
    }
}

/// Пишет файл через временный и переименование: при сбое посреди записи
/// старый файл остаётся целым.
fn write_file_atomically(path: &PathBuf, contents: &str) -> Result<(), String> {
    // pid в имени: два запущенных экземпляра не должны писать в один временный файл.
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "state.json".to_string());
    let temporary_path = path.with_file_name(format!("{file_name}.{}.tmp", std::process::id()));

    {
        let mut file = File::create(&temporary_path).map_err(|error| {
            format!(
                "{} {}: {error}",
                t("Could not create", "Не удалось создать"),
                path.display()
            )
        })?;

        file.write_all(contents.as_bytes()).map_err(|error| {
            format!(
                "{} {}: {error}",
                t("Could not write", "Не удалось записать"),
                path.display()
            )
        })?;

        file.sync_all().map_err(|error| {
            format!(
                "{} {}: {error}",
                t("Could not flush", "Не удалось сбросить на диск"),
                path.display()
            )
        })?;
    }

    std::fs::rename(&temporary_path, path).map_err(|error| {
        format!(
            "{} {}: {error}",
            t(
                "Could not replace the day file",
                "Не удалось заменить файл дня"
            ),
            path.display()
        )
    })
}

#[cfg(test)]
mod day_tests {
    use super::*;

    fn state_at(
        start: DateTime<Local>,
        task: Option<NotCompletedTask>,
        tasks: Vec<CompletedTask>,
    ) -> DailyState {
        DailyState {
            current_task: task,
            completed_tasks: Mutex::new(tasks),
            start_time: start,
            end_time: None,
        }
    }

    fn open_task(name: &str, start: DateTime<Local>) -> NotCompletedTask {
        NotCompletedTask {
            name: name.to_string(),
            dt_start: start,
        }
    }

    fn finished_task(name: &str, start: DateTime<Local>, minutes: i64) -> CompletedTask {
        CompletedTask {
            name: name.to_string(),
            dt_start: start,
            dt_end: start + chrono::Duration::minutes(minutes),
            end_comment: None,
        }
    }

    #[test]
    fn detects_previous_day_by_start_time() {
        let today = Local::now().date_naive();
        let yesterday = Local::now() - chrono::Duration::days(1);

        assert!(state_at(yesterday, None, vec![]).is_from_previous_day(today));
        assert!(!state_at(Local::now(), None, vec![]).is_from_previous_day(today));
    }

    #[test]
    fn detects_task_that_started_yesterday() {
        let today = Local::now().date_naive();
        let yesterday = Local::now() - chrono::Duration::days(1);

        let state = state_at(Local::now(), Some(open_task("old", yesterday)), vec![]);

        assert!(state.is_from_previous_day(today));
    }

    #[test]
    fn closes_open_task_at_requested_time() {
        let task_start = Local::now() - chrono::Duration::minutes(60);
        let close_at = task_start + chrono::Duration::minutes(25);

        let mut state = state_at(task_start, Some(open_task("work", task_start)), vec![]);

        state.close_current_task_at(close_at).unwrap();

        assert!(state.current_task.is_none());

        let tasks = state.completed_tasks.lock().unwrap();

        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].name, "work");
        assert_eq!(tasks[0].dt_end, close_at);
    }

    #[test]
    fn closing_earlier_than_start_gives_zero_duration() {
        let task_start = Local::now();
        let mut state = state_at(task_start, Some(open_task("work", task_start)), vec![]);

        state
            .close_current_task_at(task_start - chrono::Duration::hours(2))
            .unwrap();

        let tasks = state.completed_tasks.lock().unwrap();

        assert_eq!(tasks[0].dt_end, tasks[0].dt_start);
    }

    #[test]
    fn closing_without_open_task_changes_nothing() {
        let mut state = state_at(
            Local::now(),
            None,
            vec![finished_task("done", Local::now(), 10)],
        );

        state.close_current_task_at(Local::now()).unwrap();

        assert_eq!(state.completed_tasks.lock().unwrap().len(), 1);
    }

    #[test]
    fn new_day_clears_tasks_and_time() {
        let yesterday = Local::now() - chrono::Duration::days(1);
        let mut state = state_at(
            yesterday,
            Some(open_task("work", yesterday)),
            vec![finished_task("done", yesterday, 30)],
        );

        state.start_new_day();

        assert!(state.current_task.is_none());
        assert!(state.completed_tasks.lock().unwrap().is_empty());
        assert_eq!(state.day(), Local::now().date_naive());
    }

    #[test]
    fn empty_state_has_no_data() {
        assert!(!state_at(Local::now(), None, vec![]).has_any_data());
        assert!(
            state_at(Local::now(), Some(open_task("work", Local::now())), vec![]).has_any_data()
        );
    }
}

#[cfg(test)]
mod parse_tests {
    use super::*;

    #[test]
    fn parses_state_written_on_previous_day() {
        let json = r#"{"current_task":{"name":"Tect 2","dt_start":"2026-11-12T18:40:00+03:00"},"completed_tasks":[{"name":"Tect 1","dt_start":"2026-11-12T17:00:00+03:00","dt_end":"2026-11-12T18:40:00+03:00","end_comment":null}],"start_time":"2026-11-12T09:00:00+03:00","end_time":null}"#;

        let state: DailyState = serde_json::from_str(json).unwrap();

        assert!(state.current_task.is_some());
        assert_eq!(state.completed_tasks.lock().unwrap().len(), 1);
    }

    #[test]
    fn parses_state_file_with_bom() {
        let json = "\u{feff}{\"current_task\":null,\"completed_tasks\":[],\"start_time\":\"2026-11-12T09:00:00+03:00\",\"end_time\":null}";

        let state: DailyState = serde_json::from_str(crate::config::strip_bom(json)).unwrap();

        assert!(state.current_task.is_none());
    }
}

#[cfg(test)]
mod reliability_tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("state")
            .join(name);
        let _ = std::fs::create_dir_all(&directory);

        directory
    }

    #[test]
    fn save_and_load_round_trip() {
        let directory = test_directory("round-trip");
        let path = directory.join("state.json");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("json.tmp"));

        let start = Local::now() - chrono::Duration::hours(1);
        let mut state = DailyState::fresh();
        state.start_time = start;
        state.completed_tasks = Mutex::new(vec![CompletedTask {
            name: "done task".to_string(),
            dt_start: start,
            dt_end: start + chrono::Duration::minutes(30),
            end_comment: None,
        }]);
        state.current_task = Some(NotCompletedTask {
            name: "current task".to_string(),
            dt_start: start,
        });

        state.save(&path).unwrap();

        let loaded = DailyState::load(&path);

        assert!(loaded.warning.is_none());
        assert_eq!(
            loaded
                .state
                .current_task
                .as_ref()
                .map(|task| task.name.as_str()),
            Some("current task")
        );
        assert_eq!(loaded.state.completed_tasks.lock().unwrap().len(), 1);
        assert_eq!(loaded.state.start_time, start);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn broken_state_is_backed_up_and_replaced() {
        let directory = test_directory("broken");
        let path = directory.join("state.json");
        let broken_path = path.with_extension("json.broken");
        let _ = std::fs::remove_file(&broken_path);

        std::fs::write(&path, "{ not json }").unwrap();

        let loaded = DailyState::load(&path);

        assert!(loaded.warning.is_some());
        assert!(broken_path.exists());
        assert_eq!(
            std::fs::read_to_string(&broken_path).unwrap(),
            "{ not json }"
        );
        assert!(!loaded.state.has_any_data());
    }

    #[test]
    fn missing_state_creates_a_fresh_day() {
        let directory = test_directory("missing");
        let path = directory.join("fresh.json");
        let _ = std::fs::remove_file(&path);

        let loaded = DailyState::load(&path);

        assert!(loaded.warning.is_none());
        assert!(path.exists());
        assert_eq!(loaded.state.day(), Local::now().date_naive());
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;

    fn test_directory(name: &str) -> PathBuf {
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-output")
            .join("safety")
            .join(name);
        let _ = std::fs::create_dir_all(&directory);

        directory
    }

    fn state_with_task(start: DateTime<Local>) -> DailyState {
        let mut state = DailyState::fresh();
        state.start_time = start;
        state.completed_tasks = Mutex::new(vec![CompletedTask {
            name: "work".to_string(),
            dt_start: start,
            dt_end: start + chrono::Duration::minutes(90),
            end_comment: None,
        }]);

        state
    }

    #[test]
    fn clear_day_archives_a_copy_before_reset() {
        let directory = test_directory("clear-archive");
        let path = directory.join("state.json");
        let _ = std::fs::remove_file(&path);

        let start = Local::now() - chrono::Duration::hours(2);
        let mut state = state_with_task(start);
        state.save(&path).unwrap();

        let outcome = state.clear_todays_state(&path).unwrap();

        assert_eq!(outcome.archived_tasks, 1);
        assert_eq!(outcome.archived_seconds, 90 * 60);

        let archive_path = outcome.archive_path.expect("archive path");
        assert!(archive_path.exists());

        let archived_text = std::fs::read_to_string(&archive_path).unwrap();
        let archived: DailyState = serde_json::from_str(&archived_text).unwrap();

        assert_eq!(archived.completed_tasks.lock().unwrap().len(), 1);
        assert!(state.completed_tasks.lock().unwrap().is_empty());
        assert!(path.exists());
        assert_eq!(state.day(), Local::now().date_naive());
    }

    #[test]
    fn recovers_day_from_temporary_file() {
        let directory = test_directory("recover-tmp");
        let path = directory.join("state.json");
        let _ = std::fs::remove_file(&path);

        let start = Local::now() - chrono::Duration::hours(1);
        let interrupted = DailyState {
            current_task: Some(NotCompletedTask {
                name: "pending".to_string(),
                dt_start: start,
            }),
            completed_tasks: Default::default(),
            start_time: start,
            end_time: None,
        };
        let temporary_path = directory.join("state.json.4242.tmp");
        std::fs::write(
            &temporary_path,
            serde_json::to_string(&interrupted).unwrap(),
        )
        .unwrap();

        let loaded = DailyState::load(&path);

        assert!(loaded.warning.is_some());
        assert_eq!(
            loaded
                .state
                .current_task
                .as_ref()
                .map(|task| task.name.as_str()),
            Some("pending")
        );
        assert!(path.exists());
        assert!(!temporary_path.exists());
    }

    #[test]
    fn clear_day_without_file_changes_nothing_on_disk() {
        let directory = test_directory("clear-empty");
        let path = directory.join("missing.json");
        let _ = std::fs::remove_file(&path);

        let mut state = DailyState::fresh();
        let outcome = state.clear_todays_state(&path).unwrap();

        assert_eq!(outcome.archived_tasks, 0);
        assert!(outcome.archive_path.is_none());
        assert!(path.exists());
    }
}
#[cfg(test)]
mod export_integration_tests {
    use super::*;

    #[test]
    fn exports_previous_day_under_its_own_date() {
        let model = tagging::load_model();

        let yesterday = Local::now() - chrono::Duration::days(1);
        let start = yesterday
            .date_naive()
            .and_hms_opt(9, 0, 0)
            .unwrap()
            .and_local_timezone(Local)
            .earliest()
            .unwrap();

        let mut state = DailyState {
            current_task: None,
            completed_tasks: Mutex::new(vec![CompletedTask {
                name: "work".to_string(),
                dt_start: start,
                dt_end: start + chrono::Duration::minutes(30),
                end_comment: None,
            }]),
            start_time: start,
            end_time: None,
        };

        let file_path = state
            .save_state_as_xlsx(&model, &Config::default())
            .unwrap();
        let expected_name = format!("{}.xlsx", start.format("%d.%m.%Y"));

        assert!(file_path.ends_with(&expected_name), "got {file_path}");

        let _ = std::fs::remove_file(&file_path);
    }
}
