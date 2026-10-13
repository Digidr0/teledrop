# Verification — TeleDrop

## 1.0.0

Обновлена версия, статистика перенесена вверх главной страницы, добавлена проверка стабильных GitHub Releases (SemVer, timeout, без Telegram-токена) и кнопка открытия релиза. ESLint успешно выполнен. Release build проверяет TypeScript/Svelte и компилирует Rust; результаты предыдущих проверок ниже относятся к ранней версии, а не являются повторным тестированием 1.0.0. Полный test suite не запускался по ранее согласованной просьбе пользователя. UI обновлений с настоящим будущим релизом пока не проверен.

## Исторические проверки 0.1.0

Дата: 2026-10-12. Windows x64, Rust stable 1.99.0 (`x86_64-pc-windows-msvc`), Node 24.12.0, npm 11.6.2. Использованы обнаруженные Microsoft Visual Studio 2019 Build Tools с C++ и Windows SDK.

## Проверено

- Frontend production build: успешно, `svelte-check` без ошибок/предупреждений.
- ESLint: успешно.
- Vitest: **5 tests passed** (темы, unknown-size progress, согласование событий с SQLite).
- Rust: **17 tests passed** (имена, коллизии, redaction, mocked Telegram, очередь, dedup, persistence, recovery, cancellation, миграции и настройки).
- Clippy all targets с `-D warnings`: успешно.
- Playwright UI: **5 tests passed**. Все пять страниц Light/Dark, actual event payload rendering, theme persistence в тестовом IPC, clearing secret field, пустая история, поиск, focus, minimum window и честный browser-without-backend state.
- Скриншоты browser-тестов: `test-results/`. Они содержат **явно тестовые fixtures**, не реальные Telegram downloads.
- Первая Windows release-сборка: успешно создан `src-tauri/target/release/teledrop.exe` и NSIS-installer `TeleDrop_0.1.0_x64-setup.exe` (около 4,19 MiB).
- Настоящий release EXE: успешно запускается; native WebView2 IPC читает SQLite; все 5 страниц рендерятся в обеих темах; theme preference переживает перезапуск; процесс корректно завершается. Проверка выполнена `scripts/smoke-desktop.mjs`, без test fixtures и без настоящего токена.
- `dumpbin /dependents`: системные Windows DLL/UCRT, отдельный VC++ runtime DLL в direct imports не обнаружен. WebView2 остаётся обязательным внешним runtime.

## Последняя сборка

После приведённых проверок добавлены безопасные точки отмены вокруг filesystem operations, атомарная проверка ёмкости очереди при retry, дополнительный polling-тест и изолированный Credential Manager integration-тест. **По просьбе пользователя новые тесты не запускались.** Их наличие в source code не означает подтверждённое выполнение.

Финальная release-сборка завершилась успешно: frontend production build (`svelte-check`: 0 errors / 0 warnings), Rust optimized EXE и NSIS installer. Артефакты находятся в `release/`; упаковка portable использует этот же EXE без повторной компиляции. README, инструкции и лицензии прилагаются. SHA-256 записывается рядом с каждым артефактом.

Installer: `release/TeleDrop_0.1.0_x64-setup.exe`.
Актуальный SHA-256 записан в соседнем файле `.sha256` после каждой упаковки.

## Дополнения Telegram

Добавлены отдельная HTTPS-проверка доступности `api.telegram.org` без токена, цветные линии прогресса в журнале, история файлов на главной, `setMessageReaction` с ⚡ после успешной записи и Windows-уведомление на `/start`. Проверка сети выполняется при открытии главной, каждые 30 секунд и вручную. Ошибка проверки не доказывает блокировку. Реальные реакции и уведомления от настоящего Telegram пока не проверены.

## Ещё не подтверждено

- Повторный полный test suite / desktop smoke после последних изменений: пропущен по просьбе пользователя.
- Изолированный native Credential Manager write/read/delete integration-тест: присутствует, но не запускался. Чтение отсутствующего app credential при desktop smoke работало; roundtrip с настоящим токеном не выполнялся.
- Реальный Telegram E2E: не выполнялся, настоящий токен не предоставлялся.
- Установка/удаление NSIS на чистой машине, SmartScreen, native notifications, hardware disk-full и внешние диски: ручной чеклист.
- Production signing, tray, Windows autostart, local Bot API и HTTP Range resume: не реализованы.

Успешные unit/UI-тесты не заменяют чистую Windows-установку и реальный Telegram E2E.
