# macOS

Код кроссплатформенный, отдельная ветка не нужна. Собирать нужно на самом Mac: кросс-компиляция с Windows непрактична из-за macOS SDK.

## Требования

- macOS и Xcode Command Line Tools:

  ```sh
  xcode-select --install
  ```

- [rustup](https://rustup.rs):

  ```sh
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

## Сборка

```sh
git clone https://github.com/ncioo/sc-time-manager.git
cd sc-time-manager
cargo build --release
```

Первый build скачивает готовый ONNX Runtime (crate `ort`) — нужен интернет. Дальше сборка идёт из кэша.

## libonnxruntime.dylib

На Windows ONNX Runtime встроен в exe статически, поэтому получается один файл. На macOS он линкуется динамически, и рядом с бинарником должна лежать `libonnxruntime.dylib`: сборка кладёт её в `target/release`.

Для переноса скопируйте оба файла:

```sh
mkdir -p dist
cp target/release/sc-time-manager target/release/libonnxruntime.dylib dist/
```

Если dyld не находит библиотеку, укажите путь явно:

```sh
export DYLD_LIBRARY_PATH="$PWD/dist"
```

Или пропишите rpath:

```sh
install_name_tool -add_rpath @executable_path dist/sc-time-manager
```

Проверить зависимости бинарника можно через `otool -L dist/sc-time-manager`.

## Запуск

Приложение консольное: ему нужен терминал. Двойной клик по бинарнику обычно открывает Терминал, но надёжнее обёртка `sc-time-manager.command`:

```sh
#!/bin/sh
cd "$(dirname "$0")"
exec ./sc-time-manager
```

Не забудьте `chmod +x sc-time-manager.command`.

Буфер обмена читается через `pbpaste`; готовый `.xlsx` открывается через `open`, если включён `open_after_export`. В `output_dir` используйте абсолютный путь или `~/...`: синтаксис `%USERPROFILE%` — только для Windows.

## Gatekeeper

Сборка не подписана и не нотаризована, поэтому Finder скажет «не удаётся проверить разработчика». Варианты:

- правый клик по файлу → «Открыть» → «Открыть»;
- снять карантин: `xattr -d com.apple.quarantine ./sc-time-manager`;
- для распространения — Apple Developer ID (99 $ в год) и нотаризация через `codesign` и `notarytool`.

## См. также

- [usage.md](usage.md) — вкладки, клавиши и работа с днём.
- [config.md](config.md) — `config.json`, выгрузка и темы.
