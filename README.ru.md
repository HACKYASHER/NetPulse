> Приложение, ядра, служба и личные данные хранятся в выбранной папке NetPulse. [Расположение файлов и перенос профиля](docs/ru/INSTALLATION.md)

# NetPulse 1.0.8

[<kbd>English</kbd>](README.md)

Нативная Windows x64 версия NetPulse на C++ / Qt 6.8.3 / QML.
Сетевой контроллер на Rust работает в отдельном процессе `netpulsed.exe`.
За режим TUN отвечает служба `NetPulseService.exe`.

[Скачать установщик](https://github.com/HACKYASHER/NetPulse/releases/download/v1.0.8/NetPulse-Setup.exe)
· [Скачать portable ZIP](https://github.com/HACKYASHER/NetPulse/releases/download/v1.0.8/NetPulse-Portable.zip)
· [Изменения в 1.0.8](docs/ru/releases/v1.0.8.md)
· [Документация](docs/ru/README.md)

## Запуск

Установите `NetPulse-Setup.exe` или распакуйте переносную версию и откройте
`NetPulse/NetPulse.exe`. Для переносной версии сохраняйте рядом с EXE
всю папку: библиотеки Qt, `qml`, `cores` и плагины.

1. В разделе «Серверы» добавьте ключ, файл, QR-код или HTTPS-подписку.
2. Выберите сервер. Нажмите кнопку питания в «Обзоре» и выберите TUN, System Proxy или оба режима, затем подтвердите подключение.
3. Для TUN требуется служба: установщик добавляет её с подтверждением UAC.
   Её также можно установить или восстановить во вкладке «Настройки → Служба TUN».
4. При выходе через меню трея клиент отключает соединение и восстанавливает
   собственное изменение системного прокси.

Приложение и служба находятся в выбранной при установке папке: `<выбранная папка NetPulse>`, личные данные —
в её подпапке `data\<SID Windows>`. Прежний Qt-профиль из
`%LOCALAPPDATA%\NetPulseQt` переносится при первом запуске с проверкой копии.
Данные исходного NetPulse 3.0.4 не копируются автоматически. Для переноса создайте резервную копию
в 3.0.4 и восстановите её в «Настройки → Восстановить» под той же учётной
записью Windows. Это явная операция: восстановление заменяет данные Qt-клиента.
Одновременно подключать обе версии не следует: они управляют сетью одной ОС.

## Возможности

- Восемь разделов: обзор, серверы с подписками, маршрутизация, сеть,
  диагностика, Playground, активность и настройки.
- Закрепление серверов, действия через ПКМ, автообновление подписок при
  запуске и по интервалу, импорт ключей, файлов и QR.
- sing-box 1.14.4 и Xray 26.10.31 с проверкой SHA-256, обновлением и откатом.
- Маршрутизация по имени EXE, значки приложений, импорт/экспорт правил,
  отдельный routing-apps.json и исключения доменов.
- Вкладки сети с DNS, MTU, SNI/TLS, фрагментацией, multiplex и Sniffing.
- Ручные проверки пинга с сохранением результатов, Speed Test и сравнение MTU без перезапуска ядра.
- Playground с группами процессов, сетевыми счётчиками, маршрутами и
  событиями соединений; цветная консоль журналов с копированием.
- Проверка обновлений клиента при запуске и каждые шесть часов. Старый
  канал 3.0.4 исключён из обновлений новой архитектуры.
- SQLite/DPAPI, резервные копии, расширенный трей, автозапуск и темы RU/EN.

## Структура

```text
src/                         C++ окно, трей и частный транспорт к контроллеру
qml/                         QML экраны и общие компоненты
backend/crates/netpulse-core  Сохранённое сетевое ядро Rust
backend/crates/netpulse-host  Отдельный контроллер, типизированные команды
backend/crates/netpulse-service  Windows-служба TUN
resources/cores/              Проверенные sing-box, Xray, libcronet
scripts/                     Сборка, упаковка и проверки
licenses/                    Лицензии и сведения о происхождении
release/NetPulse/             Готовая переносная версия
release/NetPulse-Setup.exe Установщик
```

## Сборка из исходников

Нужны Qt 6.8.3 MSVC x64, MSVC 2022 и Windows SDK, Rust 1.89 или новее,
Python, CMake и Ninja. Для скрипта загрузки Qt дополнительно нужен `py7zr`.

```powershell
python -m pip install cmake ninja py7zr
python scripts/fetch-qt.py
python scripts/fetch-cores.py
python scripts/fetch-engine-sources.py
# В Developer PowerShell для Visual Studio:
python scripts/build.py --qt .tools/Qt
python scripts/package.py
```

`QTDIR` задаёт иной SDK Qt. `NETPULSE_TOOLCHAIN` позволяет использовать переносной
Rust/MSVC SDK. Параметр `--cargo-target` позволяет повторно использовать кеш сборки.
Установщик собирается NSIS 3 (`NSIS_EXE` либо `makensis` в PATH).

```powershell
python scripts/test-ipc.py
release/NetPulse/NetPulse.exe --smoke-test C:/Temp/NetPulseQt-smoke
cd backend
cargo test -p netpulse-core --lib
```

Проверка интерфейса открывает восемь разделов в изолированном профиле.
Результаты тестов и ограничения стенда перечислены в отчёте.
[Проверки выпуска](docs/ru/VERIFICATION.md)

## Лицензия

Код NetPulse — GPL-3.0-or-later, как в исходном релизе. Qt подключается динамически;
его DLL можно заменить совместимой сборкой. Исходники клиента и изменения
контроллера поставляются отдельно. Лицензии, исходные проекты и версии —
в `THIRD-PARTY.md`, `licenses/` и `cores/source-manifest.json`.
