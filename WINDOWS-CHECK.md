# Приёмка NetPulse 3.0.4 на Windows

Автоматические проверки используют только собственные локальные процессы, сокеты, синтетические профили и отдельный IPC. Установленная служба, TUN, системный прокси и DNS не менялись.

1. На тестовом устройстве обновить 3.0.3 → 3.0.4, убедиться в совпадении версий клиента и службы. Проверить TUN + System Proxy под нагрузкой не менее 15 минут.
2. При задержке службы статус должен стать неподтверждённым; после ответа предупреждение исчезает. Проверить отключение и восстановление UAC на тестовой установке.
3. Во время работающего подключения выбрать недоступного кандидата: исходный сервер и PID ядра должны сохраниться. Успешная смена может разорвать старые сессии.
4. Проверить статистику при большом числе соединений и Playground с интервалами 1/5/10 секунд; закрытые сессии не должны требовать массового сброса.
5. Проверить компактные кнопки ядра и отсутствие переноса sing-box при масштабе Windows 100/125/150%.
6. Для IPv6-only назначения проверить наличие IPv6 у физического провайдера; ошибка прямого IPv6 без маршрута не означает отказ всего VPN.

# Приёмка NetPulse 3.0.3 на Windows

Дополнительно: проверить игровой пресет при включённых TUN + System Proxy и при одном TUN. Адрес 127.0.0.1 не должен требовать пароль; правила приложений сохраняются. В Playground поиск процесса должен работать до наблюдения, фильтр соединений — по истории; интервал находится на одной линии с подписью.

Автотесты используют отдельные каталоги, процессы и локальные тестовые подключения. Установленная служба, TUN, системные прокси и DNS не меняются.

1. На тестовом устройстве проверить обновление 3.0.2 → 3.0.3, повторную установку и удаление при работающей службе. Отказ UAC должен прервать операцию до замены файлов. Старые данные должны сохраниться.
2. Включить TUN и System Proxy на 15 минут; сохранить журнал при повторении обрыва. Исходная проблема у провайдера пока не воспроизведена.
3. Подбор MTU: при TUN и недоступном ICMP должен появиться кандидат с подписью «расчётная рекомендация»; измеренная граница не выдумывается. Без TUN проверить DF до своего VPN-сервера.
4. Каталог приложений: сканировать дважды, перезапустить, снять/вернуть выбор, обновить версию приложения. Известные версии одной установки — одна строка; разные установки одноимённых EXE должны оставаться раздельными. Проверить applications.json и восстановление выбранных путей из старого бэкапа.
5. Проверить вид списка, поиск, окно ключа, выбор сервера, выбор ядра и общую подсказку списка EXE при разной ширине окна.
6. Журналы не содержат кликов/ввода/переходов; gRPC deprecated — WARNING, реальные ERROR сохраняются; обрезанные хвосты не повторяются как INFO.
7. При IPv4-only физическом подключении проверить прямой домен с A+AAAA и проксирование IPv6. Литеральный IPv6 без прямого IPv6-маршрута остаётся недоступным.

# Приёмка NetPulse 3.0.2 на Windows

Автоматические тесты не изменяют установленную службу, TUN, прокси и DNS. На устройстве остаются следующие проверки:

1. Обновить предыдущий выпуск с сохранением данных; версии установщика, клиента и службы — 3.0.2.
2. Проверить TUN, System Proxy и оба режима вместе не менее 15 минут под нагрузкой; при разрыве сохранить журналы ядра и службы.
3. При таймауте проверить обычное отключение, затем «Восстановить службу (UAC)». Чужие процессы и маршруты должны сохраняться.
4. Проверить общие SNI/TLS на совместимых серверах, Hysteria2 и Reality. Несовместимые параметры должны отклоняться до остановки текущего соединения.
5. В Playground проверить ручное обновление, интервалы 1/5/10 секунд, завершение процесса и остановку при уходе со вкладки.
6. Проверить подсказки мышью и с клавиатуры, выбор ядра в обзоре, пинг-значки.
7. На тестовой подписке проверить остаток трафика и дату из Subscription-Userinfo; без заголовка значения должны быть неизвестными.
8. Проверить JSON-редактор, отказ при неверном JSON, сохранение после обновления. На временной подписке проверить необратимый запрет и восстановление старого бэкапа.
9. Проверить MTU при отключённом TUN, затем с TUN. Неактивные адаптеры скрыты до нажатия кнопки.
10. Проверить сообщения жизненного цикла ядра/службы, отсутствие рутинного спама, копирование, цвет уровней и очистку журналов.

# Приёмка NetPulse 3.0.1 на Windows

