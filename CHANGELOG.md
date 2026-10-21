# NetPulse 3.0.4

- Чтение статистики выполняется в отдельном потоке без блокировки команд подключения. Фоновые запросы объединяются и идут не чаще раза в 5 секунд; явное обновление Playground ограничено выбранным интервалом. При ошибке сохраняются последние счётчики с пометкой недоступности.
- Список соединений ограничен по размеру ответа и числу сохраняемых подробных записей. Монитор не закрывает соединения через API и не сбрасывает сеть.
- Служба проверяет маршруты не чаще раза в секунду, повторные ответы используют результат проверки. Ошибка чтения маршрутов не считается подтверждённым исчезновением TUN.
- При таймауте службы активная сессия отображается как неподтверждённая, без зелёного статуса; устаревшее предупреждение убирается после восстановления связи.
- Перед сменой работающего сервера или ядра проверяется отдельный кандидат. Неудачная предварительная проверка сохраняет исходную сессию и процесс ядра. Успешная смена по-прежнему требует переподключения.
- Убраны длинные дампы Xray из сообщения проверки подключения: ограниченные очищенные подробности сохраняются в журнале ядра.
- Предпочтение IPv4 вместо принудительного IPv4 сохраняет ответы для IPv6-only доменов. Прямому буквальному IPv6-адресу всё ещё нужен работоспособный IPv6-маршрут провайдера.
- Выбор ядра в обзоре стал компактнее, без переноса sing-box и больших промежутков.

# NetPulse 3.0.3

- Исправлены поиск процессов/соединений Playground и выравнивание интервала.
- Игровой пресет сохраняет System Proxy; при TUN публичный локальный прокси не превращается в адрес с запросом пароля. Диагностические порты по-прежнему закрыты случайными паролями.

- Подбор MTU выдаёт помеченную расчётную рекомендацию и при TUN/фильтрации ICMP; измеренный PMTU показан отдельно.
- Каталог приложений applications.json с атомарной записью, нормализацией путей и группировкой известных версий; выбор не теряется при снятии галочек.
- Исправлены поле ключа, поиски, вид списка и выбор ядра. Убраны вопросы у каждой строки EXE.
- Удалён аудит обычных действий интерфейса; исправлены уровни Xray common/errors и обрезанные хвосты журнала.
- Прямой DNS-dial предпочитает только IPv4, когда выбранный физический интерфейс не имеет маршрута IPv6 по умолчанию. Буквальные IPv6-адреса не преобразуются.
- Установщик останавливает проверенную службу до обновления, переустановки и запуска старого удаления. Удаление получает ту же проверку владельца и восстановление зависшей службы.
- Добавлены проверки геометрии в готовом WebView, включая окна ввода ключа и выбора сервера; localhost не используется.

# 3.0.2

- Bounded service IPC, concurrent authenticated clients, background cached monitoring, bounded engine shutdown and verified UAC recovery.
- Shared SNI/TLS defaults, manual/optional 1/5/10s Playground refresh, setting help.
- Subscription quota/expiry metadata, validated encrypted per-key JSON overrides, irreversible editing restriction.
- Dashboard transport selector, ping icons, repeated MTU boundary checks and unused adapter toggle.
- Remove routine command spam and application-specific shortcut; populate core/service lifecycle journals.

# 3.0.1

- Подготовка ядер перенесена с потока интерфейса в фон. Ошибка доступа к ядру оставляет рабочие настройки и журналы; критические ошибки запуска показываются и сохраняются.
- Исправлено распознавание уже работающих старых окон. Новая версия больше не исчезает без объяснения при запущенной 2.x.
- Сеть вынесена в левую навигацию, DNS — в собственную вкладку. Выбранные вкладки заметно выделены; оформление стало сдержаннее.
- Убраны правая панель подключения и большой блок выбора сервера; экспериментальный автовыбор доступен в сети.
- В активности оставлен файловый журнал: автопрокрутка по умолчанию, обновление, копирование, цвета уровней и очистка всех управляемых журналов по датам.
- Подписки раскрываются группами; доступны редактирование названия/интервала/защиты ссылки и автоматическое обновление.
- Обновления клиента проверяются native worker при запуске, включая трей, и каждые 6 часов.
- Установщик переименован в NetPulse, сохраняет обнаружение 2.x, предлагает обновить/восстановить, удалить или выйти. Скопированные компоненты обязательно проверяются.
- Добавлена изолированная проверка реального WebView запуска готового EXE с ядрами и без них. Поставляемые компоненты по-прежнему не подписаны; отсутствие предупреждений антивируса не гарантируется.

# NetPulse 3.0.0

