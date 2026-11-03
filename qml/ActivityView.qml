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
    property var pageCursors: [null]
    property string nextCursor: ""
    property int pageNumber: 1
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
    function load(resetPage) {
        if (resetPage !== false) {pageCursors=[null];pageNumber=1;nextCursor=""}
        var generation = ++requestGeneration
        loadError="";loading=true;readRequestId=""
        var category = categories[logCategory.currentIndex], day = logDay.currentText
        if (category === "events") {
            readRequestId=appWindow.call("logs", {}, function(value) {
                if (generation !== view.requestGeneration) return
                view.loading=false
                view.displayedSource = "events"
                view.events = (value || []).filter(function(e){return view.levelFilter==="ALL"||e.level===view.levelFilter}).slice().reverse()
                view.loaded(view.displayedSource,view.events.length)
            })
            readRequestMethod="logs"
            checkRequestStarted()
            return
        }
        if (!day) { loading=false; displayedSource = category; events = []; return }
        readRequestId=appWindow.call("log_page", {day: day, category: category, cursor:pageCursors[pageNumber-1],level:levelFilter}, function(page) {
            if (generation !== view.requestGeneration) return
            view.loading=false
            view.nextCursor=page.next||""
            var parsed = String(page.text||"").split("\n").filter(function(line) { return line.trim().length }).map(function(line) {
                try { return JSON.parse(line) }
                catch (e) { return {raw: true, level: "INFO", message: line} }
            })
            view.displayedSource = category + ":" + day+":"+view.levelFilter+":"+view.pageNumber
            view.events = parsed
            view.loaded(view.displayedSource,view.events.length)
        })
        readRequestMethod="log_page"
        checkRequestStarted()
    }
    function olderPage() {
        if (!nextCursor||loading) return
        follow=false;pageCursors=pageCursors.slice(0,pageNumber).concat([nextCursor]);pageNumber++;load(false)
    }
    function newerPage() {if(pageNumber<=1||loading)return;follow=false;pageNumber--;load(false)}
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
        ActionButton { appWindow: view.appWindow; text: view.tr("Обновить", "Refresh"); onClicked: view.discover() }
        ActionButton { appWindow: view.appWindow; text: view.tr("Папка", "Folder"); onClicked: view.bridge.openLogDirectory() }
    }
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
    RowLayout {
        Layout.fillWidth: true
        visible: logCategory.currentIndex!==5
        ActionButton {appWindow:view.appWindow;text:view.tr("← Новее","← Newer");enabled:!view.loading&&view.pageNumber>1;onClicked:view.newerPage()}
        Label {text:view.tr("Страница ","Page ")+view.pageNumber;color:view.appWindow.mutedColor}
        ActionButton {appWindow:view.appWindow;text:view.tr("Старее →","Older →");enabled:!view.loading&&view.nextCursor.length>0;onClicked:view.olderPage()}
        Item {Layout.fillWidth:true}
        Label {text:view.tr("Хранение: 5 дней","Retention: 5 days");color:view.appWindow.mutedColor;font.pixelSize:11}
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
        Layout.fillHeight: true
        Layout.minimumHeight: 0
        Layout.preferredHeight: 420
        appWindow: view.appWindow
        rows: view.renderRows()
        sourceKey: view.displayedSource
        follow: view.follow
        emptyText: view.tr("В выбранном журнале пока нет записей", "No entries in this journal yet")
        onReadingStarted: view.follow=false
        onCopyRequested: function(text) { view.bridge.copyText(text); view.appWindow.toast(view.tr("Скопировано", "Copied")) }
    }
}
