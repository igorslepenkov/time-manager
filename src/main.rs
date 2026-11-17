use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};

use anyhow::{anyhow, Result};

use std::{env, io::stderr, path::PathBuf};

use time_manager::app::App;

const STATE_FILE_NAME: &str = "state.json";

fn main() -> Result<()> {
    initialize_panic_handler();
    enable_terminal_colors();

    let current_exe_path = env::current_exe()?;
    let current_dir_path = current_exe_path
        .parent()
        .ok_or_else(|| anyhow!("Could not resolve the executable directory"))?;
    let state_file_path: PathBuf = current_dir_path.join(STATE_FILE_NAME);

    let app = App::init(&state_file_path);

    startup()?;

    let status = app.run();

    shutdown()?;

    status?;

    Ok(())
}

fn startup() -> Result<()> {
    enable_raw_mode()?;
    stderr().execute(EnterAlternateScreen)?;

    Ok(())
}

fn shutdown() -> Result<()> {
    stderr().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    Ok(())
}

pub fn initialize_panic_handler() {
    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        // В хуке не паникуем второй раз: если терминал уже сломан, просто
        // пытаемся вернуть его в нормальное состояние.
        let _ = stderr().execute(LeaveAlternateScreen);
        let _ = disable_raw_mode();
        original_hook(panic_info);
    }));
}
/// Приложение рисует в stderr, а crossterm проверяет поддержку ANSI по stdout.
/// Если stdout перенаправлен (запуск из скрипта, cmd, конвейера), цвета
/// пропадают — поэтому включаем VT-режим на stderr и подсказываем crossterm,
/// что терминал ANSI поддерживает.
#[cfg(windows)]
fn enable_terminal_colors() {
    use winapi::um::consoleapi::{GetConsoleMode, SetConsoleMode};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::processenv::GetStdHandle;
    use winapi::um::winbase::STD_ERROR_HANDLE;
    use winapi::um::wincon::ENABLE_VIRTUAL_TERMINAL_PROCESSING;

    // Интерфейс построен на теме, поэтому цвета нужны всегда: иначе переменная
    // NO_COLOR в окружении делает окно белым на чёрном и темы теряют смысл.
    crossterm::style::force_color_output(true);

    if std::env::var_os("TERM").is_none() {
        std::env::set_var("TERM", "xterm-256color");
    }

    unsafe {
        let handle = GetStdHandle(STD_ERROR_HANDLE);

        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            return;
        }

        let mut mode = 0;

        if GetConsoleMode(handle, &mut mode) != 0 {
            SetConsoleMode(handle, mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING);
        }
    }
}

#[cfg(not(windows))]
fn enable_terminal_colors() {
    crossterm::style::force_color_output(true);

    if std::env::var_os("TERM").is_none() {
        std::env::set_var("TERM", "xterm-256color");
    }
}