- Новый интерфейс: компактная навигация, группы серверов по подпискам, боковая панель подключения, адаптация к ширине окна.
- «Сеть» разделена на общее, TLS/транспорт, правила/DNS и Playground.
- SNI/Host и uTLS профиля, защищённое сохранение переопределений, откат к исходным настройкам подписки.
- Опциональная фрагментация Xray с диапазонами длины, интервала и maxSplit; Mux/XUDP с проверкой совместимости до остановки соединения.
- Упорядоченные правила доменов, IP/CIDR, GeoIP, Geosite и процессов; импорт/экспорт маршрутизации включает новые правила.
- Опциональный Fake IP для TUN, исключения процессов из сниффинга, прозрачные ссылки на источники GeoIP/Geosite.
- Пресет «Игры — TUN»: DoH через VPN, TUN без System Proxy; выбранные пути Roblox учитывают обновления каталога Versions. MTU и пользовательский список сохраняются.
- Playground: таблицы TCP/UDP Windows плюс сведения ядра по выбранному процессу, ограниченная история в памяти, без отдельной базы/драйвера.
- Журналы разделены по компонентам и локальным датам; вывод ошибок ядра очищается от данных профиля, размер архива ограничен.
- Проверка на локальном TLS-сервере охватывает передачу ClientHello/SNI и сертификата без изменений, фрагментацию и Mux вместе; неверное имя сертификата по-прежнему отклоняется.

Новые сетевые опции выключены по умолчанию. Точная подмена JA4/JA4H, объединение каналов, произвольный TLS-padding, фоновый веб-шум и бесшовное сохранение игровых сессий не заявляются. Ошибка Roblox на реальном игровом сервере не воспроизведена в автоматических тестах.

## 2.0.2 — правила приложений и Discord

- Исправлено сравнение путей EXE: регистр и разделители пути Windows не мешают совпадению правил белого и чёрного списка.
- Для выбранной установки Discord / PTB / Canary правило включает новые каталоги app-*, Update.exe и DiscordSystemHelper.exe. Чужие Update.exe и другие программы остаются за пределами правила. Исключения доменов сохраняют приоритет.
- Снятие галочки отключает маршрутизацию, но сохраняет запись во «Всех приложениях». Каталог сохраняется между вкладками и перезапусками; повторный поиск дополняет его. Удаление локальных данных очищает каталог.
- Добавлена проверка настоящих запросов от отдельных процессов через комплектное ядро: новая версия Discord, обновляющий процесс, helper, различный регистр и посторонние программы. Проверка изолирована в loopback и не меняет системный прокси или TUN.

# NetPulse 2.0.1

- Фоновая проверка обновлений клиента через 5 секунд после загрузки интерфейса и каждые 6 часов. Для новой версии показывается уведомление на 12 секунд и постоянная кнопка в боковой панели; обе ведут в «О NetPulse». Раздел уже содержит найденную версию и загрузку установщика. Ошибка фоновой проверки не прерывает работу клиента. Запрос к GitHub не удерживает блокировку сетевого ядра.
- System Proxy использует один адрес `127.0.0.1:порт` для HTTP и HTTPS CONNECT. Windows может отображать адрес и порт раздельно вместо строки `http=...;https=...`. Проверка владения и восстановление сохранённого состояния, включая журналы старой версии, сохранены.
- Служба TUN собирается со статическим Visual C++ Runtime. Установщик и переносная папка больше не требуют отдельного VCRUNTIME140.dll для службы. Перед упаковкой проверяются обычные и отложенные DLL-импорты клиента, службы и комплектных ядер.
- Для интерфейса по-прежнему требуется Microsoft WebView2 Runtime. При его отсутствии установщик загружает компонент Microsoft; для этой загрузки нужен интернет.

# NetPulse 2.0.0

- Служебные DEBUG команды и действия интерфейса остаются в файловом аудите, но больше не заполняют пользовательскую ленту событий. Старые такие записи также скрыты. Ошибки доступны для выделения и копирования целого события в JSON.
- Обновления ядер: явно выделено проверяемое ядро, убран дублирующий вывод версий. Версия установленного EXE и SHA-256 проверяются после активации; результат показывает переход версий, ZIP-кеш очищается, предыдущие бинарные файлы остаются для отката. Служба TUN обновляется отдельно через UAC.
- В сети добавлен экспериментальный анализ MTU: интерфейсы Ethernet/Wi-Fi/TUN, IPv4 DF пакеты и HTTPS доступность активного VPN. Таймаут не принимается за фрагментацию. При активном userspace TUN внешний DF тест пропускается для исключения ложных локальных ответов; кандидат MTU — оценка с запасом, а не обещание оптимальной скорости. Применяется только после сохранения.
- QR: поддержаны тёмные, цветные и повёрнутые изображения, инверсия и контрастные проходы. Проверен также переданный пользователем скриншот без публикации его содержимого.
- TCP пинг по умолчанию в новой установке; сохранённый выбор не заменяется. Выбор лучшего сервера помечен экспериментальным.
- Обзор показывает IP через VPN и без NetPulse, оба адреса копируются. Запросы идут к api.ipify.org только по нажатию проверки. Сторонний системный VPN может влиять на маршрут без NetPulse.
- Обычный GitHub релиз без prerelease-флага. Вложения: установщик и portable. Полный код клиента, документы, лицензии и соответствующие исходники зависимостей доступны в основном репозитории.

