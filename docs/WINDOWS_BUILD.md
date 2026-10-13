# Windows build guide

## Prerequisites

- Windows 11 x64.
- Rust stable, host `x86_64-pc-windows-msvc`.
- Visual Studio Build Tools с компонентами **Desktop development with C++**, MSVC x64/x86 и Windows SDK. Для новых машин рекомендуется Build Tools 2022 или новее. На этой машине обнаружен и используется установленный Build Tools 2019; фактические результаты — в `VERIFICATION.md`.
- Node.js и npm, версии совместимые с lockfile (разработка: Node 24.12.0, npm 11.6.2).
- WebView2 Evergreen Runtime.
- Интернет для получения зависимостей и NSIS-инструментов при первой сборке.

Rust установлен для текущего пользователя. Если Cargo не в PATH, добавьте путь только в текущий терминал:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
rustc --version
cargo --version
```

Установка Rust: официальный `https://rustup.rs/`. Не используйте GNU-target или WSL для этого Windows release.

## Место на диске

Проект находится на `F:\Code\RUST\TeleDrop`. `node_modules`, `dist`, `src-tauri/target`, тестовые отчёты и `release` остаются на F:. Rust debug/release target может занимать несколько GB; не переносите его на C: при недостатке места.

Rustup, уже установленный Cargo cache и Chromium тестового runner в этой среде находятся в пользовательском профиле C:. Сборка не скачивает дополнительные браузеры или toolchains. Для будущих npm-установок можно направить cache на F: без глобальной смены настроек:

```powershell
npm ci --cache F:\Code\RUST\TeleDrop\.cache\npm
```

`scripts/package-windows.ps1` направляет `TEMP`/`TMP` только текущего процесса в `.build-temp` внутри проекта. Перенос существующего Rust toolchain/cache требует отдельного решения; скрипт их не переносит и не удаляет.

## Проверки

```powershell
npm ci
npm run build
npm run lint
npm run test
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml
npm run test:ui
```

Если Playwright ещё не установлен: `npx playwright install chromium`. Это зависимость разработчика, не пользователя.

## Installer

```powershell
pwsh -File scripts/package-windows.ps1 -Format Installer
```

Прямой эквивалент: `npm run build:windows`. Tauri собирает release для MSVC и NSIS-installer. Оригинальный путь:

```text
src-tauri/target/release/bundle/nsis/TeleDrop_1.0.0_x64-setup.exe
```

Скрипт копирует installer в `release/`, формирует SHA-256 и копирует пользовательскую документацию. Цифровая подпись не настроена; подписание сертификатом издателя — отдельный release-процесс, а не фиктивная «подпись».

## Portable

```powershell
pwsh -File scripts/package-windows.ps1 -Format Portable
```

Результат: `release/TeleDrop_1.0.0_x64-portable.zip`. Внутри находятся `TeleDrop.exe`, README, лицензия и notices. Не входят npm, Node, Rust, MCP и тестовые инструменты. Portable использует Windows-профиль для данных, а не папку рядом с EXE.

Для копирования одного приложения: `-Format Exe`. Этот EXE требует WebView2 и системных библиотек Windows. Это **не** автономный single-file с встроенным браузером.

Если release уже собран и проверен, используйте `-SkipBuild` для упаковки без повторной компиляции.

## WebView2

В `src-tauri/tauri.conf.json` установлено `webviewInstallMode: downloadBootstrapper`. Installer использует интернет при отсутствии runtime. Portable runtime не устанавливает.

Для офлайн-поставки можно отдельно собрать installer с официальным `offlineInstaller` или распространить WebView2 согласно документации Microsoft. Такая поставка существенно больше; в этой версии она **не собрана и не проверена**. Не устанавливайте `skip`, а затем не объявляйте installer автономным.

## Desktop smoke

```powershell
npm run test:desktop
# Или указать копию portable EXE:
node scripts/smoke-desktop.mjs release/TeleDrop.exe
```

Тест запускает настоящий EXE и подключается к его WebView2 на localhost:9223. Debug endpoint активируется переменной окружения **только тестового процесса**; production-приложение само его не включает. Не запускайте тест, если бот настроен или autoConnect включён. Тест не читает токен и не подключается к Telegram.

Проверяются команды SQLite, все пять страниц в двух темах, сохранение темы после повторного запуска, отсутствие frontend-ошибок и завершение процесса. Скриншоты: `test-results/desktop`. Ручные действия с ботом, Explorer, native dialogs и установленной версией перечислены в `E2E_CHECKLIST.md`.

## Обновления

Application identifier `app.teledrop.desktop` сохраняется между версиями. История и настройки находятся вне installation directory; обновление бинарника их не удаляет. Перед обновлением закройте приложение и сделайте резервную копию SQLite. Не копируйте credential store на другую машину: настройте токен заново в Credential Manager.
