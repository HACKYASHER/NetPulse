import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: view
    property var appWindow: ApplicationWindow.window
    property var bridge: backend
    property var days: []
    property var events: []
    property bool follow: true
    property int requestGeneration: 0
    property string displayedSource: ""
    property bool loading: false
    property string loadError: ""
    property string readRequestId: ""
    property var categories: ["main", "core", "tunnel", "subscriptions", "service", "events"]
    signal loaded(string source,int count)
    function selectCategory(index){logCategory.currentIndex=index;load()}
    spacing: 12

    function tr(ru, en) { return appWindow.tr(ru, en) }
    function renderRows() {
        return events.map(function(event) {
            var text = event.raw ? event.message : view.appWindow.logMessage(event)
            return {key: event.id || "", level: event.level || "INFO", text: text}
        })
    }
    function load() {
        var generation = ++requestGeneration
        loadError="";loading=true;readRequestId=""
        var category = categories[logCategory.currentIndex], day = logDay.currentText
        if (category === "events") {
            appWindow.call("logs", {}, function(value) {
                if (generation !== view.requestGeneration) return
                view.loading=false
                view.displayedSource = "events"
                view.events = (value || []).slice().reverse()
                view.loaded(view.displayedSource,view.events.length)
            })
            return
        }
        if (!day) { loading=false; displayedSource = category; events = []; return }
        readRequestId=appWindow.call("log_read", {day: day, category: category}, function(text) {
            if (generation !== view.requestGeneration) return
            view.loading=false
            var parsed = String(text).split("\n").filter(function(line) { return line.trim().length }).map(function(line) {
                try { return JSON.parse(line) }
                catch (e) { return {raw: true, level: "INFO", message: line} }
            })
            view.displayedSource = category + ":" + day
            view.events = parsed
            view.loaded(view.displayedSource,view.events.length)
        })
    }
    function discover() {
        appWindow.call("log_days", {}, function(value) {
            var old = logDay.currentText, next = value || []
            // Reassigning an unchanged ComboBox model resets its current item and viewport.
            if (JSON.stringify(next) !== JSON.stringify(view.days)) view.days = next
            Qt.callLater(function(){
                logDay.currentIndex = view.follow ? 0 : Math.max(0, view.days.indexOf(old))
                view.load()
            })
        })
    }
    Connections {
        target: view.bridge
        ignoreUnknownSignals: true
        function onCompleted(id,method,result,error) {
            if (id===view.readRequestId&&method==="log_read"&&error&&error.code) {
                view.loading=false;view.loadError=error.message||view.tr("Не удалось прочитать журнал","Could not read log")
            }
        }
    }
    Component.onCompleted: discover()
    Timer {
        interval: 2500
        repeat: true
        running: view.follow
        onTriggered: if (!view.appWindow.busy("log_read") && !view.appWindow.busy("logs") && !view.appWindow.busy("log_days")) view.discover()
    }
    RowLayout {
        Layout.fillWidth: true
        Choice { appWindow: view.appWindow;
            id: logCategory
            objectName: "logCategory"
            model: [view.tr("Основной", "Main"), view.tr("Ядро", "Core"), view.tr("Туннель", "Tunnel"), view.tr("Подписки", "Subscriptions"), view.tr("Служба", "Service"), view.tr("События", "Events")]
            onActivated: Qt.callLater(function(){view.load()})
        }
        Choice { appWindow: view.appWindow;
            id: logDay
            objectName: "logDay"
            Layout.fillWidth: true
            model: view.days
            enabled: logCategory.currentIndex !== 5
            onActivated: { view.follow = false; view.load() }
        }
        ActionButton { appWindow: view.appWindow; text: view.tr("Обновить", "Refresh"); onClicked: view.discover() }
        ActionButton { appWindow: view.appWindow; text: view.tr("Папка", "Folder"); onClicked: view.appWindow.call("open_log_folder") }
    }
    RowLayout {
        Layout.fillWidth: true
        Label { text: view.tr("Уровень", "Level"); color: view.appWindow.mutedColor }
        Choice {
            appWindow: view.appWindow
            objectName: "runtimeLogLevel"
            Layout.preferredWidth: 170
            model: [view.tr("Отладка", "Debug"),view.tr("Информация", "Information"),view.tr("Предупреждения", "Warnings"),view.tr("Ошибки", "Errors")]
            values: ["DEBUG","INFO","WARNING","ERROR"]
            currentIndex: Math.max(0,values.indexOf(view.appWindow.prefs.logging_level||"INFO"))
            enabled: !view.appWindow.savingPreferences&&!view.appWindow.busy("connect")&&!view.appWindow.busy("select_profile")
            ToolTip.visible: hovered
            ToolTip.text: view.tr("INFO показывает соединения; DEBUG добавляет технические подробности. Изменение перезапускает активное соединение с проверкой и восстановлением при ошибке.","INFO shows connections; DEBUG adds technical detail. Changing this restarts the active connection with validation and rollback on failure.")
            onActivated: view.appWindow.save({logging_level:selectedValue})
        }
        Label { text: view.tr("Следовать за журналом", "Follow latest log"); color: view.appWindow.mutedColor }
        Toggle { checked: view.follow; onToggled: { view.follow = checked; if (checked) view.discover() } }
        Item { Layout.fillWidth: true }
        Label { text: view.events.length + view.tr(" строк", " lines"); color: view.appWindow.mutedColor; font.pixelSize: 11 }
        ActionButton { appWindow: view.appWindow;
            text: view.tr("Копировать журнал", "Copy log")
            enabled: view.events.length > 0
            onClicked: { view.bridge.copyText(logView.plainText); view.appWindow.toast(view.tr("Скопировано", "Copied")) }
        }
    }
    Label {
        visible: view.loading||view.loadError.length>0
        text: view.loadError||view.tr("Загрузка журнала…","Loading log…")
        color: view.loadError.length>0?(view.appWindow.light?"#a62442":"#ff8e9f"):view.appWindow.mutedColor
        Layout.fillWidth: true; wrapMode: Text.Wrap
    }
    ConsoleView {
        id: logView
        objectName: "activityConsole"
        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(420, view.appWindow.height - 330)
        appWindow: view.appWindow
        rows: view.renderRows()
        sourceKey: view.displayedSource
        follow: view.follow
        emptyText: view.tr("В выбранном журнале пока нет записей", "No entries in this journal yet")
        onReadingStarted: view.follow=false
        onCopyRequested: function(text) { view.bridge.copyText(text); view.appWindow.toast(view.tr("Скопировано", "Copied")) }
    }
}