Измерение MTU через реальный пользовательский туннель и ручная физическая приёмка TUN/WinINET, UAC и трея не подтверждены. Статус обычного релиза не отменяет этих ограничений. Разработано HACKYASHER совместно с OpenAI Codex.

# NetPulse 2.0.0-alpha.7

- Исправлен запуск «Лучшего сервера» при активном TUN: синхронные обращения к службе выполняются в рабочем потоке, без вложенного запуска Tokio на async-потоке GUI. Внутренний сбой команды больше не отравляет общую блокировку ядра; доступна повторная остановка. Проверка отменяется при ручном выборе профиля, отключении или выходе. Если лучший сервер уже активен, ручная проверка не перезапускает его.
- Маршрутизация: домены-исключения идут напрямую перед правилами приложений во всех режимах, включая белый список. Домен включает поддомены; до 1000 правил. Для TUN используются DNS reverse mapping и ограниченный sniff TLS/HTTP/QUIC. При скрытом имени (собственный DNS + ECH) правило может не определиться.
- Импорт и экспорт отдельного JSON маршрутизации: режим, приложения, группы, исключения. Импорт проходит проверку и открывает черновик для просмотра и сохранения; TUN, System Proxy, DNS, MTU и остальные настройки не заменяются.
- При добавлении подписки можно разрешить копирование ссылки. По умолчанию она защищена; старые подписки остаются защищёнными. На диске все ссылки шифруются Windows DPAPI, включая доступные для копирования. Политика копирования сохраняется в зашифрованной резервной копии. Перед миграцией базы создаётся защищённая копия.

Физическая проверка переключения с TUN и выхода на пользовательской установке ещё требуется. Сборка остаётся alpha. Разработано HACKYASHER совместно с OpenAI Codex.

# NetPulse 2.0.0-alpha.6

- gRPC gun/multi в автоматическом режиме использует Xray; TLS gRPC объявляет HTTP/2. Два сохранённых Reality/gRPC профиля, ранее дававшие таймауты с sing-box, ответили с Xray. Ключи и адреса не включены в отчёт.
- Сетевые транспортные сокеты привязаны к исходящему интерфейсу вне userspace TUN; локальный SOCKS-мост остаётся на loopback. TUN использует system stack; пресет совместимости сохраняет mixed. Анализ протокола ограничен DNS.
- Уведомления закрываются автоматически: успех 5–6 секунд, ошибки и отчёт подписки 12 секунд. Наведение приостанавливает таймер; одинаковые повторяющиеся ошибки не продлевают уведомление бесконечно.
- Файловый журнал JSONL: команды, действия интерфейса, сохранение настроек, переходы соединения, диагностика и сбои. Ввод пользователя, ключи и пароли не записываются. Ротация 8 МиБ × 6; очистка «Активности» сохраняет файлы журнала.
- Настройки разделены на вкладки. В «О NetPulse» перенесены автозапуск и поведение окна, добавлены GitHub, журнал и обновление самого клиента. Сворачивание в трей выключено для новых установок. Выбор уровня журнала убран.
- Полная палитра фона, панелей, карточек, границ и текста; пять готовых вариантов. Случайное выделение текста интерфейса отключено; поля и диагностические данные можно копировать.
- «Лучший сервер»: 5/10/15 HTTPS-проверок каждого профиля, оценка потерь, медианы и разброса. При отключённом VPN выбирается профиль; при работающем — выполняется переключение. Отдельно включается автоматический переход после трёх неудачных проверок с интервалом 10 секунд. Переключение может оборвать текущие загрузки и соединения.
- Обновление клиента: официальные релизы HACKYASHER/NetPulse, включая предварительные; повторная проверка метаданных GitHub, размера и SHA-256. Сначала скачать, затем отдельной кнопкой завершить VPN и запустить установщик. Установка обновления вручную ещё не проверена.

Разработано HACKYASHER совместно с OpenAI Codex. Codex участвовал в коде, исправлениях, тестах и подготовке сборки.

Это alpha. Физическая приёмка TUN/WinINET, установщика через UAC, трея и автопереключения при реальном обрыве сети остаётся обязательной. Исполняемые файлы не подписаны. Короткие внешние замеры скорости колебались; гарантированное ускорение WAN не подтверждено. Локальный тест передал по 8 МиБ через настоящий gRPC/TLS в обе стороны; это не измерение скорости интернета.


