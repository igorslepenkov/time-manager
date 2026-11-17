# Сборка и запуск под macOS

Код кроссплатформенный, отдельная ветка для macOS не нужна. Но собирать нужно на самом Mac: кросс-компиляция с Windows непрактична из-за macOS SDK. Ниже — что установить, как собрать и что учесть при переносе.

## Что нужно

- macOS (Apple Silicon или Intel) и Xcode Command Line Tools:

  ```sh
  xcode-select --install
  ```

- Rust через [rustup](https://rustup.rs):

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- Python 3 с Pillow — только если понадобится переснимать скриншоты, для работы приложения не нужен.

## Сборка

```sh
git clone https://github.com/ncioo/sc-sc-time-manager.git
cd sc-sc-time-manager
cargo build --release
```

Первый build скачивает готовый ONNX Runtime (crate `ort`, фича `download-binaries`) — нужен интернет. Дальше сборка идёт из кэша.

## libonnxruntime.dylib

На Windows ONNX Runtime встраивается в exe статически, поэтому получается один переносимый файл. На macOS `ort` линкуется динамически, и рядом с бинарником должна лежать `libonnxruntime.dylib`: сборка кладёт её (симлинк) в `target/release`. Для переноса копируйте оба файла:

```sh
mkdir -p dist
cp target/release/sc-time-manager target/release/libonnxruntime.dylib dist/
```

Если при запуске dyld пишет, что библиотека не найдена, укажите путь явно:

```sh
export DYLD_LIBRARY_PATH="$PWD/dist"
```

Либо пропишите rpath в бинарник, чтобы он искал библиотеку рядом с собой:

```sh
install_name_tool -add_rpath @executable_path dist/sc-time-manager
```

Проверить, куда ссылается бинарник, можно через `otool -L dist/sc-time-manager`.

## Запуск

- Приложение консольное: ему нужен терминал. Двойной клик по бинарнику в Finder обычно открывает Терминал, но надёжнее сделать обёртку `sc-time-manager.command`:

  ```sh
  #!/bin/sh
  cd "$(dirname "$0")"
  exec ./sc-time-manager
  ```

  Не забудьте `chmod +x sc-time-manager.command`.

- Буфер обмена на macOS читается через `pbpaste`, поэтому `Ctrl+V` (или `Cmd+V` в терминале) работает без отдельной библиотеки.
- Готовый `.xlsx` открывается через `open`, если в `config.json` включён `open_after_export`.
- Раскрытие переменных вида `%USERPROFILE%` — это синтаксис Windows. На macOS в `output_dir` используйте абсолютный путь или `~/...`; пустая строка по-прежнему означает «рядом с бинарником».
- Рядом с бинарником создаются `config.json`, `state.json` и, при наличии своих тем, папка `themes/`. Папка должна быть доступна на запись (не кладите приложение в `/Applications`, если запускаете бинарник напрямую из неё).

## Gatekeeper

Сборка не подписана и не нотаризована, поэтому Finder скажет «не удаётся проверить разработчика». Варианты:

- правый клик по файлу → «Открыть» → «Открыть» (один раз на файл);
- снять карантин из терминала: `xattr -d com.apple.quarantine ./sc-time-manager`;
- для нормального распространения — Apple Developer ID (99 $/год) и нотаризация через `codesign` + `notarytool`.

Подпись и нотаризация нужны только если вы отдаёте сборку другим людям: на своей машине достаточно одного «Открыть».

## Тесты

```sh
cargo test
```

Часть тестов помечена `#[cfg(windows)]` (пути `%TEMP%`, `%USERPROFILE%`, буфер обмена) — на macOS они просто не запускаются, это нормально. Остальные должны проходить.

## Упаковка в .app (по желанию)

Для TUI-приложения `.app` даёт мало: всё равно нужен терминал. Если хочется иконку в Dock, минимальная структура такая:

```
Time Manager.app/
  Contents/
    MacOS/
      sc-time-manager
      libonnxruntime.dylib
```

`Info.plist` можно сгенерировать вручную (`CFBundleExecutable` = `sc-time-manager`), но проще распространять `.tar.gz` с бинарником, библиотекой и `.command`-обёрткой.
