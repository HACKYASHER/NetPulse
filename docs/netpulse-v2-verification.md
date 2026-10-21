# NetPulse 3.0.4 — проверка

Прошли 136 тестов: 99 core, 4 GUI-native и 33 frontend; TypeScript/Vite, Clippy, 30 прогонов / 720 запросов маршрутизации. Проверены медленный и недоступный API статистики, обработку ответа на 10 000 строк с сохранением не более 300 подробных записей, сохранение последних значений, четыре волны по 32 TCP-соединения с неизменным PID ядра, отказ кандидата без остановки сессии и IPv6-only DNS.

Два запуска готового EXE WebView с ядрами и без них проверяют 16 разделов/окон, пояснения и геометрию. Шестиминутный тест изолированного IPC проверяет клиентов, которые перестают читать ответы.

Физический TUN, UAC, установка поверх действующей службы и разрыв конкретного провайдера не воспроизведены: установленная сеть не менялась. Браузерные тесты через localhost не запускались. Подписи издателя нет.

# NetPulse 3.0.3 — проверка

Прошли 128 тестов: 93 core, 4 GUI-native и 31 frontend, TypeScript/Vite, Clippy, 30 прогонов / 720 запросов маршрутизации. Проверены JSON, DPAPI, gRPC/TLS, Fake IP/DNS, отдельный каталог приложений, расчёт MTU и журналы.

Два запуска готового EXE WebView с ядрами и без них проверяют 16 разделов/окон, пояснения и геометрию поисков, ввода ключа, выбора вида, строк EXE и кнопок ядра. Шестиминутный тест изолированного IPC прошёл при клиентах, которые перестают читать ответы.

Физический TUN, UAC, установка поверх действующей службы и разрыв конкретного провайдера не воспроизведены: установленная сеть не менялась. Браузерные тесты через localhost не запускались. Подписи издателя нет.

# NetPulse 3.0.2 — проверка

Прошли 118 тестов: 87 core, 4 GUI-native и 27 frontend, TypeScript/Vite, Clippy без предупреждений, 30 прогонов / 720 запросов маршрутизации. Проверены JSON-редактирование, блокировка после старого бэкапа, метаданные подписки, DPAPI, gRPC/TLS, Fake IP/DNS и журналы.

Два запуска готового EXE WebView выполняются с ядрами и без них, в отдельных новых каталогах. В каждом автоматически открываются 14 разделов и проверяется наличие пояснений у полей. Шестиминутный тест постоянного IPC проверяет доступность при клиентах, которые перестают читать ответы.

Реальный TUN, UAC, визуальная приёмка и разрыв конкретного VPN-провайдера не воспроизведены. Исправлены найденные блокировки восстановления, но точная причина первоначального разрыва пока не установлена. Браузерные тесты через localhost не запускались. Подписи издателя нет.

# NetPulse 3.0.1 — проверка

Прошли **109 тестов**: 80 core, 4 GUI-native и 25 frontend. Дополнительно пройдены **2 запуска готового WebView EXE**: с официальными ядрами и без компонентов. Во втором случае интерфейс загрузился и оставил доступ к диагностике. Тесты использовали отдельные новые каталоги данных и WebView, без установленного клиента и localhost-сервера.

Проверены 30 последовательных прогонов и 720 запросов маршрутизации от отдельных EXE в белом/чёрном списке. Прошли gRPC/TLS-передача, сохранение ClientHello и сертификата, Fake IP/DNS, редактирование/резервное копирование подписок, очистка только управляемых логов, остановка опросов, настройки и шифрование DPAPI.

Прошли TypeScript/Vite, Clippy, self-test готовых клиента и службы, PE-аудит зависимостей. Проверка установщика принимает официальный комплект и отклоняет отсутствующие ядра.

Установка поверх реального клиента с UAC, физический TUN, визуальная приёмка при разных масштабах Windows и воспроизведение Roblox на игровом сервере не проверялись. Подписи издателя нет; отсутствие предупреждений антивируса не гарантируется. Браузерные тесты через localhost не запускались.

# NetPulse 3.0 — проверка

Прошёл **101 автоматический тест**: 77 core, 3 GUI-native, 21 frontend. Дополнительно пройдены 30 последовательных прогонов маршрутизации и 720 запросов от отдельных EXE: Discord, Roblox, обновлённые версии, чужие процессы, белый и чёрный список.

Реальные локальные TLS-проверки охватывают HTTP CONNECT/SOCKS, сохранение ClientHello/SNI и исходного сертификата, отказ при неверном имени, внешний TLS VPN с фрагментацией, Mux и обеими функциями одновременно. Проверены Fake IP/DNS-перехват с собственным UDP DNS и реальные ответы для доменных исключений. Также прошли проверки конфигураций комплектными ядрами, TypeScript/Vite, Clippy, self-test готовых клиента/службы и аудит PE-зависимостей.

