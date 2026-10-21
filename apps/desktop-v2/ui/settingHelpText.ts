export const help = {
  autostart: [
    "Запускает NetPulse при входе в Windows. Проверка обновлений также выполняется при автозапуске.",
    "Launch NetPulse when signing into Windows, including update checks.",
  ],
  close_to_tray: [
    "Кнопка закрытия скрывает окно в трей. Для полного выхода используйте меню значка.",
    "Closing the window hides it to the tray. Use the tray menu to quit.",
  ],
  minimize_to_tray: [
    "При сворачивании скрывает окно с панели задач в трей.",
    "Minimizing hides the taskbar window to the tray.",
  ],
  auto_reconnect: [
    "Подключается к сохранённому серверу при запуске и повторяет подключение после сбоя с задержками. Текущие сессии могут оборваться.",
    "Connect to the saved server at startup and retry after failures. Existing sessions may end.",
  ],
  tun: [
    "Захватывает IP-трафик через виртуальный адаптер. Нужен для UDP и правил по приложениям. Использует службу Windows, которая устанавливается с UAC.",
    "Capture IP traffic using a virtual adapter. Required for UDP and process rules. Uses the Windows service installed through UAC.",
  ],
  proxy: [
    "Назначает локальный прокси для приложений, использующих настройки Windows. Не все приложения его поддерживают; UDP обычно требует TUN.",
    "Set the local proxy for apps using Windows proxy settings. Other apps and UDP usually need TUN.",
  ],
  sniff: [
    "Эти EXE исключаются из распознавания HTTP, TLS и QUIC. Содержимое HTTPS не расшифровывается; доменные правила могут стать менее точными.",
    "Skip HTTP/TLS/QUIC detection for these EXE paths. HTTPS is not decrypted; domain routing may become less precise.",
  ],
  app: [
    "Добавляет EXE в общий список правил. В белом списке выбранные приложения идут через VPN, в чёрном — напрямую. Правила процессов требуют TUN.",
    "Add this EXE to routing. Whitelist apps use VPN; blacklist apps bypass it. Process rules require TUN.",
  ],
  samples: [
    "Число HTTPS-проверок каждого сервера. Больше проверок лучше выявляет потери, но дольше выполняется.",
    "Number of HTTPS samples per server. More samples help reveal loss but take longer.",
  ],
  failover: [
    "После трёх неудачных HTTPS-проверок выбирает доступный резервный сервер. Переключение может разорвать TCP и игровые сессии.",
    "Select a fallback after three failed HTTPS checks. Switching can break TCP and game sessions.",
  ],
  core: [
    "Выбирает ядро транспорта VPN. Автоматически учитывает протокол. Hysteria2, TUN и правила процессов обслуживает sing-box; Xray поддерживает XHTTP, фрагментацию и Mux.",
    "Choose the VPN transport core. Automatic follows protocol requirements. sing-box handles Hysteria2, TUN and process rules; Xray provides XHTTP, fragmentation and Mux.",
  ],
  release: [
    "Версия выбранного ядра из официального репозитория. Перед заменой проверяются SHA-256 и конфигурация; VPN нужно отключить.",
    "Official core release to install. Checksum and configuration are validated. Disconnect VPN first.",
  ],
  testRoute: [
    "Сравнивает прямой маршрут и VPN либо только выбранный маршрут. Тест передаёт реальные данные.",
    "Compare direct and VPN routes or test one route. Sends real payload data.",
  ],
  duration: [
    "Время каждой фазы измерения скорости. Большая длительность повышает расход трафика.",
    "Duration of each speed-test phase. Longer tests use more data.",
  ],
  cap: [
    "Общий предел полезных данных теста. TLS, HTTP и другие служебные байты идут дополнительно.",
    "Maximum test payload. TLS/HTTP and protocol overhead are additional.",
  ],
  download: [
    "HTTPS-адрес файла для измерения загрузки. Сервер должен разрешать тестовые запросы.",
    "HTTPS download file used by the speed test. The server must allow test requests.",
  ],
  upload: [
    "Необязательный HTTPS-адрес, принимающий тестовые POST-загрузки. Не указывайте обычную страницу сайта.",
    "Optional HTTPS endpoint accepting test POST uploads; not a normal web page.",
  ],
  post: [
    "Подтверждает, что указанный сервер предназначен для загрузки тестовых данных методом POST.",
    "Confirm that this endpoint is intended to accept test POST data.",
  ],
  traffic: [
    "Разрешает тесту расходовать выбранный объём трафика; служебные байты не входят в лимит.",
    "Allow the selected test payload amount. Protocol overhead is extra.",
  ],
  tail: [
    "Автоматически обновляет текущий журнал и прокручивает вниз. При выключении сохраняет позицию чтения. По умолчанию включено.",
    "Refresh and follow the log tail. Disable to keep the reading position. Enabled by default.",
  ],
  name: [
    "Локальное название для удобства поиска. Не меняет адрес сервера или данные авторизации.",
    "Local display name for searching. Does not change server address or credentials.",
  ],
  url: [
    "HTTPS-ссылка подписки от провайдера. Может содержать секрет доступа; хранится зашифрованной средствами Windows.",
    "Provider HTTPS subscription URL. May contain an access secret; encrypted using Windows.",
  ],
  protect: [
    "Запрещает копирование ссылки через интерфейс клиента. Это локальная настройка, а не защита от владельца компьютера.",
    "Disable copying the URL through this client. A local restriction, not protection against the computer owner.",
  ],
  protocol: [
    "Протокол должен совпадать с конфигурацией VPN-сервера. Смена одного названия не преобразует сервер.",
    "Must match the VPN server protocol. Changing this value does not convert the server.",
  ],
  host: [
    "Домен или IP VPN-сервера без схемы, пути и порта.",
    "VPN server domain or IP, without a scheme, path or port.",
  ],
  port: [
    "Порт VPN-сервера от 1 до 65535, указанный провайдером.",
    "VPN server port, 1–65535, supplied by the provider.",
  ],
  username: [
    "Имя пользователя для SOCKS/HTTP-аутентификации, если сервер её требует.",
    "SOCKS/HTTP username when server authentication requires it.",
  ],
  secret: [
    "UUID или пароль из настроек сервера. Хранится зашифрованным и не выводится в журнал.",
    "Server UUID or password. Stored encrypted and excluded from logs.",
  ],
  cipher: [
    "Метод шифрования Shadowsocks. Должен совпадать с сервером.",
    "Shadowsocks encryption method. Must match the server.",
  ],
  sni: [
    "Имя VPN-сервера в TLS ClientHello. Оно должно соответствовать серверу и сертификату; произвольная замена может нарушить соединение.",
    "VPN server name in TLS ClientHello. Must match server requirements and certificates; an arbitrary change can break connectivity.",
  ],
  sniMode: [
    "Из конфигурации — сохраняет SNI ключа. По адресу — использует адрес сервера. Своё имя задаёт общий SNI. Отключение поддерживается только sing-box без Reality; проверка сертификата сохраняется.",
    "Inherit the key SNI, use server address, set a shared name, or omit SNI with sing-box without Reality. Certificate validation stays enabled.",
  ],
  certName: [
    "Домен для проверки сертификата VPN-сервера, без схемы и пути. При отключённом SNI это имя всё равно нужно для проверки TLS.",
    "Domain used to validate the VPN certificate, without scheme or path. Still required when SNI is omitted.",
  ],
  httpHost: [
    "Переопределяет HTTP Host только для WebSocket, XHTTP и HTTPUpgrade. Пустое поле сохраняет значение ключа.",
    "Override HTTP Host for WebSocket, XHTTP and HTTPUpgrade. Leave blank to inherit the key.",
  ],
  utls: [
    "Выбирает реализацию внешнего TLS ClientHello. Не меняет TLS приложений внутри VPN. Hysteria2/QUIC не поддерживается; точное совпадение JA4 с текущим браузером не гарантируется.",
    "Choose the outer TLS ClientHello implementation. Does not change app TLS inside VPN. Excludes Hysteria2/QUIC; no exact current-browser JA4 match is promised.",
  ],
  proxyPort: [
    "Порт локального прокси NetPulse на 127.0.0.1. Должен быть свободен и отличаться от других локальных служб.",
    "Local NetPulse proxy port on 127.0.0.1. Must be free and not conflict with other services.",
  ],
  probe: [
    "HTTPS-адрес проверки доступности через VPN. Используется при подключении, измерении задержки и проверках резервного сервера.",
    "HTTPS URL for VPN reachability checks during connection, latency tests and failover.",
  ],
  language: [
    "Язык интерфейса и пояснений. Не меняет настройки сети.",
    "Interface and help language. Does not change network settings.",
  ],
  theme: [
    "Тёмная, светлая, системная или своя палитра интерфейса.",
    "Dark, light, system, or custom interface palette.",
  ],
  accent: [
    "Цвет выделения выбранных элементов и основных кнопок.",
    "Accent color for selected items and primary buttons.",
  ],
  surface: [
    "Базовый цвет фона для своей темы; оттенки панелей рассчитываются автоматически.",
    "Base background color for the custom theme; panel shades are derived automatically.",
  ],
  radius: [
    "Радиус скругления элементов в пикселях.",
    "Corner radius in pixels.",
  ],
  density: [
    "Меняет отступы и плотность элементов интерфейса.",
    "Adjust interface spacing and density.",
  ],
  motion: [
    "Включает анимацию переходов и переключателей интерфейса.",
    "Enable interface transition and switch animations.",
  ],
  mtuTarget: [
    "IPv4-адрес для ICMP-пакетов с запретом фрагментации. Для оценки пути VPN укажите IPv4 своего VPN-сервера и отключите TUN перед тестом. Отсутствие ICMP-ответов не доказывает малый MTU.",
    "IPv4 target for Don't Fragment ICMP probes. Use your VPN server IPv4 with TUN disconnected to estimate its path. Missing replies do not prove a small MTU.",
  ],
  preset: [
    "Применяет согласованный набор TUN, System Proxy, DNS и MTU. Пресет игр включает TUN, сохраняя выбранный System Proxy и правила приложений; изменения применяются после сохранения.",
    "Apply a preset for TUN, System Proxy, DNS and MTU. The games preset enables TUN and keeps your System Proxy choice and app rules. Save to apply.",
  ],
  mtu: [
    "Максимальный размер IP-пакета на адаптере NetPulse, 1280–9000 байт. Слишком большой размер может вызывать потери, слишком малый повышает накладные расходы.",
    "Maximum IP packet size on the NetPulse adapter, 1280–9000 bytes. Oversized packets can cause loss; small packets increase overhead.",
  ],
  dns: [
    "Системный использует Windows. UDP не шифруется. DoH/DoT шифруют запросы до выбранного резолвера, но начальное разрешение его имени может использовать Windows.",
    "System DNS uses Windows. UDP is unencrypted. DoH/DoT encrypt requests to the resolver; initial resolver hostname lookup may use Windows.",
  ],
  dnsRoute: [
    "Отправляет запросы встроенного DNS через VPN или напрямую. Для системного DNS маршрут управляется Windows.",
    "Send built-in resolver requests through VPN or directly. Windows controls the system DNS route.",
  ],
  dnsServer: [
    "IP или домен DNS-резолвера. Для DoH/DoT сервер должен поддерживать соответствующий шифрованный протокол.",
    "DNS resolver IP or domain. DoH/DoT require compatible encrypted resolver support.",
  ],
  dnsPath: [
    "Путь HTTPS-запроса к DoH, обычно /dns-query. Используется только DNS over HTTPS.",
    "DoH HTTPS request path, usually /dns-query. Only used for DNS over HTTPS.",
  ],
  fake: [
    "В режиме TUN выдаёт DNS-адреса-заглушки и хранит связь с доменом в памяти. После переключения перезапустите соединения приложений из-за DNS-кеша.",
    "In TUN mode return mapped synthetic DNS addresses. Restart app connections after switching because of DNS caches.",
  ],
  fragment: [
    "Включает фрагментацию внешнего соединения средствами Xray. Может замедлить подключение или оказаться несовместимой с сервером. По умолчанию выключена.",
    "Enable Xray outer-connection fragmentation. Can slow connection setup or be incompatible. Off by default.",
  ],
  packets: [
    "tlshello разбивает TLS ClientHello. 1–3 применяется к первым TCP-записям. Это настройка Xray, а не перехват TLS приложений.",
    "tlshello fragments TLS ClientHello; 1–3 applies to initial TCP writes. An Xray transport setting, not app TLS interception.",
  ],
  length: [
    "Диапазон длины фрагмента в байтах. Xray выбирает размер в указанном диапазоне для фрагментов.",
    "Fragment length range in bytes, selected by Xray.",
  ],
  interval: [
    "Диапазон задержки между фрагментами, в миллисекундах. Увеличение задержки замедляет начало соединения.",
    "Delay range between fragments in milliseconds. Higher values slow connection setup.",
  ],
  split: [
    "Ограничение максимального разделения фрагмента в реализации Xray. Число или диапазон 1–65535.",
    "Xray maximum fragment split parameter. A number or range from 1 to 65535.",
  ],
  mux: [
    "Объединяет потоки VLESS/Trojan в мультиплексное соединение Xray. Не поддерживается с Vision, gRPC, XHTTP и Hysteria2. Сервер должен поддерживать выбранный режим.",
    "Multiplex VLESS/Trojan streams using Xray. Excludes Vision, gRPC, XHTTP and Hysteria2. Requires compatible server support.",
  ],
  tcpMux: [
    "Максимальная конкуренция потоков TCP, от -1 до 128. -1 отключает TCP Mux; 0 отдаёт выбор ядру.",
    "TCP stream concurrency from -1 to 128. -1 disables TCP Mux; 0 uses core defaults.",
  ],
  xudp: [
    "Конкуренция UDP-потоков XUDP, от -1 до 1024. Требует поддержки сервера.",
    "XUDP stream concurrency, -1 to 1024. Requires server support.",
  ],
  quic: [
    "Поведение UDP на порту 443 внутри Mux: reject отклоняет, allow разрешает, skip обходит мультиплексор.",
    "UDP 443 handling in Mux: reject, allow, or skip multiplexing.",
  ],
  processSearch: [
    "Ищет в списке запущенных процессов по имени EXE, полному пути и PID. Список читается при первом открытии; кнопка «Найти процессы» обновляет его. Для смены процесса остановите наблюдение.",
    "Search running processes by EXE name, full path or PID. The list loads on first opening; Find processes refreshes it. Stop observation to choose another process.",
  ],
  connectionSearch: [
    "Фильтрует уже собранные соединения по адресу, домену, приложению, PID, протоколу, состоянию и маршруту. Пробелы по краям игнорируются; несколько слов должны встретиться в одной строке. Новые соединения появляются после наблюдения или обновления.",
    "Filter collected connections by address, domain, app, PID, protocol, state or route. Surrounding whitespace is ignored; all search terms must match one row. Observe or refresh to collect connections.",
  ],
  process: [
    "Выберите запущенный процесс по PID. Все соединения ядра показывает только данные, захваченные VPN. Наблюдение не запускает процесс.",
    "Choose a running PID. All core connections shows VPN-captured data. Observation does not launch a process.",
  ],
  filter: [
    "Фильтрует отображаемые записи по имени приложения, адресу или названию. Не меняет маршрутизацию.",
    "Filter displayed records by app, address or name. Does not change routing.",
  ],
  autoRefresh: [
    "Автоматически обновляет Playground только во время наблюдения и пока вкладка открыта. По умолчанию выключено.",
    "Refresh Playground automatically only while observing and the tab is open. Off by default.",
  ],
  refreshRate: [
    "Пауза между завершением запроса и следующим обновлением: 1, 5 или 10 секунд. Параллельные запросы не запускаются.",
    "Delay after a request completes: 1, 5, or 10 seconds. Requests never overlap.",
  ],
  proxy_https_rtt: [
    "Реальный HTTPS GET через профиль VPN с проверкой TLS и авторизации. Расходует небольшой объём трафика.",
    "Real HTTPS GET through the VPN profile, validating TLS and authentication. Uses a small payload.",
  ],
  proxy_https_head_rtt: [
    "HTTPS HEAD через профиль: проверяет соединение без загрузки тела ответа. Некоторые серверы не поддерживают HEAD.",
    "HTTPS HEAD through the profile without a response body. Some endpoints do not support HEAD.",
  ],
  tcp_connect_rtt: [
    "Задержка установления TCP до сервера вне TUN NetPulse. Не проверяет авторизацию и UDP Hysteria2.",
    "TCP connection delay to the server outside NetPulse TUN. Does not validate authentication or Hysteria2 UDP.",
  ],
  icmp_rtt: [
    "ICMP Echo по текущим маршрутам Windows. Может блокироваться сервером и сам по себе не проверяет VPN.",
    "ICMP Echo using Windows routes. Servers may block it; it does not verify VPN.",
  ],
  pingMode: [
    "double создаёт два отдельных HTTPS-соединения. keepalive повторяет запрос по открытому соединению, если сервер разрешает.",
    "double uses separate HTTPS connections; keepalive reuses one when supported.",
  ],
  json: [
    "Редактируется JSON исходящего сервера, а не произвольный файл ядра. Перед сохранением проверяются схема и конфигурация. Секреты остаются в зашифрованном хранилище.",
    "Edit the server outbound JSON, not an arbitrary core file. Schema and runtime configuration are validated. Secrets remain encrypted in storage.",
  ],
  bypass: [
    "Домены и их поддомены направляются напрямую. Один домен в строке, без URL и пути. Применяется после сохранения.",
    "Route these domains and subdomains directly. One domain per line, without URL or path. Save to apply.",
  ],
  subInterval: [
    "Период обновления подписки в часах, от 1 до 168. Проверка выполняется, пока NetPulse запущен.",
    "Subscription refresh interval in hours, 1–168. Checked while NetPulse is running.",
  ],
  lock: [
    "После сохранения навсегда запрещает ручное редактирование JSON серверов этой подписки в клиенте. Старый бэкап не снимает запрет. Обновления провайдера продолжают работать.",
    "Permanently disable manual JSON editing for this subscription after saving. Older backups do not unlock it. Provider refreshes continue.",
  ],
  ruleEnabled: [
    "Включает это правило; выключенное правило хранится, но не применяется. Порядок правил имеет значение.",
    "Enable this rule. Disabled rules are retained but ignored. Rule order matters.",
  ],
  ruleKind: [
    "domain — домен, cidr — подсеть IP, geoip — страна адреса, geosite — набор доменов, process — путь EXE в TUN.",
    "domain: domain; cidr: IP subnet; geoip: address country; geosite: domain set; process: EXE path in TUN.",
  ],
  ruleValue: [
    "Значение зависит от типа: домен без схемы, IP/маска, код страны из двух букв, имя geosite или полный путь EXE.",
    "Use a domain without scheme, IP/prefix, two-letter country, geosite set name, or full EXE path.",
  ],
  ruleAction: [
    "VPN направляет совпавший трафик в туннель; напрямую обходит VPN; блокировать отклоняет соединение.",
    "VPN tunnels matching traffic; direct bypasses VPN; block rejects the connection.",
  ],
  source: [
    "Ограничивает список выбранной подпиской или источником. Сами серверы и подключение не изменяются.",
    "Filter the list by subscription or source. Does not modify servers or connections.",
  ],
  group: [
    "Название локальной группы приложений для повторного выбора нескольких EXE.",
    "Local app group name for selecting several EXE paths together.",
  ],
  method: [
    "TCP и ICMP проверяют доступность сервера, HTTPS GET/HEAD — реальный маршрут через VPN.",
    "TCP/ICMP check server reachability; HTTPS GET/HEAD test the VPN route.",
  ],
  view: [
    "Меняет только отображение: список или плитки. Настройка сохраняется локально.",
    "Change list or tile presentation only. Saved locally.",
  ],
  day: [
    "Дата локального каталога журналов. Старые записи доступны по выбранной дате.",
    "Date of the local log directory. Select a date to read older records.",
  ],
  key: [
    "Вставьте ключ подключения или конфигурацию поддерживаемого формата. Пароли не отправляются сторонним сервисам распознавания.",
    "Paste a supported connection key or configuration. Passwords are not sent to external decoding services.",
  ],
  sort: [
    "Сортирует видимые серверы по названию или последнему результату задержки; не переключает активный сервер.",
    "Sort visible servers by name or latest latency result. Does not switch the active server.",
  ],
} as const;
export type HelpTopic = keyof typeof help;
export function helpText(topic: HelpTopic, language: string) {
  return help[topic][language === "ru" ? 0 : 1];
}
