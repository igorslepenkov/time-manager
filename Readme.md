# Steady Control time-manager

### Установка Rust

- Установить Rust из официального источника (https://www.rust-lang.org/tools/install)

#### Linux

```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

#### Windows

- Перейти по ссылке https://forge.rust-lang.org/infra/other-installation-methods.html#rustup
- Скачать rustup-init.exe

### Клонировать репозиторий с кодом

```
git clone git@github.com:igorslepenkov/time-manager.git
Или
git clone https://github.com/igorslepenkov/time-manager.git

Затем
git checkout feat/ratatui/add_ui
```

### Билд

В корне проекта:

```
cargo build --release
```

### Дальшейшие шаги

#### Linux

- Для своего удобства выделите папку под приложение.
- Переместите бинарник в папку
- Добавьте папку в переменную $PATH (это можно сделать через .bashrc или любым другим способом).

В bashrc можно сделать слудующую запись:

```
export TIME_MANAGER="путь к папке"

export PATH="$PATH:$TIME_MANAGER"

```

#### Windows

- Сделайте папку в любом месте где будет удобно и перенесите туда бинарник
- Сделайте ярлыки для папки и для бинарника на рабочем столе