Физические TUN/UAC/обновление поверх работающего клиента, визуальная приёмка при разных масштабах Windows и воспроизведение Roblox на реальном игровом сервере не выполнены. Устранение всех причин SslConnectFail и рост скорости не гарантируются. Браузерные UI-тесты не запускались согласно ограничению пользователя. EXE не подписаны. Результаты: [VERIFICATION.json](VERIFICATION.json), [ручная проверка Windows](WINDOWS-CHECK.md).

# NetPulse 2.0.2 — проверка

Прошли 86 тестов: 65 core, 3 GUI-native, 18 frontend. Проверка настоящих запросов от отдельных исполняемых файлов покрывает новую версию Discord, Update.exe, DiscordSystemHelper.exe, различный регистр, экранирование специальных символов и отсутствие расширения правил на чужие программы. Проверены оба направления маршрутизации: белый и чёрный список. Тест использует loopback и не изменяет глобальные настройки Windows.

Дополнительно: 30 последовательных прогонов маршрутизации, 480 запросов от разных процессов, без ошибок.

Проверки каталога покрывают снятие галочки, повторное открытие и поиск, сохранение/очистку и повреждённое либо недоступное хранилище. Пройдены проверки подключения, отката, прокси HTTP/CONNECT, именованных каналов службы, QR, обновлений, Clippy, TypeScript/Vite и self-test готовых клиента и службы. PE-зависимости проверены.

Физические TUN/UAC/установка поверх работающего клиента и воспроизведение сбоя Discord на компьютере друга не проверены. Исправления подтверждают перечисленные ошибки правил и списка, но не гарантируют устранение всех причин падения NetPulse или Discord. Браузерные UI-тесты не запускались согласно ранее установленному ограничению пользователя. EXE не подписаны.

# NetPulse 2.0.1 — проверка

Прошёл 81 тест: 63 core, 3 GUI-native, 15 frontend. Новая изолированная WinINET-проверка подтвердила HTTP и HTTPS CONNECT через общий loopback endpoint; она не изменяет системный прокси. Три проверки фонового обновления покрывают задержку запуска, повторение, отсутствие перекрывающихся запросов, завершение окна и восстановление после отсутствия сети. Clippy, TypeScript/Vite и self-test готовых клиента и службы прошли.

PE-аудит обычных и отложенных импортов NetPulse.exe, NetPulseService.exe, sing-box.exe, xray.exe и libcronet.dll не обнаружил зависимостей от отдельно устанавливаемых VCRUNTIME/MSVCP DLL. Служба собрана с `+crt-static`. WebView2 остаётся внешним требованием интерфейса; стандартный установщик загружает его при отсутствии.

Физическая установка через UAC, окно настроек прокси Windows и восстановление глобального WinINET на устройстве не проверялись в этом прогоне, чтобы сохранить текущее подключение. Исправление зависимости службы не доказывает устранение всех возможных причин падения окна. EXE не подписаны. Точные результаты и отпечатки: VERIFICATION.json.

# NetPulse 2.0.0 — проверка

Регрессии: DEBUG-аудит не заполняет ленту и не скрывает старую ошибку; событие загружается для копирования; фактические версии EXE и очистка только собственных ZIP; алгоритм DF не считает потерю доказательством MTU; настоящий DF echo к loopback; TCP default с сохранением пользовательского выбора; цветной/инвертированный/повёрнутый QR и отдельная приватная проверка предоставленного снимка. Полный цикл официального обновления во временной папке подтвердил Xray 26.3.27 -> 26.10.17, оба ядра, удаление ZIP, независимую проверку для службы и откат. Системная сеть не менялась.

MTU: ICMP ограничен текущим IPv4 маршрутом; при активном NetPulse TUN DF измерение пропускается. HTTPS-доступность не является PMTU или тестом пропускной способности. Кандидат MTU с запасом 80 байт — явная оценка, требует ручного применения и проверки. ICMP блокировка даёт неопределённый результат. Браузерная приёмка не выполнялась по сохранённому ограничению пользователя. Физическая приёмка TUN/WinINET, установки через UAC, трея и нового MTU на устройстве остаётся необходимой. EXE не подписаны.

# NetPulse 2.0 alpha.7 — проверка

