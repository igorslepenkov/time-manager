//! Чтение текста из системного буфера обмена.

/// Возвращает текст из буфера обмена. Пустая строка или ошибка — None.
pub fn read_clipboard() -> Option<String> {
    read_clipboard_impl().filter(|text| !text.is_empty())
}

#[cfg(target_os = "windows")]
fn read_clipboard_impl() -> Option<String> {
    use std::ptr;
    use winapi::shared::minwindef::HGLOBAL;
    use winapi::um::winbase::{GlobalLock, GlobalUnlock};
    use winapi::um::winuser::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, CF_UNICODETEXT,
    };

    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return None;
        }

        if OpenClipboard(ptr::null_mut()) == 0 {
            return None;
        }

        let handle = GetClipboardData(CF_UNICODETEXT);
        let mut text = None;

        if !handle.is_null() {
            let pointer = GlobalLock(handle as HGLOBAL) as *const u16;

            if !pointer.is_null() {
                let mut length = 0usize;

                while *pointer.add(length) != 0 {
                    length += 1;
                }

                text = Some(String::from_utf16_lossy(std::slice::from_raw_parts(
                    pointer, length,
                )));

                GlobalUnlock(handle as HGLOBAL);
            }
        }

        CloseClipboard();

        text
    }
}

#[cfg(target_os = "macos")]
fn read_clipboard_impl() -> Option<String> {
    read_from_command("pbpaste", &[])
}

#[cfg(all(unix, not(target_os = "macos")))]
fn read_clipboard_impl() -> Option<String> {
    read_from_command("xclip", &["-selection", "clipboard", "-o"])
        .or_else(|| read_from_command("xsel", &["--clipboard", "--output"]))
}

#[cfg(not(target_os = "windows"))]
fn read_from_command(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}