1. Завершить старый клиент через «Выход» в трее. Проверить, что установщик показывает 3.0.1 и предлагает обновить/восстановить с сохранением данных, удалить либо оставить текущую версию.
2. После обновления убедиться, что сохранены подписки/настройки, появился отдельный пункт «Сеть» и корректно выделяются вкладки. Установку службы проверить с обычным UAC.
3. Проверить запуск без ядер: интерфейс должен открыться с сообщением о недоступном компоненте. Затем проверить восстановление из проверенного официального комплекта по кнопке. Не отключать защиту ради теста.
4. В активности проверить автопрокрутку, её отключение/сохранение выбора, копирование ошибки, цвета и очистку всех категорий/дат. Очистка удаляет прежнюю диагностику.
5. В подписке изменить название/интервал/защиту ссылки; убедиться, что активный сервер сохраняется. Проверить повторный запуск, автообновление и отказ копирования защищённой ссылки.
6. Проверить автозапуск в трей и уведомление о новой версии, затем реальный TUN/System Proxy на устройстве. Локальные тесты не заменяют проверку Roblox/провайдера и обновления с UAC.
7. При блокировке компонентов сохранить точное обнаружение и SHA-256 из истории антивируса. Контрольная сумма не означает гарантированную безопасность; подписанного издательского сертификата пока нет.

# NetPulse 3.0 — дополнительные ручные проверки

Автоматические тесты используют собственные локальные серверы и не изменяют рабочее VPN-подключение. До утверждения физической сетевой совместимости проверить на отдельной Windows x64:

- Установка поверх 2.0.2, существующая база и подписки, обновление службы через UAC, сохранение старого uninstall-ключа и корректное имя NetPulse 3.0 в списке программ.
- Вид двух панелей при 960/1240/1920 px, масштабировании Windows 100/125/150%, светлой и тёмной теме. Проверить фокус, клавиатуру, отсутствие бокового переполнения.
- TUN, System Proxy и совместный режим: подключение/отключение, смена профиля, восстановление маршрутов, DNS и прокси после ошибки и выхода.
- Roblox: сохранить пресет «Игры — TUN», добавить игру/загрузчик при белом списке, перезапустить игру; сравнить с другим клиентом на том же сервере. Проверить UDP, отсутствие бесконечного обновления, сопоставить ошибки с журналом Roblox. Наличие SslConnectFail само по себе не доказывает подмену сертификата.
- SNI/Host/uTLS: настройка и сброс, неверное имя сертификата, ограничения Reality/Hysteria2, сохранение при перезапуске, обновлении неизменённой записи подписки и восстановлении копии. Активный профиль переподключается.
- Фрагментация/Mux: включать только при поддерживаемом профиле, проверять скорость и UDP на собственном сервере, несовместимая настройка не должна останавливать старое соединение.
- Fake IP: A/AAAA через TUN, доменные исключения, реальное DNS-разрешение для CIDR/GeoIP, перезапуск приложений после изменения DNS/Fake IP; отключение и восстановление обычного интернета.
- Внешние правила: GitHub доступен и недоступен, неизвестное имя набора, DNS-ошибка, приоритет правил относительно списка приложений, отсутствие бесконечного ожидания загрузки.
- Playground: наблюдение своего процесса и нескольких экземпляров одного EXE, завершение и повторный запуск/PID, TCP/UDP IPv4/IPv6, быстрая смена раздела во время опроса, отсутствие опроса после остановки. Сверить метаданные с реальными сокетами; UDP-адреса и байты доступны только ядру для захваченных соединений.
- Журналы: локальная полночь, смена дня, секции и копирование, ротация/лимит 128 МиБ, редактирование дат/путей отклоняется. Пароли/UUID/URL подписок не должны попадать в файл.

Ниже — прежний список проверок, который также остаётся применимым.

## Проверка исправлений 2.0.2

1. В белом списке добавить Discord.exe, сохранить и перезапустить Discord. Проверить обновление и загрузку; отдельно повторить в TUN и с TUN + System Proxy.
2. После обновления Discord до нового app-* проверить совпадение правила без повторного добавления. Update.exe и DiscordSystemHelper.exe охватываются только внутри выбранной установки Discord; произвольные EXE не включаются автоматически.
3. Снять галочку в «Добавленных», перейти во «Все приложения», вернуть галочку. Повторить после перехода на другую вкладку и перезапуска клиента. Повторный поиск не должен удалять ранее добавленные записи.
4. Повторить с чёрным списком и исключением домена: исключение домена должно иметь приоритет.
5. При вылете самого NetPulse сохранить журнал из «Активности» и сведения Windows об ошибке. Изолированная проверка маршрутов не доказывает устранение всех причин падения приложения или проблем на конкретном сервере.

# NetPulse 2.0.1 — proxy and runtime acceptance