Прошли 70 тестов: 56 core, 2 GUI-native (QR и async selection), 12 frontend. Исправление вложенного Tokio проверено отдельной регрессией; искусственный panic больше не отравляет mutex ядра и не блокирует повторное отключение. Настоящий HTTP запрос по исключённому домену проходит напрямую, а контрольный запрос идёт через недоступный proxy и завершается отказом. Миграция, DPAPI и восстановление политики копирования подписок проверены. Физический сеанс TUN/WinINET на установленном клиенте не изменялся; ручная приёмка переключения, остановки и выхода ещё требуется.

# NetPulse 2.0 alpha.6 — проверка

Точные результаты новой сборки находятся в release-v2/VERIFICATION.json. gRPC/TLS проверяется настоящими ядрами на owned loopback: 8 МиБ download и 8 МиБ upload с проверкой содержимого. Updater проверен на официальном установщике alpha.5 без запуска установки. Автопереключение и новая скорость физического TUN пока не прошли ручную приёмку.

# NetPulse 2.0 alpha.5 — проверка

- **43 native tests passed; 0 failed; 0 ignored.** Только собственные локальные fixtures и обезличенные параметры.
- Реальный HTTPS/proxy: connect, switch, отказ профиля/rollback, отказ SQLite с восстановлением прежнего runtime, копируемый endpoint, counters, disconnect, engine-exit detection.
- 36 sing-box checks: 9 профилей × 4 route modes, включая HY2 QUIC BBR и gRPC Reality gun. Конфигурации TUN не применяются.
- Два Xray checks: XHTTP Reality auto и gRPC multi. Внешняя доступность серверов этим не проверяется.
- HTTPS Speed Test: настоящие download/POST, payload cap, положительные throughput/idle RTT и отмена. Тестовый CA только в cfg(test) client, не в Windows.
- Настоящий уникальный Named Pipe: impersonation/SID, PID/process handle, отказ неверному SID и лишним полям/командам/SDDL injection. Служба не устанавливается.
- Windows Job hard-crash останавливает собственный движок.
- TUN readiness требует живой интерфейс и default routes обеих IP-семей; наличие только on-link адреса или одной половины маршрутов не принимается за готовность.
- DPAPI corruption/roundtrip, persistence/dedup, зашифрованный migration backup, отказ будущей схеме, encrypted history, atomic backup/restore failure.
- Parser: empty/Salamander/QUIC BBR fm, gRPC gun, XHTTP auto, partial reports, strict TLS, duplicate JSON/YAML aliases, отсутствие исходных ключей в ошибках.
- **9 frontend state tests passed:** нормализация, реальные изменения/возврат значения, отделение нативной геометрии окна.
- Workspace Clippy all targets с запретом предупреждений и TypeScript/Vite build проходят. Финальные packaged smoke GUI/service фиксируются в VERIFICATION.json.
- Native receipt закрепляет SHA-256 ядра/manifests/lock и отвергает изменения во время/после тестов.

Не выполнена физическая приёмка SYSTEM/UAC, обычных пакетов IPv4/IPv6/UDP/DNS через TUN, восстановления адаптера/DNS/маршрутов после падения службы, WinINET apply/restore/crash recovery, внешних предоставленных серверов и смены публичного IP. Реализация не равнозначна подтверждённой работоспособности этих сценариев. Сборка alpha.5; stable_ready=false.

Браузерные/визуальные тесты localhost не запускались по ограничению пользователя. Восемь новых скриншотов использованы как требования. Инструкция физической проверки: netpulse-v2-windows-check.md. Автообновление подписок по расписанию пока отсутствует; ручное обновление доступно.


Регрессии, добавленные в alpha.4: production ACK после полного чтения ответа в уникальном Named Pipe (фрагментация, задержка, 6 переподключений); сохранение runtime/session при IPC timeout и отказе stop; все сочетания ручных переключателей и режимов маршрутизации; раздельные результаты разных методов пинга; offline установка реальных ядер, repair/rollback и отказ подменённому файлу; сохранение постороннего черновика при немедленном переключении сети.

Ядра включены в установщик. Доступны официальные стабильные и предварительные версии: API метаданные, SHA-256 архивов, контроль компонентов, проверка конфигураций и откат. Ошибка проверки/активации не должна заменять рабочие файлы; предыдущий комплект сохраняется. Полная ручная приёмка TUN+System Proxy, обновления существующей службы, трея, автозапуска и фотоимпорта ещё не выполнена.

Фотоимпорт: дополнительный GUI-native тест декодирует две настоящие PNG QR fixtures, отличает подписку от ключа и отклоняет повреждённое фото. Windows File Dialog и действия трея вручную не проверены.


