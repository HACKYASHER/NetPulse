import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: win
    objectName: "netpulseWindow"
    width: 1220; height: 840; minimumWidth: 1000; minimumHeight: 700
    visible: !startHidden || smokeMode
    title: "NetPulse 1.0.0 beta"
    property var modelData: backend.data
    property var prefs: modelData.preferences || ({})
    property var snapshot: modelData.snapshot || ({state: "disconnected"})
    property var profiles: modelData.profiles || []
    property var subscriptions: modelData.subscriptions || []
    property bool light: prefs.theme === "light" || (prefs.theme === "system" && Application.styleHints.colorScheme === Qt.ColorScheme.Light)
    readonly property color bgColor: light ? "#f4f5fa" : "#10121b"
    readonly property color panelColor: light ? "#ffffff" : "#191c28"
    readonly property color sidebarColor: light ? "#eceef6" : "#141620"
    readonly property color borderColor: light ? "#dfe2ed" : "#2a2e3e"
    readonly property color textColor: light ? "#222536" : "#edf0fa"
    readonly property color mutedColor: light ? "#747b92" : "#9098af"
    readonly property color accentColor: "#9a8aff"
    property int page: 0
    property var titles: [tr("Обзор", "Overview"), tr("Серверы", "Servers"), tr("Подписки", "Subscriptions"), tr("Маршрутизация", "Routing"), tr("Сеть", "Network"), tr("Диагностика", "Diagnostics"), "Playground", tr("Активность", "Activity"), tr("Настройки", "Settings"), tr("Ядра", "Engines")]
    property var captions: [tr("Ваше подключение под контролем", "Your connection, under control"), tr("Профили и доступные подключения", "Profiles and available connections"), tr("Источники серверов и обновления", "Server sources and updates"), tr("Выберите, куда направлять трафик", "Choose where your traffic goes"), tr("Подключение, DNS и транспорт", "Connection, DNS and transport"), tr("Измерения на реальных соединениях", "Measurements over real connections"), tr("Соединения выбранного процесса", "Connections for a selected process"), tr("События приложения и сетевых компонентов", "Application and network component events"), tr("Внешний вид, запуск и данные", "Appearance, startup and data"), tr("Комплектные и доступные сетевые ядра", "Bundled and available network engines")]
    property var callbacks: ({})
    property var preferenceQueue: ({})
    property bool savingPreferences: false
    property bool waitingPreferencesRefresh: false
    property string notice: ""
    property string diagnosticId: ""
    property var job: null
    property string profileSearch: ""
    property string appSearch: ""
    property var applications: []
    property var processes: []
    property var analysis: null
    property var coreReleases: []
    property string publicIpText: "—"
    property int selectedProcess: -1
    property bool observing: false
    property int observationSeconds: 0
    property int smokePages: 0
    property bool smokeFinished: false
    property var pages: [overviewPage, serversPage, subscriptionsPage, routingPage, networkPage, diagnosticsPage, playgroundPage, activityPage, settingsPage, enginesPage]
    color: bgColor
    palette.window: bgColor; palette.windowText: textColor; palette.text: textColor
    palette.base: light ? "#f6f7fb" : "#111520"
    palette.button: light ? "#f0f1f8" : "#222635"
    palette.buttonText: textColor; palette.mid: borderColor
    palette.highlight: "#8775ff"; palette.highlightedText: "white"
    palette.placeholderText: mutedColor

    function tr(ru, en) { return prefs.language === "en" ? en : ru }
    function busy(method) { return backend.pending.indexOf(method) >= 0 }
    function clone(value) { return JSON.parse(JSON.stringify(value)) }
    function call(method, params, done) {
        var id = backend.request(method, params || {})
        if (done && id) { var c = Object.assign({}, callbacks); c[id] = done; callbacks = c }
        return id
    }
    function save(patch) {
        preferenceQueue = Object.assign({}, preferenceQueue, patch)
        flushPreferences()
    }
    function flushPreferences() {
        if (savingPreferences || Object.keys(preferenceQueue).length === 0) return
        var patch = preferenceQueue; preferenceQueue = ({}); savingPreferences = true
        if (!call("save_preferences", {preferences: Object.assign(clone(prefs), patch)}, function() { toast(tr("Настройки сохранены", "Settings saved")) })) savingPreferences = false
    }
    function toast(text) { notice = text; toastTimer.restart() }
    function bytes(value) {
        if (value === null || value === undefined) return "—"
        var units = ["B", "KB", "MB", "GB", "TB"], i = 0
        while (value >= 1024 && i < 4) { value /= 1024; i++ }
        return value.toFixed(i ? 1 : 0) + " " + units[i]
    }
    function stateLabel() {
        var map = {disconnected: tr("Не подключено", "Disconnected"), connected: tr("Подключено", "Connected"), connecting: tr("Подключение…", "Connecting…"), validating: tr("Проверка…", "Validating…"), switching: tr("Смена сервера…", "Switching…"), disconnecting: tr("Отключение…", "Disconnecting…"), error: tr("Ошибка подключения", "Connection error")}
        return map[snapshot.state] || snapshot.state
    }
    function selected() {
        for (var i = 0; i < profiles.length; i++) if (profiles[i].id === prefs.selected_profile) return profiles[i]
        return profiles.length ? profiles[0] : null
    }
    function connectSelected() {
        if (snapshot.engine_pid) { call("disconnect"); return }
        var p = selected()
        if (!p) { importDialog.open(); return }
        call("connect", {id: p.id})
    }
    function beginJob(method, params) { call(method, params, function(id) { diagnosticId = id; job = null; jobTimer.start() }) }
    function editJson(title, content, saveMethod, params) {
        jsonDialog.title = title; jsonEditor.text = content; jsonDialog.saveMethod = saveMethod; jsonDialog.parameters = params || {}; jsonDialog.open()
    }
    function selectPage(index) {
        page = index
        if (index !== 6) observing = false
        if (index === 3) call("read_application_catalog", {}, function(items) { applications = items || [] })
        if (index === 6) call("playground_processes", {}, function(items) { processes = items || [] })
        if (index === 7) backend.refresh()
    }
    function observe() {
        var p = selectedProcess >= 0 ? processes[selectedProcess] : null
        call("connection_analysis", {pid: p ? p.pid : null, path: p ? p.path : ""}, function(result) { analysis = result })
    }
    onClosing: function(close) { close.accepted = false; if (prefs.close_to_tray && !smokeMode) hide(); else backend.quit() }
    onVisibilityChanged: if (visibility === Window.Minimized && prefs.minimize_to_tray && !smokeMode) hide()
    Connections {
        target: backend
        function onCompleted(id, method, result, error) {
            var done = callbacks[id]
            if (done) { var c = Object.assign({}, callbacks); delete c[id]; callbacks = c; if (!error.code) done(result) }
            if (method === "save_preferences") {
                if (error.code) { savingPreferences = false; preferenceQueue = ({}); backend.refresh() }
                else waitingPreferencesRefresh = true
            }
            if (method === "bootstrap" && waitingPreferencesRefresh) {
                waitingPreferencesRefresh = false; savingPreferences = false; flushPreferences()
            }
            if (method === "bootstrap" && result && !error.code && smokeMode && result.engine_ready && !smokeTimer.running && !smokeFinished) smokeTimer.start()
            if (!error.code && (method === "import_text" || method === "import_file" || method === "import_clipboard" || method === "update_subscription")) {
                if (result) toast(tr("Импортировано профилей: ", "Profiles imported: ") + result.imported)
            }
        }
    }
    Timer { id: toastTimer; interval: 4500; onTriggered: notice = "" }
    Timer {
        id: jobTimer; interval: 1000; repeat: true
        onTriggered: {
            if (!diagnosticId || busy("diagnostic_status")) return
            call("diagnostic_status", {id: diagnosticId}, function(result) {
                job = result
                if (!result.running) { jobTimer.stop(); backend.refresh() }
            })
        }
    }
    Timer { interval: 1000; running: observing; repeat: true; onTriggered: { observationSeconds++; if (observationSeconds >= 5 && !busy("connection_analysis")) { observationSeconds = 0; observe() } } }
    Timer { id: smokeTimer; interval: 500; repeat: true; onTriggered: { if (pageLoader.status !== Loader.Ready) return; if (page < 9) { page++; smokePages++ } else { stop(); page = 0; smokePages = 10; smokeFinished = true } } }

    RowLayout {
        anchors.fill: parent; spacing: 0
        Rectangle {
            Layout.preferredWidth: 220; Layout.fillHeight: true; color: sidebarColor
            Rectangle { width: 1; height: parent.height; anchors.right: parent.right; color: borderColor }
            ColumnLayout {
                anchors.fill: parent; anchors.margins: 18; spacing: 5
                RowLayout {
                    Layout.topMargin: 10; Layout.bottomMargin: 32; spacing: 12
                    Image { source: "../resources/pulse.svg"; Layout.preferredWidth: 38; Layout.preferredHeight: 38 }
                    Column { spacing: 1; Label { textFormat: Text.PlainText; text: "NetPulse"; color: textColor; font.pixelSize: 21; font.weight: Font.DemiBold } Label { textFormat: Text.PlainText; text: "NETWORK CLIENT"; color: mutedColor; font.pixelSize: 9; font.letterSpacing: 1.6 } }
                }
                Repeater {
                    model: titles
                    delegate: ItemDelegate {
                        required property int index
                        required property string modelData
                        Layout.fillWidth: true; implicitHeight: 45
                        contentItem: RowLayout { spacing: 13
                            Label { textFormat: Text.PlainText; text: ["◉", "⇄", "↻", "⑂", "⌘", "⌁", "◎", "≡", "⚙", "◇"][index]; color: page === index ? accentColor : mutedColor; font.pixelSize: 20; Layout.preferredWidth: 22; horizontalAlignment: Text.AlignHCenter }
                            Label { textFormat: Text.PlainText; text: modelData; color: page === index ? textColor : mutedColor; font.pixelSize: 13; font.weight: page === index ? Font.DemiBold : Font.Normal; Layout.fillWidth: true }
                        }
                        background: Rectangle { color: page === index ? (light ? "#e0dcfb" : "#2a2544") : hovered ? (light ? "#e5e8f1" : "#1c2030") : "transparent"; radius: 10 }
                        onClicked: selectPage(index)
                    }
                }
                Item { Layout.fillHeight: true }
                Card {
                    Layout.fillWidth: true; implicitHeight: 64
                    RowLayout { anchors.fill: parent; anchors.margins: 12; spacing: 9
                        Rectangle { width: 7; height: 7; radius: 4; color: modelData.engine_ready ? "#6bd9b0" : "#e0b86d" }
                        Column { spacing: 3; Label { textFormat: Text.PlainText; text: modelData.engine_ready ? tr("Ядра готовы", "Engines ready") : modelData.runtime_preparing ? tr("Подготовка ядер…", "Preparing engines…") : tr("Проверьте ядра", "Check engines"); font.pixelSize: 11; color: textColor } Label { textFormat: Text.PlainText; text: "NetPulse 1.0.0 beta"; font.pixelSize: 10; color: mutedColor } }
                    }
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true; Layout.fillHeight: true; spacing: 0
            RowLayout {
                Layout.fillWidth: true; Layout.margins: 30; Layout.bottomMargin: 24
                ColumnLayout { spacing: 6; Layout.fillWidth: true
                    Label { textFormat: Text.PlainText; text: titles[page]; font.pixelSize: 27; font.weight: Font.DemiBold; color: textColor }
                    Label { textFormat: Text.PlainText; text: captions[page]; font.pixelSize: 12; color: mutedColor }
                }
                Rectangle { implicitWidth: stateText.implicitWidth + 30; height: 32; radius: 16; color: snapshot.state === "connected" ? (light ? "#def4eb" : "#17392f") : panelColor; border.color: borderColor
                    Label { textFormat: Text.PlainText; id: stateText; anchors.centerIn: parent; text: stateLabel(); color: snapshot.state === "connected" ? "#64d5a8" : mutedColor; font.pixelSize: 11 }
                }
            }
            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: borderColor }
            Rectangle {
                visible: backend.errorMessage.length > 0
                Layout.fillWidth: true; Layout.leftMargin: 30; Layout.rightMargin: 30; Layout.topMargin: 18
                implicitHeight: errorRow.implicitHeight + 24; radius: 10; color: light ? "#ffe6e9" : "#39232d"
                RowLayout { id: errorRow; anchors.fill: parent; anchors.margins: 12
                    Label { textFormat: Text.PlainText; text: backend.errorMessage; wrapMode: Text.WordWrap; Layout.fillWidth: true; color: light ? "#9c3646" : "#ffb0bd"; font.pixelSize: 12 }
                    ToolButton { text: "×"; onClicked: backend.clearError() }
                }
            }
            ScrollView {
                id: bodyScroll
                Layout.fillWidth: true; Layout.fillHeight: true; Layout.margins: 30; Layout.topMargin: 24
                contentWidth: availableWidth; clip: true
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                Loader { id: pageLoader; width: bodyScroll.availableWidth; sourceComponent: pages[page] }
            }
        }
    }
    Rectangle {
        visible: notice.length > 0; anchors.horizontalCenter: parent.horizontalCenter; anchors.bottom: parent.bottom; anchors.bottomMargin: 24
        width: Math.min(noticeText.implicitWidth + 44, parent.width - 100); height: 48; radius: 12; color: light ? "#302c48" : "#e6e1ff"; z: 5
        Label { textFormat: Text.PlainText; id: noticeText; anchors.centerIn: parent; text: notice; color: light ? "#ffffff" : "#28203f"; font.pixelSize: 13 }
    }

    Component {
        id: overviewPage
        ColumnLayout {
            spacing: 20
            Card {
                Layout.fillWidth: true; implicitHeight: 245
                gradient: Gradient { GradientStop { position: 0; color: light ? "#e8e4ff" : "#29243f" } GradientStop { position: 1; color: panelColor } }
                RowLayout {
                    anchors.fill: parent; anchors.margins: 30; spacing: 35
                    ColumnLayout { Layout.fillWidth: true; spacing: 12
                        Label { textFormat: Text.PlainText; text: snapshot.engine_pid ? tr("Соединение установлено", "Connection established") : tr("Готовы к подключению", "Ready to connect"); color: textColor; font.pixelSize: 23; font.weight: Font.DemiBold }
                        Label { textFormat: Text.PlainText; text: selected() ? selected().name : tr("Добавьте сервер или подписку, чтобы начать", "Add a server or subscription to get started"); color: mutedColor; Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 14 }
                        RowLayout {
                            Label { textFormat: Text.PlainText; text: "● " + (snapshot.tun_active ? "TUN" : "System Proxy"); color: accentColor; font.pixelSize: 12 }
                            Label { textFormat: Text.PlainText; text: "  ·  " + (prefs.transport_engine || "auto"); color: mutedColor; font.pixelSize: 12 }
                        }
                        Item { implicitHeight: 6 }
                        RowLayout {
                            ActionButton { text: tr("Выбрать сервер", "Choose server"); onClicked: selectPage(1) }
                            ActionButton { text: tr("Добавить", "Add"); onClicked: importDialog.open() }
                        }
                    }
                    Rectangle {
                        Layout.preferredWidth: 142; Layout.preferredHeight: 142; radius: 71
                        color: snapshot.engine_pid ? "#193d36" : light ? "#d6cffb" : "#393052"
                        border.color: snapshot.engine_pid ? "#58d1a5" : "#7463b9"; border.width: 1
                        Rectangle { anchors.fill: parent; anchors.margins: 12; radius: width / 2; color: snapshot.engine_pid ? "#28584c" : "#8775ff"
                            Label { textFormat: Text.PlainText; anchors.centerIn: parent; text: "⏻"; font.pixelSize: 49; color: "white" }
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; enabled: backend.ready && !busy("connect") && !busy("disconnect"); onClicked: connectSelected() }
                            BusyIndicator { anchors.centerIn: parent; running: busy("connect") || busy("disconnect"); visible: running }
                        }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true; spacing: 14
                Metric { Layout.fillWidth: true; label: tr("ЗАГРУЗКА", "DOWNLOAD"); value: bytes(snapshot.metrics ? snapshot.metrics.download_bps : null) + "/s"; hint: tr("Текущая скорость ядра", "Current engine throughput") }
                Metric { Layout.fillWidth: true; label: tr("ОТДАЧА", "UPLOAD"); value: bytes(snapshot.metrics ? snapshot.metrics.upload_bps : null) + "/s"; hint: tr("Текущая скорость ядра", "Current engine throughput") }
                Metric { Layout.fillWidth: true; label: tr("ЗАДЕРЖКА", "LATENCY"); value: snapshot.probe_ms !== null && snapshot.probe_ms !== undefined ? snapshot.probe_ms + " ms" : "—"; hint: tr("HTTPS-проверка соединения", "Connection HTTPS probe") }
            }
            Card {
                Layout.fillWidth: true; implicitHeight: quickSettings.implicitHeight + 40
                ColumnLayout { id: quickSettings; anchors.fill: parent; anchors.margins: 20; spacing: 18
                    Label { textFormat: Text.PlainText; text: tr("Режим подключения", "Connection mode"); color: textColor; font.pixelSize: 15; font.weight: Font.DemiBold }
                    SettingRow { Layout.fillWidth: true; title: "System Proxy"; description: tr("Для приложений, использующих системный прокси Windows", "For applications that use the Windows system proxy"); checked: !!prefs.system_proxy; enabled: !busy("quick_network"); onToggled: function(value) { call("quick_network", {system_proxy: value}) } }
                    SettingRow { Layout.fillWidth: true; title: "TUN"; description: tr("Захват трафика через виртуальный адаптер; требуется служба", "Traffic capture through a virtual adapter; requires the service"); checked: !!prefs.tun; enabled: !busy("quick_network"); onToggled: function(value) { call("quick_network", {tun: value}) } }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Серверов: ", "Servers: ") + profiles.length + tr("    Подписок: ", "    Subscriptions: ") + subscriptions.length; color: mutedColor; font.pixelSize: 12; Layout.fillWidth: true }
                ActionButton { text: tr("Проверить IP", "Check IP"); enabled: !busy("public_ips"); onClicked: call("public_ips", {}, function(value) { publicIpText = JSON.stringify(value); editJson(tr("Публичные адреса", "Public addresses"), JSON.stringify(value, null, 2), "", {}) }) }
            }
        }
    }

    Component {
        id: serversPage
        ColumnLayout {
            spacing: 16
            RowLayout { Layout.fillWidth: true
                Input { Layout.fillWidth: true; placeholderText: tr("Найти по имени, адресу или протоколу…", "Search name, address or protocol…"); text: profileSearch; onTextChanged: profileSearch = text }
                ActionButton { text: tr("Проверить", "Test"); enabled: profiles.length > 0 && !jobTimer.running; onClicked: beginJob("start_latency", {ids: profiles.map(function(p) { return p.id }), method: prefs.ping_method || "tcp_connect_rtt"}) }
                ActionButton { text: "+ " + tr("Добавить", "Add"); primary: true; onClicked: importDialog.open() }
            }
            Label { textFormat: Text.PlainText; text: tr("Профили: ", "Profiles: ") + profiles.length; color: mutedColor; font.pixelSize: 12 }
            Card { visible: profiles.length === 0; Layout.fillWidth: true; implicitHeight: 210
                Column { anchors.centerIn: parent; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Здесь появятся ваши серверы", "Your servers will appear here"); color: textColor; font.pixelSize: 18; anchors.horizontalCenter: parent.horizontalCenter }
                    Label { textFormat: Text.PlainText; text: tr("Импортируйте ключ, файл, QR-код или подписку", "Import a link, file, QR code or subscription"); color: mutedColor; font.pixelSize: 12; anchors.horizontalCenter: parent.horizontalCenter }
                    ActionButton { text: tr("Импортировать", "Import"); primary: true; anchors.horizontalCenter: parent.horizontalCenter; onClicked: importDialog.open() }
                }
            }
            Repeater {
                model: profiles.filter(function(p) { return (p.name + " " + p.host + " " + p.protocol).toLowerCase().indexOf(profileSearch.toLowerCase()) >= 0 })
                delegate: Card {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: 100
                    border.color: prefs.selected_profile === modelData.id ? "#8775ff" : borderColor
                    RowLayout { anchors.fill: parent; anchors.margins: 16; spacing: 14
                        Rectangle { width: 44; height: 44; radius: 13; color: light ? "#eae5ff" : "#302943"; Label { textFormat: Text.PlainText; anchors.centerIn: parent; text: "⇄"; color: accentColor; font.pixelSize: 22 } }
                        ColumnLayout { Layout.fillWidth: true; spacing: 5
                            Label { textFormat: Text.PlainText; text: modelData.name; color: textColor; Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 14; font.weight: Font.DemiBold }
                            Label { textFormat: Text.PlainText; text: modelData.protocol.toUpperCase() + " · " + modelData.host + ":" + modelData.port; color: mutedColor; Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 11 }
                            Label { textFormat: Text.PlainText; text: modelData.latency_ms !== null ? modelData.latency_ms + " ms · " + (modelData.latency_method || "") : tr("Не проверен", "Not tested"); color: modelData.latency_ms !== null ? "#65cfa5" : mutedColor; font.pixelSize: 11 }
                        }
                        ActionButton { text: prefs.selected_profile === modelData.id ? tr("Выбран", "Selected") : tr("Выбрать", "Select"); onClicked: call("select_profile", {id: modelData.id}) }
                        ActionButton { text: "{ }"; ToolTip.text: tr("Редактировать профиль JSON", "Edit profile JSON"); onClicked: call("profile_json", {id: modelData.id}, function(json) { editJson(modelData.name, json, "save_profile_json", {id: modelData.id}) }) }
                        ActionButton { text: "×"; danger: true; ToolTip.text: tr("Удалить профиль", "Delete profile"); onClicked: backend.confirmDelete("delete_profile", {id: modelData.id}, tr("Удалить профиль «", "Delete profile “") + modelData.name + "»?") }
                    }
                }
            }
        }
    }

    Component {
        id: subscriptionsPage
        ColumnLayout {
            spacing: 16
            Card { Layout.fillWidth: true; implicitHeight: subForm.implicitHeight + 40
                ColumnLayout { id: subForm; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("Добавить подписку", "Add subscription"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    Input { id: subName; Layout.fillWidth: true; placeholderText: tr("Название", "Name") }
                    Input { id: subUrl; Layout.fillWidth: true; placeholderText: "https://…"; echoMode: TextInput.PasswordEchoOnEdit }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Ссылки хранятся в зашифрованном виде", "Subscription URLs are stored encrypted"); color: mutedColor; font.pixelSize: 11; Layout.fillWidth: true }
                        ActionButton { text: tr("Добавить", "Add"); primary: true; enabled: subUrl.text.length > 0 && !busy("add_subscription"); onClicked: call("add_subscription", {name: subName.text || tr("Подписка", "Subscription"), url: subUrl.text, protected: true}, function() { subUrl.clear(); subName.clear() }) }
                    }
                }
            }
            Repeater { model: subscriptions
                delegate: Card {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: subRow.implicitHeight + 36
                    ColumnLayout { id: subRow; anchors.fill: parent; anchors.margins: 18; spacing: 12
                        RowLayout { Layout.fillWidth: true
                            ColumnLayout { Layout.fillWidth: true; spacing: 5
                                Label { textFormat: Text.PlainText; text: modelData.name; color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                                Label { textFormat: Text.PlainText; text: tr("Серверов: ", "Servers: ") + modelData.count + tr(" · Обновление: каждые ", " · Refresh every ") + modelData.interval_hours + tr(" ч", " h"); color: mutedColor; font.pixelSize: 12 }
                            }
                            ActionButton { text: tr("Обновить", "Refresh"); enabled: !busy("update_subscription"); onClicked: call("update_subscription", {id: modelData.id}) }
                            ActionButton { text: "…"; onClicked: { subEdit.sub = modelData; subEditName.text = modelData.name; subEditHours.value = modelData.interval_hours; subEditLock.checked = modelData.config_locked; subEdit.open() } }
                            ActionButton { text: "×"; danger: true; onClicked: backend.confirmDelete("delete_subscription", {id: modelData.id}, tr("Удалить подписку и её серверы?", "Delete the subscription and its servers?")) }
                        }
                        Label { textFormat: Text.PlainText; visible: !!modelData.usage; text: modelData.usage ? tr("Использовано: ", "Used: ") + bytes((modelData.usage.download || 0) + (modelData.usage.upload || 0)) + " / " + bytes(modelData.usage.total) : ""; color: mutedColor; font.pixelSize: 12 }
                    }
                }
            }
        }
    }

    Component {
        id: routingPage
        ColumnLayout {
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: routeForm.implicitHeight + 40
                ColumnLayout { id: routeForm; anchors.fill: parent; anchors.margins: 20; spacing: 16
                    Label { textFormat: Text.PlainText; text: tr("Режим маршрутизации", "Routing mode"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    ComboBox { id: routeMode; Layout.fillWidth: true; model: [tr("Весь трафик через VPN", "All traffic through VPN"), tr("Только выбранные приложения", "Selected applications only"), tr("Всё, кроме выбранных приложений", "All except selected applications")]; currentIndex: Math.max(0, ["global", "whitelist", "blacklist"].indexOf(prefs.route_mode)); onActivated: call("quick_network", {route_mode: ["global", "whitelist", "blacklist"][currentIndex]}) }
                    Label { textFormat: Text.PlainText; text: tr("Правила приложений применяются в TUN. При System Proxy маршрут также зависит от настроек самого приложения.", "Application rules apply in TUN. With System Proxy, the route also depends on each application's settings."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    RowLayout { Layout.fillWidth: true
                        ActionButton { text: tr("Импорт правил", "Import rules"); onClicked: call("import_routing", {}, function(value) { if (value) { delete value.version; save(value) } }) }
                        ActionButton { text: tr("Экспорт правил", "Export rules"); onClicked: call("export_routing", {routing: {version: 1, route_mode: prefs.route_mode, applications: prefs.applications, application_groups: prefs.application_groups, bypass_domains: prefs.bypass_domains, routing_rules: prefs.routing_rules, sniff_excluded_apps: prefs.sniff_excluded_apps}}) }
                        ActionButton { text: tr("Все параметры JSON", "All settings JSON"); onClicked: editJson(tr("Параметры NetPulse", "NetPulse preferences"), JSON.stringify(prefs, null, 2), "save_preferences") }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Приложения", "Applications"); font.pixelSize: 17; font.weight: Font.DemiBold; color: textColor; Layout.fillWidth: true }
                ActionButton { text: tr("Сканировать", "Scan"); enabled: !busy("scan_applications"); onClicked: call("scan_applications", {}, function(items) { applications = items; call("save_application_catalog", {applications: items}) }) }
                ActionButton { text: "+ EXE"; onClicked: call("choose_exe", {}, function(item) { if (item) { applications = applications.concat([item]); call("save_application_catalog", {applications: applications}) } }) }
            }
            Input { Layout.fillWidth: true; placeholderText: tr("Поиск приложения…", "Search applications…"); text: appSearch; onTextChanged: appSearch = text }
            Label { textFormat: Text.PlainText; text: tr("Выбрано: ", "Selected: ") + (prefs.applications || []).length; color: mutedColor; font.pixelSize: 12 }
            Repeater { model: applications.filter(function(a) { return (a.name + a.path).toLowerCase().indexOf(appSearch.toLowerCase()) >= 0 }).slice(0, 150)
                delegate: Card {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: 72
                    RowLayout { anchors.fill: parent; anchors.margins: 14
                        CheckBox { checked: (prefs.applications || []).indexOf(modelData.path) >= 0; enabled: !busy("save_preferences"); onToggled: { var paths = (prefs.applications || []).filter(function(p) { return p !== modelData.path }); if (checked) paths.push(modelData.path); save({applications: paths}) } }
                        ColumnLayout { Layout.fillWidth: true; spacing: 4
                            Label { textFormat: Text.PlainText; text: modelData.name; color: textColor; font.pixelSize: 13 }
                            Label { textFormat: Text.PlainText; text: modelData.path; color: mutedColor; font.pixelSize: 10; Layout.fillWidth: true; elide: Text.ElideMiddle }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: networkPage
        ColumnLayout {
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: networkForm.implicitHeight + 40
                ColumnLayout { id: networkForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Параметры подключения", "Connection settings"); font.pixelSize: 16; font.weight: Font.DemiBold; color: textColor }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Транспортное ядро", "Transport engine"); Layout.fillWidth: true; color: mutedColor }
                        ComboBox { model: ["auto", "sing-box", "xray"]; currentIndex: Math.max(0, model.indexOf(prefs.transport_engine || "auto")); onActivated: call("quick_network", {transport_engine: currentText}) }
                    }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Локальный порт прокси", "Local proxy port"); color: mutedColor; Layout.fillWidth: true }
                        SpinBox { id: proxyPort; from: 1024; to: 65535; value: prefs.proxy_port || 2080; editable: true }
                    }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: "MTU"; color: mutedColor; Layout.fillWidth: true }
                        SpinBox { id: mtu; from: 1280; to: 9000; value: prefs.mtu || 1400; editable: true }
                    }
                    SettingRow { Layout.fillWidth: true; title: "Fake IP"; description: tr("Применяется только с TUN; после изменения переоткройте соединения приложений", "Applies with TUN only; reopen application connections after changing it"); checked: !!prefs.fake_ip; onToggled: function(value) { save({fake_ip: value}) } }
                    Input { id: probeUrl; Layout.fillWidth: true; text: prefs.probe_url || ""; placeholderText: "HTTPS probe URL" }
                    ActionButton { text: tr("Сохранить подключение", "Save connection"); primary: true; enabled: !busy("save_preferences"); onClicked: save({proxy_port: proxyPort.value, mtu: mtu.value, probe_url: probeUrl.text}) }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: dnsForm.implicitHeight + 40
                ColumnLayout { id: dnsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: "DNS"; font.pixelSize: 16; font.weight: Font.DemiBold; color: textColor }
                    ComboBox { id: dnsMode; Layout.fillWidth: true; model: ["system", "udp", "tls", "https"]; currentIndex: Math.max(0, model.indexOf(prefs.dns ? prefs.dns.mode : "system")) }
                    Input { id: dnsServer; Layout.fillWidth: true; visible: dnsMode.currentText !== "system"; text: prefs.dns && prefs.dns.server ? prefs.dns.server : ""; placeholderText: "DNS server" }
                    Input { id: dnsPath; Layout.fillWidth: true; visible: dnsMode.currentText === "https"; text: prefs.dns && prefs.dns.path ? prefs.dns.path : "/dns-query"; placeholderText: "/dns-query" }
                    ComboBox { id: dnsRoute; Layout.fillWidth: true; model: [tr("DNS напрямую", "DNS over direct route"), tr("DNS через VPN", "DNS over VPN")]; currentIndex: prefs.dns_route === "vpn" ? 1 : 0 }
                    ActionButton { text: tr("Сохранить DNS", "Save DNS"); primary: true; enabled: !busy("save_preferences"); onClicked: { var d = {mode: dnsMode.currentText}; if (d.mode !== "system") d.server = dnsServer.text; if (d.mode === "https") d.path = dnsPath.text; save({dns: d, dns_route: dnsRoute.currentIndex ? "vpn" : "direct"}) } }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: serviceForm.implicitHeight + 40
                ColumnLayout { id: serviceForm; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("Служба TUN", "TUN service"); font.pixelSize: 16; font.weight: Font.DemiBold; color: textColor }
                    Label { textFormat: Text.PlainText; text: modelData.service ? (modelData.service.installed ? tr("Установлена", "Installed") : tr("Не установлена", "Not installed")) + (modelData.service.running ? tr(" · Работает", " · Running") : "") : "—"; color: mutedColor }
                    Label { textFormat: Text.PlainText; text: tr("Установка и удаление службы вызывают стандартное окно UAC Windows.", "Installing or removing the service opens the standard Windows UAC dialog."); color: mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        ActionButton { text: tr("Установить / восстановить", "Install / repair"); enabled: !busy("manage_service"); onClicked: call("manage_service", {install: true, upgrade: !!(modelData.service && modelData.service.installed)}) }
                        ActionButton { text: tr("Удалить службу", "Remove service"); enabled: !busy("manage_service") && !!(modelData.service && modelData.service.installed); onClicked: call("manage_service", {install: false}) }
                    }
                    ActionButton { text: tr("TLS, фрагментация, multiplex и расширенные правила…", "TLS, fragmentation, multiplex and advanced rules…"); onClicked: editJson(tr("Расширенные параметры", "Advanced preferences"), JSON.stringify(prefs, null, 2), "save_preferences") }
                }
            }
        }
    }

    Component {
        id: diagnosticsPage
        ColumnLayout {
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: latencyForm.implicitHeight + 40
                ColumnLayout { id: latencyForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Проверка серверов", "Server latency"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    ComboBox { id: latencyMethod; Layout.fillWidth: true; model: ["tcp_connect_rtt", "proxy_https_rtt", "proxy_https_head_rtt", "icmp_rtt"]; currentIndex: Math.max(0, model.indexOf(prefs.ping_method)) }
                    RowLayout {
                        ActionButton { text: tr("Проверить все", "Test all"); primary: true; enabled: profiles.length > 0 && !jobTimer.running; onClicked: beginJob("start_latency", {ids: profiles.map(function(p) { return p.id }), method: latencyMethod.currentText}) }
                        ActionButton { text: tr("Лучший сервер", "Best server"); enabled: profiles.length > 0 && !jobTimer.running; onClicked: beginJob("start_best_server", {samples: prefs.selection_samples || 5}) }
                    }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: speedForm.implicitHeight + 40
                ColumnLayout { id: speedForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: "Speed Test"; color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    Label { textFormat: Text.PlainText; text: tr("Укажите HTTPS-адреса для измерения. Тест расходует трафик; POST выполняется только при включённой отправке.", "Provide HTTPS measurement endpoints. The test uses traffic; POST is sent only when upload is enabled."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    Input { id: downloadUrl; Layout.fillWidth: true; placeholderText: tr("HTTPS URL загрузки", "Download HTTPS URL") }
                    Input { id: uploadUrl; Layout.fillWidth: true; placeholderText: tr("HTTPS URL отправки (необязательно)", "Upload HTTPS URL (optional)") }
                    RowLayout { Layout.fillWidth: true
                        ComboBox { id: speedMode; model: ["direct", "vpn", "compare"]; Layout.fillWidth: true }
                        SpinBox { id: speedBudget; from: 1; to: 100; value: 10; editable: true }
                        Label { textFormat: Text.PlainText; text: "MiB"; color: mutedColor }
                        CheckBox { id: uploadAllowed; text: "POST" }
                    }
                    ActionButton { text: tr("Начать измерение", "Start test"); primary: true; enabled: downloadUrl.text.length > 0 && !jobTimer.running; onClicked: beginJob("start_speed", {request: {download_url: downloadUrl.text, upload_url: uploadUrl.text || null, upload_endpoint_confirmed: uploadAllowed.checked, mode: speedMode.currentText, duration_seconds: 10, max_payload_mib: speedBudget.value}}) }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: mtuForm.implicitHeight + 40
                ColumnLayout { id: mtuForm; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("Анализ MTU", "MTU analysis"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    RowLayout { Layout.fillWidth: true
                        Input { id: mtuTarget; Layout.fillWidth: true; placeholderText: tr("IPv4-адрес узла", "IPv4 target address") }
                        ActionButton { text: tr("Проверить", "Analyze"); enabled: mtuTarget.text.length > 0 && !jobTimer.running; onClicked: beginJob("start_mtu", {target: mtuTarget.text}) }
                    }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: jobResult.implicitHeight + 40
                ColumnLayout { id: jobResult; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Результаты измерений", "Measurement results"); color: textColor; Layout.fillWidth: true; font.pixelSize: 16; font.weight: Font.DemiBold }
                        ActionButton { visible: jobTimer.running; text: tr("Отменить", "Cancel"); onClicked: call("cancel_diagnostic", {id: diagnosticId}) }
                        ActionButton { text: tr("История", "History"); onClicked: call("diagnostic_history", {kind: "speed"}, function(result) { editJson(tr("История Speed Test", "Speed Test history"), JSON.stringify(result, null, 2), "") }) }
                    }
                    BusyIndicator { visible: jobTimer.running; running: visible; implicitHeight: 30 }
                    Label { textFormat: Text.PlainText; text: job ? (job.cancelled ? tr("Отменено", "Cancelled") : job.running ? tr("Выполняется", "Running") : tr("Завершено", "Completed")) + " · " + job.completed + " / " + job.total : tr("Запустите проверку — здесь появятся реальные результаты", "Start a test to see real results here"); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    Label { textFormat: Text.PlainText; visible: !!(job && job.error); text: job && job.error ? job.error.message : ""; color: "#ef8fa0"; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                    Repeater { model: job ? job.results : []
                        delegate: ColumnLayout {
                            required property var modelData
                            readonly property var value: modelData.Ok || modelData
                            Layout.fillWidth: true
                            Label { textFormat: Text.PlainText; text: value.mode || value.profile_name || value.name || (job ? job.kind : ""); color: textColor; font.pixelSize: 13; font.weight: Font.DemiBold }
                            Label { textFormat: Text.PlainText; visible: value.download_mbps !== undefined; text: "↓ " + (value.download_mbps === null ? "—" : Number(value.download_mbps).toFixed(2)) + " Mbps     ↑ " + (value.upload_mbps === null ? "—" : Number(value.upload_mbps).toFixed(2)) + " Mbps"; color: mutedColor; font.pixelSize: 12 }
                            Label { textFormat: Text.PlainText; visible: value.median_ms !== undefined || value.mean_ms !== undefined; text: tr("Задержка: ", "Latency: ") + (value.median_ms || value.mean_ms || "—") + " ms"; color: mutedColor; font.pixelSize: 12 }
                            Label { textFormat: Text.PlainText; visible: value.suggested_mtu !== undefined; text: "MTU: " + (value.suggested_mtu || "—") + (value.estimate ? tr(" · расчётная оценка", " · estimate") : ""); color: mutedColor; font.pixelSize: 12 }
                            Label { textFormat: Text.PlainText; visible: !!(value.error || modelData.Err); text: value.error ? value.error.message : modelData.Err ? modelData.Err.message : ""; color: "#ef8fa0"; Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12 }
                        }
                    }
                    ActionButton { visible: !!job; text: tr("Подробности", "Details"); onClicked: editJson(tr("Подробности измерения", "Measurement details"), JSON.stringify(job, null, 2), "") }
                }
            }
        }
    }

    Component {
        id: playgroundPage
        ColumnLayout {
            spacing: 16
            RowLayout { Layout.fillWidth: true
                ComboBox { id: processPicker; Layout.fillWidth: true; model: [tr("Все соединения ядра", "All engine connections")].concat(processes.map(function(p) { return p.name + " · " + p.pid })); currentIndex: selectedProcess + 1; onActivated: selectedProcess = currentIndex - 1 }
                ActionButton { text: tr("Процессы", "Processes"); onClicked: call("playground_processes", {}, function(items) { processes = items || [] }) }
            }
            RowLayout {
                ActionButton { text: observing ? tr("Остановить", "Stop") : tr("Наблюдать", "Observe"); primary: !observing; onClicked: { observing = !observing; if (observing) observe() } }
                ActionButton { text: tr("Обновить", "Refresh"); enabled: !busy("connection_analysis"); onClicked: observe() }
                Label { textFormat: Text.PlainText; text: observing ? tr("Обновление каждые 5 секунд", "Refresh every 5 seconds") : tr("Наблюдение остановлено", "Observation stopped"); color: mutedColor; font.pixelSize: 11 }
            }
            Card { Layout.fillWidth: true; implicitHeight: analysisText.implicitHeight + 40
                Label { textFormat: Text.PlainText; id: analysisText; anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 20; text: analysis ? tr("Найдено соединений: ", "Connections found: ") + analysis.connections.length + (analysis.truncated ? tr(" · список ограничен", " · truncated") : "") : tr("Выберите процесс и начните наблюдение. Отображаются сведения Windows и ядра; содержимое HTTPS не читается.", "Choose a process and start observing. Shows Windows and engine metadata; HTTPS contents are not read."); wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
            }
            Repeater { model: analysis ? analysis.connections : []
                delegate: Card {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: connectionRow.implicitHeight + 28
                    ColumnLayout { id: connectionRow; anchors.fill: parent; anchors.margins: 14; spacing: 5
                        RowLayout { Layout.fillWidth: true
                            Label { textFormat: Text.PlainText; text: modelData.destination || modelData.local; color: textColor; Layout.fillWidth: true; elide: Text.ElideMiddle; font.pixelSize: 13 }
                            Label { textFormat: Text.PlainText; text: modelData.network.toUpperCase() + " · " + modelData.state; color: mutedColor; font.pixelSize: 11 }
                        }
                        Label { textFormat: Text.PlainText; text: (modelData.process || "—") + " · " + modelData.route + " · ↓ " + bytes(modelData.download) + " ↑ " + bytes(modelData.upload); color: mutedColor; Layout.fillWidth: true; elide: Text.ElideMiddle; font.pixelSize: 11 }
                    }
                }
            }
        }
    }

    Component {
        id: activityPage
        ColumnLayout {
            spacing: 14
            property var days: []
            property string fileLog: ""
            Component.onCompleted: call("log_days", {}, function(value) { days = value || [] })
            RowLayout { Layout.fillWidth: true
                ComboBox { id: logCategory; model: ["main", "core", "tunnel", "subscriptions", "service"] }
                ComboBox { id: logDay; model: parent.parent.days; Layout.fillWidth: true }
                ActionButton { text: tr("Прочитать файл", "Read log file"); enabled: logDay.currentText.length > 0; onClicked: { var view = parent.parent; call("log_read", {day: logDay.currentText, category: logCategory.currentText}, function(value) { view.fileLog = value }) } }
                ActionButton { text: tr("Папка", "Folder"); onClicked: call("open_log_folder") }
            }
            Card { visible: parent.fileLog.length > 0; Layout.fillWidth: true; implicitHeight: fileText.implicitHeight + 30
                Label { textFormat: Text.PlainText; id: fileText; anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 15; text: parent.parent.fileLog; color: mutedColor; font.family: "Consolas"; font.pixelSize: 11; wrapMode: Text.WrapAnywhere }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Последние события", "Recent events"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold; Layout.fillWidth: true }
                ActionButton { text: tr("Обновить", "Refresh"); onClicked: backend.refresh() }
                ActionButton { text: tr("Очистить", "Clear"); danger: true; onClicked: backend.confirmDelete("clear_logs", {}, tr("Очистить журнал событий?", "Clear the event log?")) }
            }
            Repeater { model: modelData.logs || []
                delegate: Card {
                    required property var modelData
                    Layout.fillWidth: true; implicitHeight: eventText.implicitHeight + 48
                    ColumnLayout { anchors.fill: parent; anchors.margins: 14; spacing: 6
                        Label { textFormat: Text.PlainText; text: new Date(modelData.timestamp * 1000).toLocaleTimeString() + "  ·  " + modelData.subsystem + "  ·  " + modelData.level; color: modelData.level === "ERROR" ? "#f48e9f" : mutedColor; font.pixelSize: 10 }
                        Label { textFormat: Text.PlainText; id: eventText; text: modelData.message; Layout.fillWidth: true; wrapMode: Text.WrapAnywhere; color: textColor; font.pixelSize: 12 }
                    }
                }
            }
        }
    }

    Component {
        id: settingsPage
        ColumnLayout {
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: appearanceForm.implicitHeight + 40
                ColumnLayout { id: appearanceForm; anchors.fill: parent; anchors.margins: 20; spacing: 18
                    Label { textFormat: Text.PlainText; text: tr("Внешний вид", "Appearance"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Тема", "Theme"); color: mutedColor; Layout.fillWidth: true }
                        ComboBox { model: ["dark", "light", "system"]; currentIndex: Math.max(0, model.indexOf(prefs.theme)); onActivated: save({theme: currentText}) }
                    }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Язык", "Language"); color: mutedColor; Layout.fillWidth: true }
                        ComboBox { model: ["Русский", "English"]; currentIndex: prefs.language === "en" ? 1 : 0; onActivated: save({language: currentIndex ? "en" : "ru"}) }
                    }
                    SettingRow { Layout.fillWidth: true; title: tr("Запускать с Windows", "Start with Windows"); description: tr("Запуск в системном трее", "Start in the system tray"); checked: !!prefs.autostart; onToggled: function(v) { save({autostart: v}) } }
                    SettingRow { Layout.fillWidth: true; title: tr("Закрывать в трей", "Close to tray"); description: tr("Кнопка закрытия скрывает окно; выход доступен в меню значка", "Closing hides the window; Quit is available in the tray menu"); checked: !!prefs.close_to_tray; onToggled: function(v) { save({close_to_tray: v}) } }
                    SettingRow { Layout.fillWidth: true; title: tr("Автовосстановление", "Auto reconnect"); description: tr("Параметр сетевого контроллера", "Network controller preference"); checked: !!prefs.auto_reconnect; onToggled: function(v) { save({auto_reconnect: v}) } }
                    SettingRow { Layout.fillWidth: true; title: tr("Автосмена сервера", "Automatic failover"); description: tr("После трёх неудачных проверок; текущие соединения могут оборваться", "After three failed probes; existing connections may be interrupted"); checked: !!prefs.auto_failover; onToggled: function(v) { save({auto_failover: v}) } }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: dataForm.implicitHeight + 40
                ColumnLayout { id: dataForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Данные и резервные копии", "Data and backups"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    Label { textFormat: Text.PlainText; text: tr("У Qt-версии отдельный каталог. Резервную копию NetPulse 3.0.4 можно восстановить под той же учётной записью Windows.", "The Qt version has a separate folder. NetPulse 3.0.4 backups can be restored under the same Windows account."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        ActionButton { text: tr("Создать копию", "Create backup"); onClicked: call("export_data", {kind: "backup"}) }
                        ActionButton { text: tr("Восстановить", "Restore"); onClicked: call("restore_data", {backup: true}) }
                        ActionButton { text: tr("Настройки JSON", "Settings JSON"); onClicked: call("export_data", {kind: "settings"}) }
                        ActionButton { text: tr("Папка данных", "Data folder"); onClicked: backend.openDataDirectory() }
                    }
                    ActionButton { text: tr("Редактор всех параметров…", "Edit all preferences…"); onClicked: editJson(tr("Параметры NetPulse", "NetPulse preferences"), JSON.stringify(prefs, null, 2), "save_preferences") }
                    Label { textFormat: Text.PlainText; text: "NetPulse 1.0.0 beta  ·  C++ / Qt Quick  ·  Rust NetworkCore"; color: mutedColor; font.pixelSize: 11 }
                }
            }
        }
    }

    Component {
        id: enginesPage
        ColumnLayout {
            spacing: 18
            Repeater { model: [{name: "sing-box", version: modelData.engine_version, ready: modelData.engine_ready}, {name: "Xray", version: modelData.xray_version, ready: modelData.xray_ready}]
                delegate: Card { required property var modelData; Layout.fillWidth: true; implicitHeight: 105
                    RowLayout { anchors.fill: parent; anchors.margins: 24
                        ColumnLayout { Layout.fillWidth: true; spacing: 7
                            Label { textFormat: Text.PlainText; text: modelData.name; color: textColor; font.pixelSize: 19; font.weight: Font.DemiBold }
                            Label { textFormat: Text.PlainText; text: modelData.version || "—"; color: mutedColor; font.pixelSize: 12 }
                        }
                        Label { textFormat: Text.PlainText; text: modelData.ready ? tr("● Проверено", "● Verified") : tr("● Недоступно", "● Unavailable"); color: modelData.ready ? "#69d5ac" : "#e0b86d"; font.pixelSize: 12 }
                    }
                }
            }
            RowLayout {
                ActionButton { text: tr("Вернуть комплектные", "Restore bundled"); enabled: !snapshot.engine_pid && !busy("activate_core_bundle"); onClicked: call("activate_core_bundle", {rollback: false}) }
                ActionButton { text: tr("Откатить ядра", "Roll back engines"); enabled: !snapshot.engine_pid; onClicked: call("activate_core_bundle", {rollback: true}) }
            }
            RowLayout {
                ActionButton { text: "sing-box " + tr("выпуски", "releases"); enabled: !busy("list_core_releases"); onClicked: call("list_core_releases", {engine: "sing-box"}, function(items) { coreReleases = items }) }
                ActionButton { text: "Xray " + tr("выпуски", "releases"); enabled: !busy("list_core_releases"); onClicked: call("list_core_releases", {engine: "xray"}, function(items) { coreReleases = items }) }
            }
            Label { textFormat: Text.PlainText; text: tr("Обновления загружаются из официальных источников и проходят проверку. Для замены ядра сначала отключитесь.", "Updates are downloaded from official sources and verified. Disconnect before replacing an engine."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
            Repeater { model: coreReleases
                delegate: Card { required property var modelData; Layout.fillWidth: true; implicitHeight: 72
                    RowLayout { anchors.fill: parent; anchors.margins: 14
                        Label { textFormat: Text.PlainText; text: modelData.engine + " · " + modelData.tag; Layout.fillWidth: true; color: textColor }
                        ActionButton { text: tr("Установить", "Install"); enabled: !snapshot.engine_pid && !busy("update_core"); onClicked: call("update_core", {release: modelData}) }
                    }
                }
            }
        }
    }

    Dialog {
        id: importDialog; anchors.centerIn: parent; width: 620; modal: true
        title: tr("Добавить подключение", "Add connection"); standardButtons: Dialog.NoButton
        ColumnLayout { anchors.fill: parent; spacing: 14
            Label { textFormat: Text.PlainText; text: tr("Вставьте ключ, HTTPS-ссылку подписки или конфигурацию", "Paste a profile link, HTTPS subscription URL or configuration"); color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
            ScrollView { Layout.fillWidth: true; Layout.preferredHeight: 170
                TextArea { id: importText; placeholderText: "vless://…  vmess://…  trojan://…  ss://…"; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; font.pixelSize: 13; color: textColor; background: Rectangle { color: palette.base; radius: 10; border.color: borderColor } }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { text: tr("Буфер", "Clipboard"); onClicked: importText.text = backend.clipboardText() }
                ActionButton { text: tr("Файл", "File"); onClicked: { call("import_file", {qr: false}); importDialog.close() } }
                ActionButton { text: "QR"; onClicked: { call("import_file", {qr: true}); importDialog.close() } }
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Отмена", "Cancel"); onClicked: { importText.clear(); importDialog.close() } }
                ActionButton { text: tr("Импорт", "Import"); primary: true; enabled: importText.text.length > 0 && !busy("import_text"); onClicked: call("import_text", {text: importText.text}, function() { importText.clear(); importDialog.close() }) }
            }
        }
    }
    Dialog {
        id: jsonDialog; anchors.centerIn: parent; width: Math.min(win.width - 100, 840); height: win.height - 120; modal: true
        property string saveMethod: ""
        property var parameters: ({})
        standardButtons: Dialog.NoButton
        ColumnLayout { anchors.fill: parent; spacing: 12
            ScrollView { Layout.fillWidth: true; Layout.fillHeight: true
                TextArea { id: jsonEditor; readOnly: jsonDialog.saveMethod === ""; selectByMouse: true; wrapMode: TextEdit.NoWrap; font.family: "Consolas"; font.pixelSize: 12; color: textColor; background: Rectangle { color: palette.base; radius: 10; border.color: borderColor } }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Проверка значений выполняется до применения", "Values are validated before applying"); color: mutedColor; font.pixelSize: 11; Layout.fillWidth: true }
                ActionButton { text: tr("Закрыть", "Close"); onClicked: { jsonEditor.clear(); jsonDialog.close() } }
                ActionButton { text: tr("Сохранить", "Save"); primary: true; visible: jsonDialog.saveMethod !== ""; enabled: !busy(jsonDialog.saveMethod); onClicked: {
                    try {
                        var value = JSON.parse(jsonEditor.text), p = clone(jsonDialog.parameters)
                        if (jsonDialog.saveMethod === "save_preferences") p.preferences = value
                        else p.json = jsonEditor.text
                        call(jsonDialog.saveMethod, p, function() { jsonEditor.clear(); jsonDialog.close(); toast(tr("Сохранено", "Saved")) })
                    } catch (e) { toast(tr("Некорректный JSON", "Invalid JSON")) }
                } }
            }
        }
    }
    Dialog {
        id: subEdit; anchors.centerIn: parent; width: 480; modal: true
        title: tr("Параметры подписки", "Subscription settings"); property var sub: ({})
        ColumnLayout { anchors.fill: parent; spacing: 14
            Input { id: subEditName; Layout.fillWidth: true }
            RowLayout { Label { textFormat: Text.PlainText; text: tr("Интервал, часов", "Interval, hours"); color: mutedColor } SpinBox { id: subEditHours; from: 1; to: 720; editable: true } }
            CheckBox { id: subEditLock; text: tr("Запретить редактирование профилей", "Lock profile configuration") }
            RowLayout { Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Отмена", "Cancel"); onClicked: subEdit.close() }
                ActionButton { text: tr("Сохранить", "Save"); primary: true; onClicked: call("save_subscription", {id: subEdit.sub.id, name: subEditName.text, interval_hours: subEditHours.value, protected: true, lock_configuration: subEditLock.checked}, function() { subEdit.close() }) }
            }
        }
    }
}