# 2.0.0-alpha.5

- Separate immediate collection views/favorites from settings drafts; remove the Connections ping selector and desktop browser context menu; space subscriptions away from servers.
- Bind TCP latency sockets to a live default interface outside NetPulse TUN, retain sub-millisecond display accuracy, and refresh active-profile latency on the actual VPN runtime every 30 seconds.
- Require an explicit TUN/System Proxy choice before connecting. Explicit disconnect clears both switches. Keep failed proxy restoration leases/listeners and retry rather than abandoning recovery until GUI exit.
- Keep service cleanup ownership and journal until confirmed; retry pending route cleanup and report errors instead of silently accepting a stopped PID.
- Migrate old per-mode app lists to one shared list. Remove Direct from the UI; import old Direct as Global. Manual TUN remains explicit.
- Settings: MTU 1280–9000, DNS VPN/direct, balanced/compatibility/custom presets; move DNS out of Routing and runtime repair controls into Cores and updates; remove overview speed diagnostics.
- Installer hooks automatically install/upgrade the service with original-user SID and UAC; uninstall removes the managed service first.
- Physical TUN, WinINET, installer UAC and visual desktop acceptance remain unverified; see verification documentation.

# 2.0.0-alpha.4

- Reuse inactive Wintun adapters and wait for a closing session; preserve active/foreign interfaces. Require service/client version compatibility with an actionable upgrade message.
- Exactly two collection views: Icons and List, right-aligned picker, corrected application-row layout and legacy-view migration.
- Proxy GET/HEAD, TCP and native Windows ICMP IPv4/IPv6; best of two samples, independent method histories, double/keepalive modes and configurable HTTPS URL.
- Ping job lifecycle recovers after navigation/status errors, catches worker panics and releases controls after failure/cancellation. User-edit tracking prevents startup save-bar flashes.
- Reset manual network switches on ordinary startup while preserving explicit autoconnect intent.
- Preferred Auto/sing-box/Xray transport; bundle Xray 26.10.17 prerelease. Official GitHub stable/prerelease selection, transparent links and SHA-256, staged configuration checks, transactional rollback and independent elevated archive verification.

# 2.0.0-alpha.3

- Framed service replies stay open until client acknowledgement; retries are idempotent. Transient IPC errors retain the connection/session and stops require confirmation.
- Compact server cards, SVG flags, HTTPS-through-profile latency with failures/timeouts; TCP measurements excluded from summaries.
- Immediate connection/TUN/System Proxy/routing controls preserve unrelated drafts. Whitelist/Blacklist derive effective TUN independently of manual switches.
- Explorer views and selected-item tabs; Windows autostart/tray preferences and actions; photo QR subscriptions and profiles.
- Offline bundled verified sing-box/Xray, compatibility-gated checks, bundle repair/rollback and protected service upgrade with previous-file recovery.
- Regression coverage for delayed/fragmented Named Pipe responses, monitor/stop failures, routing combinations, offline bundle activation/rollback/tampering and measurement provenance.

# NetPulse changelog

## 2.0.0-alpha.2

Поддержка HY2 QUIC BBR fm, gRPC gun и XHTTP Reality через Xray. Служба TUN с UAC, защищёнными файлами, SID/PID/SCM и typed pipe. WinINET watchdog с DPAPI и compare-before-restore. Проверяемый restart/rollback, dirty state/navigation guard, поиск сервера/избранное, отдельные process-path списки, расширенный инвентарь EXE/иконки/виды/группы. Latency/Speed Test с отменой, payload cap и историей. Миграция SQLite 2, сохранение окна/reconnect/log level. 25 native + 3 frontend state tests. Физическая SYSTEM/TUN/WinINET-приёмка ещё требуется.

- Отдельный Rust NetworkCore вместо прежнего Python Session/Coordinator.
- Tauri/React интерфейс: пять разделов, RU/EN, Dark/Light/System/Custom, собственный pulse mark.
- Закреплённый sing-box, stdin config, SHA-256, Windows Job lifecycle, bounded sanitized diagnostics.
- Проверенные local proxy connect/switch/rollback, HTTPS readiness, настоящие counters/график/IP client.
- DPAPI/SQLite, connection imports/partial subscriptions, duplicate-field protection, strict TLS, Finalmask Salamander.
- Нативные clipboard/file/QR/EXE диалоги; local settings, logs, backup/restore/delete commands.
- Ограничения явно показаны: Windows Service/TUN, System Proxy crash recovery и применение whitelist/blacklist не готовы. Стабильный релиз заблокирован до выполнения критериев A–I.
