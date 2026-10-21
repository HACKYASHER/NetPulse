# Сборка NetPulse 2.0 на Windows

Полный код клиента: `apps/desktop-v2/ui` (React), `apps/desktop-v2/src-tauri/src` (команды и окно), `apps/desktop-v2/crates/netpulse-core/src` (сеть, маршруты, хранение, ядра), `apps/desktop-v2/crates/netpulse-service` (служба TUN). Исходники оригинальных ядер и зависимостей: `third-party/sources`, лицензии: `third-party/licenses`. Папки `target`, `node_modules`, локальные базы и конфигурация пользователя в репозиторий не входят.

Нужны Windows x64, Rust/MSVC (актуальная стабильная версия, сборка проверена на Rust 1.99.1), Visual Studio C++ Build Tools + Windows SDK, Python 3.12+, Node.js и pnpm 11.25.0. Команды выполняются из корня в Developer PowerShell. Локальный вспомогательный SDK необязателен: `scripts/native-v2.py` использует обычные инструменты, если `.local/toolchain` отсутствует.

```powershell
python -m venv .venv
.venv/Scripts/python -m pip install -r requirements-test.txt
.venv/Scripts/python scripts/bootstrap-v2.py
.venv/Scripts/python scripts/stage-cores-v2.py
pnpm install --frozen-lockfile
.venv/Scripts/python scripts/native-v2.py cargo rustc -p netpulse-service --bin NetPulseService --release -- -C target-feature=+crt-static
Copy-Item apps/desktop-v2/target/release/NetPulseService.exe apps/desktop-v2/resources/NetPulseService.exe
.venv/Scripts/python scripts/test-native-v2.py
.venv/Scripts/python scripts/test-routing-v2.py
.venv/Scripts/python scripts/package-native-v2.py
```

`bootstrap-v2.py` скачивает только оригинальные закреплённые версии Xray/sing-box по HTTPS с официального GitHub и проверяет SHA-256 каждого EXE/DLL. Он не запускает скачанные EXE. Исходники и лицензии из репозитория копируются в промежуточные папки. Установщик создаётся Tauri/NSIS; Tauri может загрузить стандартные сборочные инструменты. Итог: `release-v2/NetPulse-Setup.exe`, `NetPulse.exe`, `cores/`, `licenses/` и служба. Для полной переносной версии нужны все эти компоненты, а не один EXE.

Тесты используют собственные loopback HTTPS/gRPC fixtures, служба в систему не устанавливается, маршруты/WinINET не меняются. Браузерные тесты localhost не запускались по ограничению пользователя. Физическую проверку смотрите в `WINDOWS-CHECK.md`. Первый запуск TUN требует установки службы через обычный UAC. Параметры пользователей и подписки в исходниках отсутствуют.

Автоматические Source code ZIP/TAR GitHub теперь содержат код клиента и соответствующие исходники зависимостей. Сборка из исходников не гарантирует побитовое совпадение PE файла при другом SDK/компиляторе; версии и контрольные суммы входных компонентов закреплены.