- With a newer public GitHub release available, wait five seconds after startup: an update notice must appear and its button must open Settings → About NetPulse with that release ready to download. Dismiss the notice or wait 12 seconds: the sidebar update button remains. Repeat offline and with slow GitHub responses: connection controls must remain available. Downloads and installation require an explicit action.
- Install the new setup after exiting the previous client. The installer upgrades the service through normal Windows UAC. WebView2 remains required; the default installer downloads it from Microsoft if it is missing.
- With System Proxy enabled, Windows Settings must show address `127.0.0.1` and the configured port separately. Verify HTTP and HTTPS browsing, then disconnect and confirm the previous proxy/PAC/bypass settings return. Old recovery journals retain their original states and comparison rules.
- Packaging inspects ordinary and delayed PE imports for the GUI, service, sing-box, Xray and libcronet. A separately installed Visual C++ Redistributable must not be required by those binaries. The service is linked with `+crt-static` during packaging.
- The isolated WinINET test checks HTTP requests and HTTPS CONNECT using only a loopback fixture and a per-session proxy. It does not change the user's Windows proxy. Physical Windows Settings, service installation and disconnect/crash restoration still require device acceptance.

# NetPulse 2.0 alpha.7 — additional acceptance

- With TUN active, select best server; verify GUI remains responsive, stop works, and tray Exit succeeds. Repeat with System Proxy and both switches; cancel mid-selection and manually choose a server. Confirm the cancelled job does not override the manual choice.
- Add a domain exception for an owned test destination while its application is whitelisted. Verify it uses the physical route; other traffic follows the whitelist. Test ordinary DNS and visible TLS names; hidden ECH/private DNS may prevent domain identification.
- Export routing JSON, change a rule, import the file and review the draft. Save/cancel; confirm unrelated settings and TUN/Proxy switches are unchanged.
- Add disposable protected/copyable subscriptions, verify copy is enabled only for the latter, then back up/restore. Existing subscriptions must remain protected after migration.

# NetPulse 2.0 alpha.5 — Windows acceptance

These physical checks are pending. Automated tests use owned loopback fixtures and config checks; they do not modify the user's adapters, service or WinINET settings.

1. Exit the previous client from the tray. Run NetPulse-Setup.exe. Confirm the Windows UAC request for automatic TUN service install/upgrade. The installer must report failure/cancellation clearly. The GUI runs as the current user; the service's protected files reside in Program Files/NetPulse Core 2. A portable installation can repair the service in Settings → Network.
2. Restart without auto-reconnect. TUN and System Proxy must be off, and no Save prompt should flash. Press Connect with neither enabled: select TUN and/or System Proxy before any traffic is redirected. Both may be enabled together. Disabling both stops the session.
3. Connect/disconnect repeatedly with TUN, System Proxy, then both. The previous WinINET/PAC state and normal IPv4/IPv6/DNS connectivity must return immediately with the client still open. Switch servers; check no repeated bind-port or live-adapter conflict. A cleanup failure must be visible and retain recovery ownership for retry.
4. Check proxy recovery on deliberate disconnect and after terminating only the owned GUI. If another application changed proxy settings, preserve its new settings. Do not terminate foreign processes or clear global routes/DNS to perform acceptance.
5. Settings → Ping: select GET, HEAD, TCP and ICMP; then test selected server/list. TCP must not measure a local NetPulse TUN SYN response. ICMP follows current Windows routing. HY2 uses UDP; a TCP timeout does not prove HY2 is broken. GET/HEAD must verify the actual profile with TLS/authentication. Test cancel/retry/navigation; controls must become usable after completion/error. Old pre-alpha.5 TCP measurements are hidden until retested.
6. Overview ping refreshes on the current active runtime every 30 seconds and manually. Check method/timestamp/error and reset after disconnect/server switch. The startup readiness time must not be presented as a current ping. Speed diagnostics have been removed from Overview.
7. Connections/Routing/server picker: only Icons (Значки) and List (Список), selector on the right. View changes and stars save immediately without a Save prompt; preserve pending appearance/DNS/app edits. Check stars in Connections and the picker, compact EXE alignment, server/subscription spacing, no default browser right-click menu. Copy buttons and Ctrl+C remain available.
8. Routing: one shared app selection across Global, Whitelist and Blacklist. Whitelist sends selected apps through VPN; blacklist bypasses VPN for them; Global sends all apps through VPN. Child EXEs must be selected separately. TUN enables process routing for all traffic; System Proxy covers only proxy-aware apps. Changing the mode must not secretly enable TUN. Existing separate lists migrate once without reviving removed entries.
9. Settings → MTU, DNS and presets: test balanced (MTU 1500, DoH via VPN), compatibility (1400, direct DoH) and custom. Invalid MTU is rejected. MTU changes apply only to NetPulse's adapter. Test UDP/DoT/DoH and both DNS detours; system DNS and DNS-server/bootstrap hostname resolution use Windows. DNS is absent from Routing. Save active network edits with validation/rollback.
10. Settings → Cores and updates: Xray/sing-box versions, official GitHub page/archive/SHA, stable and prerelease selection, offline bundled repair and rollback. Updates validate configurations before activation. Privileged service copies are updated separately through the service UAC repair/upgrade control after an in-client core update. Repair controls are absent from Network.
11. Verify Windows autostart, tray minimize/close, tray Exit cleanup and QR import from owned test photos. Browser localhost tests remain disabled by the user's restriction.

The application's signatures are not configured. Physical UAC/TUN/WinINET/tray/installer behavior and user-supplied remote servers have not been claimed verified.
