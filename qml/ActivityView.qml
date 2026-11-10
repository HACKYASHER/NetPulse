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
    property string readRequestMethod: ""
    property string levelFilter: "ALL"
    property string olderCursor: ""
    readonly property bool consoleFitsViewport: logView.y >= 0 && Math.abs(logView.y + logView.height - height) < 1
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
    function parseRows(text) {
        return String(text || "").split("\n").filter(function(line) { return line.trim().length }).map(function(line) {
            try { return JSON.parse(line) }
            catch (e) { return {raw: true, level: "INFO", message: line} }
        })
    }
    function eventKey(event) { return event.id || JSON.stringify(event) }
    function finishLoad(source, rows, cursor) {
        displayedSource=source
        events=rows
        olderCursor=cursor || ""
        loading=false
        readRequestId=""
        loaded(source,rows.length)
        Qt.callLater(function(){logView.checkHistory()})
    }
    function readPage(generation, day, category, cursor, done) {
        readRequestMethod="log_page"
        readRequestId=appWindow.call("log_page", {day:day, category:category, cursor:cursor || null, level:levelFilter}, function(page) {
            if (generation !== view.requestGeneration) return
            done(view.parseRows(page.text),page.next || "")
        })
        checkRequestStarted()
    }
    function load() {
        var generation = ++requestGeneration
        loadError="";loading=true;readRequestId=""
        var category = categories[logCategory.currentIndex], day = logDay.currentText
        var source=category+":"+day+":"+levelFilter
        if (category === "events") {
            readRequestMethod="logs"
            readRequestId=appWindow.call("logs", {}, function(value) {
                if (generation !== view.requestGeneration) return
                view.finishLoad(source,(value || []).filter(function(e){return view.levelFilter==="ALL"||e.level===view.levelFilter}).slice().reverse(),"")
            })
            checkRequestStarted()
            return
        }
        if (!day) { finishLoad(source,[],""); return }
        var previous=displayedSource===source ? events : []
        var anchor=previous.length ? eventKey(previous[previous.length-1]) : ""
        var accumulated=[], visited=({})
        // Catch up across as many transport chunks as needed; a burst must not
        // discard entries between the displayed tail and the newest 500 records.
        function accept(rows,next) {
            accumulated=rows.concat(accumulated)
            if (!anchor) {view.finishLoad(source,accumulated,next);return}
            for (var i=rows.length-1;i>=0;--i) {
                if (view.eventKey(rows[i])===anchor) {
                    view.finishLoad(source,previous.concat(accumulated.slice(i+1)),view.olderCursor)
                    return
                }
            }
            if (!next) {view.finishLoad(source,accumulated,"");return}
            if (visited[next]) {view.loading=false;view.loadError=view.tr("Журнал изменился. Повторите обновление.","The log changed. Refresh to retry.");return}
            visited[next]=true
            view.readPage(generation,day,category,next,accept)
        }
        readPage(generation,day,category,null,function(rows,next) {
            accept(rows,next)
        })
    }
    function loadOlder() {
        if (!olderCursor || loading || displayedSource!==categories[logCategory.currentIndex]+":"+logDay.currentText+":"+levelFilter) return
        var generation=++requestGeneration, source=displayedSource
        var cursor=olderCursor
        loading=true;loadError=""
        readPage(generation,logDay.currentText,categories[logCategory.currentIndex],cursor,function(rows,next) {
            if (next===cursor) {view.loading=false;view.loadError=view.tr("Журнал изменился. Повторите обновление.","The log changed. Refresh to retry.");return}
            view.finishLoad(source,rows.concat(view.events),next)
        })
    }
    function checkRequestStarted() {
        if (!readRequestId) {loading=false;loadError=tr("Не удалось начать чтение журнала. Повторите обновление.","Could not start reading the log. Refresh to retry.")}
    }
    function discover() {
        var generation=requestGeneration
        appWindow.call("log_days", {}, function(value) {
            if (generation!==view.requestGeneration) return
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
            if (id===view.readRequestId&&method===view.readRequestMethod&&error&&error.code) {
                view.loading=false;view.loadError=error.message||view.tr("Не удалось прочитать журнал","Could not read log")
            }
        }
    }
    Component.onCompleted: discover()
    Timer {
        interval: 2500
        repeat: true
        running: view.follow
        onTriggered: if (!view.loading && !view.appWindow.busy("log_page") && !view.appWindow.busy("logs") && !view.appWindow.busy("log_days")) view.discover()
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
        ActionButton { objectName: "logRefresh"; appWindow: view.appWindow; text: view.tr("Обновить", "Refresh"); onClicked: view.discover() }
        ActionButton { objectName: "logFolder"; appWindow: view.appWindow; text: view.tr("Папка", "Folder"); onClicked: view.bridge.openLogDirectory() }
    }
    GridLayout {
        Layout.fillWidth: true
        columns: view.width<1000 ? 2 : 3
        columnSpacing: 12
        rowSpacing: 8
        RowLayout {
        Layout.fillWidth: true
        Label { text: view.tr("Уровень", "Level"); color: view.appWindow.mutedColor }
        Choice {
            appWindow: view.appWindow
            objectName: "runtimeLogLevel"
            Layout.preferredWidth: 170
            model: [view.tr("Все записи", "All entries"),view.tr("Трассировка", "Trace"),view.tr("Отладка", "Debug"),view.tr("Информация", "Information"),view.tr("Предупреждения", "Warnings"),view.tr("Ошибки", "Errors")]
            values: ["ALL","TRACE","DEBUG","INFO","WARNING","ERROR"]
            currentIndex: Math.max(0,values.indexOf(view.levelFilter))
            ToolTip.visible: hovered
            ToolTip.text: view.tr("Фильтр отображения. «Все записи» показывает все сохранённые уровни. Соединение не прерывается. Журналы хранятся пять дней.","Display filter. All entries shows every recorded level. The connection stays active. Logs are retained for five days.")
            onActivated: {view.levelFilter=selectedValue;view.load()}
        }
        Item { Layout.fillWidth: true }
        }
        RowLayout {
        Layout.fillWidth: true
        Label { text: view.tr("Следовать за журналом", "Follow latest log"); color: view.appWindow.mutedColor }
        Toggle { checked: view.follow; onToggled: { view.follow = checked; if (checked) view.discover() } }
        }
        RowLayout {
        Layout.columnSpan: view.width<1000 ? 2 : 1
        Layout.fillWidth: true
        Item { Layout.fillWidth: true }
        Label { text: view.events.length + (view.olderCursor ? "+" : "") + view.tr(" строк", " lines"); color: view.appWindow.mutedColor; font.pixelSize: 11 }
        ActionButton { objectName: "logLatest"; appWindow: view.appWindow; text: view.tr("К последним ↓", "Latest ↓"); onClicked: {view.follow=true;logView.scrollToBottom();view.discover()} }
        ActionButton { appWindow: view.appWindow;
            objectName: "logCopy"
            text: view.tr("Копировать журнал", "Copy log")
            enabled: view.events.length > 0
            onClicked: { view.bridge.copyText(logView.plainText); view.appWindow.toast(view.tr("Скопировано", "Copied")) }
        }
        }
    }
    Label {
        text: view.loadError||(view.loading ? view.tr("Загрузка журнала…","Loading log…") : view.tr("Хранение: 5 дней · Старые записи подгружаются при прокрутке вверх", "Retention: 5 days · Scroll up to read earlier entries"))
        color: view.loadError.length>0?(view.appWindow.light?"#a62442":"#ff8e9f"):view.appWindow.mutedColor
        Layout.fillWidth: true; wrapMode: Text.Wrap
    }
    ConsoleView {
        id: logView
        objectName: "activityConsole"
        Layout.fillWidth: true
        Layout.fillHeight: true
        Layout.minimumHeight: 0
        Layout.preferredHeight: 420
        appWindow: view.appWindow
        rows: view.renderRows()
        sourceKey: view.displayedSource
        follow: view.follow
        hasOlder: view.olderCursor.length>0
        onHistoryRequested: view.loadOlder()
        emptyText: view.tr("В выбранном журнале пока нет записей", "No entries in this journal yet")
        onReadingStarted: view.follow=false
        onCopyRequested: function(text) { view.bridge.copyText(text); view.appWindow.toast(view.tr("Скопировано", "Copied")) }
    }
}