Alpha.4: регрессии неактивного/активного Wintun; сброс ручных переключателей при обычном запуске с сохранением autoconnect; GET/HEAD double/keepalive через собственный HTTPS proxy fixture с проверкой числа TLS-соединений; реальный ICMP IPv4/IPv6 loopback. Frontend: штатное завершение/отмена/ошибка проверки и отсутствие save prompt при программной инициализации.

Отдельно выполнен production-path smoke официального обновления: список релизов Xray/sing-box, скачивание обоих официальных архивов, SHA-256, извлечение и конфигурации, независимая проверка записей доверия для службы и rollback. Использовался новый временный каталог; системная сеть и установка службы не изменялись. Новые Xray конфигурации проверены на настоящем Xray 26.10.17, включая Reality/XHTTP/gRPC и выбранные TCP/WS/Trojan/SS/SOCKS/HTTP.


## Alpha.5 acceptance scope

TCP diagnostics set IP_UNICAST_IF (network byte order) / IPV6_UNICAST_IF (host order) on each non-loopback socket. Best live default route excludes NetPulse, loopback and all detected userspace TUN adapters, including foreign sing-tun/Wintun adapters. Other OS interception may still affect routing. ICMP uses Windows echo APIs and current routing. Latency is not fabricated or clamped; values below 1 ms show <1 ms. HY2 is UDP, so TCP reachability does not verify HY2; use HTTPS through profile for protocol validation.

Active-profile HTTPS latency uses the authenticated diagnostic VPN inbound of the current runtime, not an isolated connection or the original startup readiness time. Two bounded requests, 30 second refresh; method comes exclusively from Settings.

View/favorite writes touch only saved UI fields and merge into draft/baseline without a network restart or save prompt. Unrelated drafts remain pending. Old application lists migrate once into a deduplicated shared list; process rules require traffic to reach NetPulse (TUN or a proxy-aware application).

Proxy restoration retries transient errors while retaining the lease and live listener. Service Stop retains ownership/journal on cleanup failure; Status exposes the error and poll retries pending cleanup. These are code-level fixes; the reported physical adapter failure is not claimed reproduced or conclusively resolved without a real Windows TUN/WinINET acceptance run.

Automatic service setup uses Tauri NSIS hooks in the original user context and the existing UAC helper. The helper captures the user's SID before elevation, verifies bundled runtime hashes, upgrades existing managed installations and waits for readback. Runtime installer/UAC tests have not run on the user's existing installation.

References: https://learn.microsoft.com/en-us/windows/win32/winsock/ipproto-ip-socket-options ; https://learn.microsoft.com/en-us/windows/win32/winsock/ipproto-ipv6-socket-options ; https://sing-box.sagernet.org/configuration/dns/server/https/ ; https://v2.tauri.app/distribute/windows-installer/


Alpha.5 also asks the independent proxy watchdog to restore immediately after local restoration failure, instead of waiting for GUI death. The listener and lease remain available while restoration is unresolved; the worker preserves foreign proxy state. A deliberate disconnect suppresses automatic reconnect and clears the saved manual switches after confirmed cleanup.

New actual runtime checks: 18 combinations of DNS UDP/TLS/HTTPS, direct/VPN resolver detours and Global/Whitelist/Blacklist process routes at MTU 1400. Existing active authenticated diagnostic inbound is exercised for two HTTPS samples without replacing the live engine. New shared-list migration test proves removed paths are not resurrected on the next normalization. UI regression proves immediate views/favorites preserve unrelated MTU/DNS/application drafts and do not create unsaved state.

A live read-only investigation found a second sing-tun VPN adapter. Production TCP smoke demonstrated sub-millisecond local accept there; the diagnostic selector now excludes foreign userspace TUN as well as NetPulse and has a regression for a foreign TUN with a better route metric. No foreign adapter or process was stopped.


## Alpha.7 regression scope

- A Tauri async selection start dispatches synchronous IPC work to spawn_blocking; a nested private Tokio runtime regression verifies the dispatch.
- An injected command panic while holding the core mutex is caught before the guard unwinds; cleanup and later disconnect still work without poisoned mutex.
- Domain exceptions precede process rules in all three route modes; bundled sing-box validates DNS reverse mapping, suffix rules and bounded sniff. An owned HTTP fixture verifies the excluded hostname reaches direct while other traffic fails through an unreachable proxy. No OS route or WinINET changes.
- Routing-only JSON validates schema/version/domain/path limits and preserves unrelated preferences; frontend import preserves unsaved edits and switch state.
- Schema migration protects old subscription links; copyable links stay DPAPI-encrypted; encrypted backup restores copy policy.
- Physical TUN switching/stop/exit and native dialogs remain unverified on the installed client. Browser localhost tests remain disabled at the user's request.
