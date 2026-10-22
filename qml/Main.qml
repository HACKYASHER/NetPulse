import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ApplicationWindow {
    id: win
    objectName: "netpulseWindow"
    width: 1220; height: 840; minimumWidth: 1000; minimumHeight: 700
    visible: !startHidden || smokeMode
    title: "NetPulse"
    property var modelData: ({})
    property var persistedPreferences: ({})
    property var prefs: Object.assign({},persistedPreferences,sentPreferences,preferenceQueue)
    property var sentPreferences: ({})
    property var snapshot: backend.data.snapshot || ({state: "disconnected"})
    property var profiles: modelData.profiles || []
    property var subscriptions: modelData.subscriptions || []
    property bool light: prefs.theme === "light" || (prefs.theme === "system" && Application.styleHints.colorScheme === Qt.ColorScheme.Light)
    readonly property color bgColor: light ? "#f4f5fa" : "#10121b"
    readonly property color panelColor: light ? "#ffffff" : "#191c28"
    readonly property color sidebarColor: light ? "#eceef6" : "#141620"
    readonly property color borderColor: light ? "#dfe2ed" : "#2a2e3e"
    readonly property color textColor: light ? "#222536" : "#edf0fa"
    readonly property color mutedColor: light ? "#536078" : "#a7b1c8"
    readonly property color accentColor: light ? "#6047ce" : "#aa9aff"
    property int page: 0
    property var titles: [tr("Обзор", "Overview"), tr("Серверы", "Servers"), tr("Маршрутизация", "Routing"), tr("Сеть", "Network"), tr("Диагностика", "Diagnostics"), "Playground", tr("Активность", "Activity"), tr("Настройки", "Settings")]
    property var captions: [tr("Ваше подключение под контролем","Your connection, under control"),tr("Серверы и подписки","Servers and subscriptions"),tr("Выберите, куда направлять трафик","Choose where your traffic goes"),tr("DNS, MTU, транспорт и ядра","DNS, MTU, transport and engines"),tr("Измерения на реальных соединениях","Measurements over real connections"),tr("Процессы и их соединения","Processes and their connections"),tr("События приложения и сетевых компонентов","Application and network component events"),tr("Внешний вид, обновления и данные","Appearance, updates and data")]

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
    property var ipResult: ({})
    property var activeProbe: ({id:"",ms:null})
    property var clientUpdate: null
    property string updateStatus: ""
    property bool updateDownloaded: false
    property bool startupChecked: false
    property bool showAddedApps: true
    property string sourceFilter: "all"
    property int serverSort: 0
    property var serverContext: ({})
    property int selectedProcess: -1
    property var observedProcess: null
    property var flowRecords: ({})
    property var flowSamples: ({})
    property bool observing: false
    property int observationSeconds: 0
    property int smokePages: 0
    property int smokeStep: 0
    property string smokeFrame: "overview"
    property string smokeCaptured: ""
    property var smokeSequence: [{page:0,name:"overview"},{page:1,name:"servers"},{page:2,name:"routing"},{page:3,tab:0,name:"connection"},{page:3,tab:1,name:"dns"},{page:3,tab:2,name:"mtu"},{page:3,tab:3,name:"tls"},{page:3,tab:4,name:"transport"},{page:3,tab:5,name:"service"},{page:3,tab:6,name:"engines"},{page:4,name:"diagnostics"},{page:5,name:"playground"},{page:6,name:"activity"},{page:7,name:"settings"},{page:0,dialog:"mode",name:"connection-mode"},{page:1,dialog:"import",name:"add-connection"},{page:4,dialog:"history",name:"measurement-history"}]
    property bool smokeFinished: false
    property var pages: [overviewPage, serversPage, routingPage, networkPage, diagnosticsPage, playgroundPage, activityPage, settingsPage]
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
        var patch = preferenceQueue; sentPreferences=patch; preferenceQueue = ({}); savingPreferences = true
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
        modeDialog.open()
    }
    function showServers(){selectPage(1)}
    function showNetwork(){selectPage(3)}
    function showSettings(){selectPage(7)}
    function showPublicIps(){ipResult=({});ipDialog.open();call("public_ips",{},function(value){ipResult=value})}
    function reconnectSelected(){modeDialog.open()}
    function connectTun(){modeDialog.start(true,false)}
    function connectProxy(){modeDialog.start(false,true)}
    function beginJob(method, params) { call(method, params, function(id) { diagnosticId = id; job = null; jobTimer.start() }) }
    function editJson(title, content, saveMethod, params) {
        jsonDialog.title = title; jsonEditor.text = content; jsonDialog.saveMethod = saveMethod; jsonDialog.parameters = params || {}; jsonDialog.open()
    }
    function selectPage(index) {
        page = index
        if (index !== 5) observing = false
        if (index === 2) call("read_application_catalog", {}, function(items) { applications = items || [] })

        if (index === 6) backend.refresh()
    }
    function processKey(p) { return p?exe(p.path)+":"+(p.pid||"all"):"engine" }
    function observe() {
        if(busy("connection_analysis"))return
        var p=observedProcess,key=processKey(p)
        call("connection_analysis",{pid:p?p.pid:null,path:p?p.path:""},function(r){
            if(processKey(observedProcess)===key)analysis=r
            var before=flowSamples[key]||{},next={},log=(flowRecords[key]||[]).slice(),at=new Date().toLocaleTimeString()
            function describe(c){return (c.pid?"PID "+c.pid+"  ":"")+c.network.toUpperCase()+"  "+c.local+" → "+(c.destination||"UDP")+"  "+(c.route||tr("Windows / маршрут неизвестен","Windows / unknown route"))+"  ↓ "+bytes(c.download)+" ↑ "+bytes(c.upload)}
            ;(r.connections||[]).forEach(function(c){var id=c.id||c.network+c.local+c.destination;next[id]=c;if(!before[id])log.push(at+"  ["+tr("ОБНАРУЖЕНО","OBSERVED")+"]  "+describe(c));else if(before[id].state!==c.state)log.push(at+"  ["+c.state.toUpperCase()+"]  "+describe(c))})
            Object.keys(before).forEach(function(id){if(!next[id])log.push(at+"  ["+tr("ЗАКРЫТО","CLOSED")+"]  "+describe(before[id]))})
            var samples=Object.assign({},flowSamples),records=Object.assign({},flowRecords);samples[key]=next;records[key]=log.slice(-300)
            while(Object.keys(records).length>20){var oldest=Object.keys(records)[0];delete records[oldest];delete samples[oldest]}
            flowSamples=samples;flowRecords=records
        })
    }
    function exe(path) { return String(path).replace(/\\/g,"/").split("/").pop().toLowerCase() }
    function cleanName(name) { return String(name||"").replace(/[\uD83C][\uDDE6-\uDDFF][\uD83C][\uDDE6-\uDDFF]\s*/g,"") }
    function displayedServer() { return snapshot.active || selected() }
    function pinned(id) { return (prefs.favorites||[]).indexOf(id)>=0 }
    function serverGroups() {
        var groups=subscriptions.map(function(x){ return Object.assign({},x,{items:[]}) })
        groups.push({id:"",name:tr("Отдельные серверы","Standalone servers"),items:[]})
        profiles.forEach(function(p){
            if ((cleanName(p.name)+" "+p.host+" "+p.protocol).toLowerCase().indexOf(profileSearch.toLowerCase())<0) return
            if(sourceFilter!=="all" && (p.subscription_id||"")!==sourceFilter) return
            var g=groups.find(function(g){return g.id===(p.subscription_id||"")});if(g)g.items.push(p)
        })
        groups.forEach(function(g){g.items.sort(function(a,b){
            var pins=Number(pinned(b.id))-Number(pinned(a.id));if(pins)return pins
            if(serverSort===1)return (a.latency_ms===null?1e9:a.latency_ms)-(b.latency_ms===null?1e9:b.latency_ms)
            return cleanName(a.name).localeCompare(cleanName(b.name))
        })})
        return groups.filter(function(g){return g.id ? sourceFilter==="all"||sourceFilter===g.id : g.items.length>0})
    }
    function refreshPing() {
        if(!snapshot.active || busy("active_latency"))return
        var id=snapshot.active.id
        call("active_latency",{id:id},function(r){if(snapshot.active&&snapshot.active.id===id)activeProbe={id:id,ms:r.avg_ms}})
    }
    function checkUpdate(manual) {
        if(busy("check_client_update"))return
        if(manual)updateStatus=tr("Проверка…","Checking…")
        call("check_client_update",{},function(r){
            clientUpdate=r.release||null
            updateStatus=r.error ? r.error.message : clientUpdate ? tr("Доступна версия ","Available version ")+clientUpdate.tag : tr("Установлена актуальная версия","You are up to date")
            if(manual)toast(updateStatus)
        })
    }
    function routingDocument() { return {version:1,route_mode:prefs.route_mode,applications:prefs.applications||[],application_groups:prefs.application_groups||[],bypass_domains:prefs.bypass_domains||[],routing_rules:prefs.routing_rules||[],sniff_excluded_apps:prefs.sniff_excluded_apps||[]} }
    function routingApps() {
        var result=[],seen={}
        applications.forEach(function(a){var name=exe(a.path);if(seen[name])return;seen[name]=true;if(!showAddedApps||(prefs.applications||[]).indexOf(name)>=0)result.push(a)})
        ;(prefs.applications||[]).forEach(function(name){if(!seen[name])result.push({name:name,path:name,source:"saved"})})
        return result.filter(function(a){return (a.name+" "+exe(a.path)).toLowerCase().indexOf(appSearch.toLowerCase())>=0})
    }
    function editSubscription(sub) { subEdit.sub=sub;subEditName.text=sub.name;subEditHours.value=sub.interval_hours;subEditProtect.checked=sub.url_protected;subEditLock.checked=sub.config_locked;subEdit.open() }
    function logMessage(event) { return new Date(event.timestamp*1000).toLocaleString()+"  ["+event.level+"]  "+event.subsystem+" / "+event.code+"  "+backend.readableText(event.message) }
    function resultText(r) {
        if(r.comparison)return r.comparison.suggested_mtu?tr("Лучший результат: MTU ","Best result: MTU ")+r.comparison.suggested_mtu:tr("MTU: убедительного улучшения нет","MTU: no reliable improvement")
        if(r.avg_ms!==undefined)return tr("Задержка: ","Latency: ")+(r.avg_ms===null?"—":Number(r.avg_ms).toFixed(1)+" ms")+"   "+r.successes+" / "+(r.successes+r.failures+r.timeouts)+tr(" успешно"," successful")+"   "+r.method
        if(r.download_mbps!==undefined)return "↓ "+(r.download_mbps===null?"—":Number(r.download_mbps).toFixed(2))+" Mbps   ↑ "+(r.upload_mbps===null?"—":Number(r.upload_mbps).toFixed(2))+" Mbps   · "+r.mode
        if(r.suggested_mtu!==undefined)return "MTU: "+(r.suggested_mtu||"—")+" · "+(r.estimate?tr("расчётная рекомендация","estimated recommendation"):tr("измерено","measured"))+" · "+r.target
        return JSON.stringify(r)
    }
    function tuningReason(reason) {
        if(prefs.language==="en")return reason
        var messages={
            "No reliable improvement over the original MTU":"Убедительного улучшения относительно исходного MTU нет",
            "Baseline measurement failed; keep the original MTU":"Исходное измерение не удалось; сохраните текущий MTU",
            "The connection changed during the test; repeat before choosing an MTU":"Условия соединения изменились во время теста; повторите проверку",
            "The leading candidate needs a repeated measurement":"Лучший кандидат требует повторного измерения",
            "Repeated candidate measurements were inconsistent":"Повторные измерения кандидата дали нестабильные результаты",
            "Best repeated result in this test; apply manually and verify your applications":"Лучший повторённый результат этого теста; примените вручную и проверьте свои приложения"
        }
        return messages[reason]||reason
    }
    onClosing: function(close) { close.accepted = false; if (prefs.close_to_tray && !smokeMode) hide(); else backend.quit() }
    onVisibilityChanged: if (visibility === Window.Minimized && prefs.minimize_to_tray && !smokeMode) hide()
    Connections {
        target: backend
        function onCompleted(id, method, result, error) {
            if(method==="bootstrap"&&!error.code&&result) {
                modelData=result
                if(JSON.stringify(persistedPreferences)!==JSON.stringify(result.preferences))persistedPreferences=result.preferences
            }
            var done = callbacks[id]
            if (done) { var c = Object.assign({}, callbacks); delete c[id]; callbacks = c; if (!error.code) done(result) }
            if (method === "save_preferences") {
                if (error.code) { savingPreferences = false; sentPreferences=({}); preferenceQueue = ({}); backend.refresh() }
                else waitingPreferencesRefresh = true
            }
            if (method === "bootstrap" && waitingPreferencesRefresh) {
                waitingPreferencesRefresh = false; savingPreferences = false; sentPreferences=({}); flushPreferences()
            }
            if (method === "bootstrap" && result && !error.code && !smokeMode && !startupChecked) { startupChecked=true;checkUpdate(false) }
            if (method === "bootstrap" && result && !error.code && smokeMode && result.engine_ready && !smokeTimer.running && !smokeFinished) smokeTimer.start()
            if (!error.code && (method === "import_text" || method === "import_file" || method === "import_clipboard" || method === "update_subscription")) {
                if (result) toast(tr("Импортировано профилей: ", "Profiles imported: ") + result.imported)
            }
        }
    }
    Timer { interval: 15000; running: backend.ready && !smokeMode && !!snapshot.engine_pid && !(job&&job.running&&job.kind==="mtu_tuning"); repeat: true; triggeredOnStart: true; onTriggered: refreshPing() }
    Timer { interval: 6*60*60*1000; running: backend.ready && !smokeMode; repeat: true; onTriggered: checkUpdate(false) }
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
    Timer { id: smokeTimer; interval: 750; repeat: true; onTriggered: {
        if(pageLoader.status!==Loader.Ready||smokeCaptured!==smokeFrame)return
        smokeCaptured=""
        modeDialog.close();importDialog.close();historyDialog.close()
        smokeStep++
        if(smokeStep>=smokeSequence.length){stop();page=0;smokePages=8;smokeFinished=true;return}
        var step=smokeSequence[smokeStep];page=step.page
        Qt.callLater(function(){
            if(step.tab!==undefined&&pageLoader.item)pageLoader.item.tab=step.tab
            if(step.dialog==="mode")modeDialog.open()
            if(step.dialog==="import")importDialog.open()
            if(step.dialog==="history"){historyDialog.open();historyDialog.load()}
            smokeFrame=step.name
        })
    } }


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
                            Label { textFormat: Text.PlainText; text: ["◉", "⇄", "⑂", "⌘", "⌁", "◎", "≡", "⚙"][index]; color: page === index ? accentColor : mutedColor; font.pixelSize: 20; Layout.preferredWidth: 22; horizontalAlignment: Text.AlignHCenter }
                            Label { textFormat: Text.PlainText; text: modelData; color: page === index ? textColor : mutedColor; font.pixelSize: 13; font.weight: page === index ? Font.DemiBold : Font.Normal; Layout.fillWidth: true }
                        }
                        background: Rectangle { color: page === index ? (light ? "#e0dcfb" : "#2a2544") : hovered ? (light ? "#e5e8f1" : "#1c2030") : "transparent"; radius: 10 }
                        onClicked: selectPage(index)
                    }
                }
                Item { Layout.fillHeight: true }
                ActionButton { Layout.fillWidth: true; visible: !!clientUpdate; text: tr("Обновить клиент","Update client"); primary: true; onClicked: updateDialog.open() }
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
                        RowLayout { Layout.fillWidth: true; spacing: 9
                            Flag { visible: !!displayedServer(); name: displayedServer()?displayedServer().name:""; Layout.preferredWidth: 27; Layout.preferredHeight: 18 }
                            Label { textFormat: Text.PlainText; text: displayedServer()?cleanName(displayedServer().name):tr("Добавьте сервер или подписку","Add a server or subscription"); color: textColor; Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 14 }
                        }
                        RowLayout {
                            Label { textFormat: Text.PlainText; text: "● " + (snapshot.engine_pid ? (snapshot.tun_active ? "TUN" : "System Proxy") : tr("Отключено","Offline")); color: accentColor; font.pixelSize: 12 }
                            Label { textFormat: Text.PlainText; text: "  ·  " + (prefs.transport_engine || "auto"); color: mutedColor; font.pixelSize: 12 }
                        }
                        Item { implicitHeight: 6 }
                        RowLayout {
                            ActionButton { text: tr("Выбрать сервер", "Choose server"); onClicked: selectPage(1) }
                            ActionButton { objectName: "overviewAdd"; text: tr("Добавить", "Add"); onClicked: { selectPage(1);importDialog.open() } }
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
                Metric { Layout.fillWidth: true; label: tr("ЗАДЕРЖКА", "LATENCY"); value: snapshot.active && activeProbe.id===snapshot.active.id && activeProbe.ms!==null ? Number(activeProbe.ms).toFixed(0)+" ms" : snapshot.engine_pid && snapshot.probe_ms!==null && snapshot.probe_ms!==undefined ? snapshot.probe_ms+" ms" : "—"; hint: tr("HTTPS-проверка соединения", "Connection HTTPS probe") }
            }
            Card { Layout.fillWidth: true; implicitHeight: 96
                RowLayout { anchors.fill: parent; anchors.margins: 20
                    ColumnLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Транспортное ядро","Transport engine"); color: textColor; font.bold: true }
                        Label { textFormat: Text.PlainText; text: tr("Авто выбирает совместимый транспорт. TUN и правила приложений обслуживает sing-box; некоторые транспорты требуют Xray.","Auto chooses a compatible transport. sing-box handles TUN and app rules; some transports require Xray."); color: mutedColor; font.pixelSize: 11 }
                    }
                    Choice { objectName: "overviewEngine"; model: ["auto","sing-box","xray"]; currentIndex: Math.max(0,model.indexOf(prefs.transport_engine||"auto")); onActivated: call("quick_network",{transport_engine:currentText}) }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Серверов: ", "Servers: ") + profiles.length + tr("    Подписок: ", "    Subscriptions: ") + subscriptions.length; color: mutedColor; font.pixelSize: 12; Layout.fillWidth: true }
                ActionButton { text: tr("Проверить IP", "Check IP"); enabled: !busy("public_ips"); onClicked: showPublicIps() }
            }
        }
    }

    Component {
        id: serversPage
        ColumnLayout {
            spacing: 14
            Input { objectName: "serverSearch"; Layout.fillWidth: true; placeholderText: tr("Поиск серверов…","Search servers…"); text: profileSearch; onTextChanged: profileSearch=text }
            RowLayout { Layout.fillWidth: true
                Choice { model: [tr("Все источники","All sources")].concat(subscriptions.map(function(s){return s.name})); onActivated: sourceFilter=currentIndex===0?"all":subscriptions[currentIndex-1].id }
                Choice { model: [tr("По названию","By name"),tr("По пингу","By latency")]; currentIndex: serverSort; onActivated: serverSort=currentIndex }
                Item { Layout.fillWidth: true }
                ActionButton { text: "+ "+tr("Добавить подключение","Add connection"); primary: true; onClicked: importDialog.open() }
            }
            Label { textFormat: Text.PlainText; text: tr("Серверов: ","Servers: ")+profiles.length+tr(" · ПКМ — действия с сервером"," · Right-click a server for actions"); color: mutedColor; font.pixelSize: 12 }
            Label { textFormat: Text.PlainText; visible: profiles.length===0&&subscriptions.length===0; text: tr("Добавьте подписку или ключ подключения","Add a subscription or connection link"); color: textColor; Layout.topMargin: 30 }
            Repeater {
                model: serverGroups()
                delegate: Card {
                    required property var modelData
                    id: groupCard
                    property bool collapsed: false
                    Layout.fillWidth: true; implicitHeight: groupContents.implicitHeight+20
                    ColumnLayout { id: groupContents; anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 10; spacing: 0
                        RowLayout { Layout.fillWidth: true; Layout.margins: 7; spacing: 12
                            ToolButton { text: groupCard.collapsed ? "▸" : "▾"; onClicked: groupCard.collapsed=!groupCard.collapsed }
                            ColumnLayout { Layout.fillWidth: true; spacing: 4
                                Label { textFormat: Text.PlainText; text: groupCard.modelData.name+"  ("+groupCard.modelData.items.length+")"; color: textColor; font.bold: true }
                                Label { textFormat: Text.PlainText; visible: !!groupCard.modelData.id; text: (groupCard.modelData.updated_at?new Date(groupCard.modelData.updated_at*1000).toLocaleString():tr("Ещё не обновлялась","Not refreshed yet"))+tr(" · каждые "," · every ")+groupCard.modelData.interval_hours+tr(" ч · при запуске"," h · on startup"); color: mutedColor; font.pixelSize: 10 }
                            }
                            ActionButton { visible: !!groupCard.modelData.id; text: "↻"; implicitWidth: 40; enabled: !busy("update_subscription"); onClicked: call("update_subscription",{id:groupCard.modelData.id}) }
                            ActionButton { visible: !!groupCard.modelData.id; text: "…"; implicitWidth: 40; onClicked: { subscriptionContext.sub=groupCard.modelData;subscriptionContext.popup() } }
                        }
                        Label { textFormat: Text.PlainText; visible: !!groupCard.modelData.usage && !groupCard.collapsed; Layout.margins: 8; text: {var u=groupCard.modelData.usage||{};return tr("Трафик: ","Traffic: ")+bytes((u.upload||0)+(u.download||0))+" / "+(u.total?bytes(u.total):"∞")+(u.expire?" · "+new Date(u.expire*1000).toLocaleDateString():"")} color: mutedColor; font.pixelSize: 11 }
                        Repeater { model: groupCard.collapsed?[]:groupCard.modelData.items
                            delegate: Rectangle {
                                required property var modelData
                                id: serverRow
                                Layout.fillWidth: true; implicitHeight: 44
                                color: prefs.selected_profile===modelData.id ? (light?"#e8e2ff":"#29243e") : rowMouse.containsMouse?(light?"#edf0f8":"#202535"):"transparent"
                                Rectangle { height: 1; width: parent.width; anchors.bottom: parent.bottom; color: borderColor }
                                RowLayout { anchors.fill: parent; anchors.leftMargin: 10; anchors.rightMargin: 12; spacing: 10
                                    Flag { name: serverRow.modelData.name; Layout.preferredWidth: 25; Layout.preferredHeight: 17 }
                                    Label { textFormat: Text.PlainText; text: (pinned(serverRow.modelData.id)?"📌  ":"")+cleanName(serverRow.modelData.name)+(prefs.selected_profile===serverRow.modelData.id?"  ✓":""); Layout.fillWidth: true; color: textColor; elide: Text.ElideRight; font.pixelSize: 12; font.bold: true }
                                    Label { textFormat: Text.PlainText; text: serverRow.modelData.protocol.toUpperCase(); color: mutedColor; font.pixelSize: 10; Layout.preferredWidth: 90 }
                                    Label { textFormat: Text.PlainText; text: serverRow.modelData.host+":"+serverRow.modelData.port; color: mutedColor; font.pixelSize: 10; Layout.preferredWidth: Math.min(230,win.width/6); elide: Text.ElideMiddle }
                                    Label { textFormat: Text.PlainText; text: serverRow.modelData.latency_ms===null?"—":serverRow.modelData.latency_ms+" ms"; color: serverRow.modelData.latency_ms===null?mutedColor:(light?"#167051":"#70dbb1"); Layout.preferredWidth: 65; horizontalAlignment: Text.AlignRight; font.pixelSize: 11 }
                                }
                                MouseArea { id: rowMouse; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.LeftButton|Qt.RightButton; cursorShape: Qt.PointingHandCursor
                                    onClicked: function(mouse){if(mouse.button===Qt.RightButton){serverContext=serverRow.modelData;serverMenu.popup()}else call("select_profile",{id:serverRow.modelData.id})}
                                    onDoubleClicked: function(mouse){if(mouse.button===Qt.LeftButton){call("select_profile",{id:serverRow.modelData.id},function(){if(!snapshot.engine_pid)modeDialog.open()})}}
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: routingPage
        ColumnLayout { spacing: 16
            Card { Layout.fillWidth: true; implicitHeight: routingForm.implicitHeight+40
                ColumnLayout { id: routingForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Режим маршрутизации","Routing mode"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Choice { Layout.fillWidth: true; model: [tr("Весь трафик через VPN","All traffic over VPN"),tr("Только выбранные приложения","Selected applications only"),tr("Кроме выбранных приложений","Except selected applications")]; currentIndex: ["global","whitelist","blacklist"].indexOf(prefs.route_mode||"global"); onActivated: call("quick_network",{route_mode:["global","whitelist","blacklist"][currentIndex]}) }
                    Label { textFormat: Text.PlainText; text: tr("Правила совпадают по имени EXE и применяются в TUN. Список сохраняется в routing-apps.json.","Rules match EXE names and apply in TUN. The list is saved in routing-apps.json."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        ActionButton { text: tr("Импорт правил","Import rules"); onClicked: call("import_routing",{},function(r){if(r)save({route_mode:r.route_mode,applications:r.applications,application_groups:r.application_groups,bypass_domains:r.bypass_domains,routing_rules:r.routing_rules,sniff_excluded_apps:r.sniff_excluded_apps})}) }
                        ActionButton { text: tr("Экспорт правил","Export rules"); onClicked: call("export_routing",{routing:routingDocument()}) }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Приложения","Applications")+" · "+(prefs.applications||[]).length; color: textColor; font.pixelSize: 17; font.bold: true; Layout.fillWidth: true }
                ActionButton { text: showAddedApps?tr("Показать все найденные","Show all discovered"):tr("Показать добавленные","Show added"); onClicked: showAddedApps=!showAddedApps }
                ActionButton { text: tr("Сканировать","Scan"); enabled: !busy("scan_applications"); onClicked: call("scan_applications",{},function(items){applications=items;showAddedApps=false;call("save_application_catalog",{applications:items})}) }
                ActionButton { text: "+ EXE"; onClicked: call("choose_exe",{},function(item){if(item){applications=applications.concat([item]);call("save_application_catalog",{applications:applications});var list=(prefs.applications||[]).slice();if(list.indexOf(exe(item.path))<0)list.push(exe(item.path));save({applications:list})}}) }
            }
            Input { Layout.fillWidth: true; placeholderText: tr("Поиск приложения…","Search applications…"); text: appSearch; onTextChanged: appSearch=text }
            Label { textFormat: Text.PlainText; visible: routingApps().length===0; text: tr("Добавьте EXE или запустите сканирование","Add an EXE or scan installed applications"); color: mutedColor }
            ListView { id: routingList; Layout.fillWidth: true; Layout.preferredHeight: Math.min(520,Math.max(1,count)*58); clip: true; model: routingApps(); spacing: 2; boundsBehavior: Flickable.StopAtBounds; ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle { required property var modelData; width: routingList.width; height: 56; color: panelColor; radius: 8
                    RowLayout { anchors.fill: parent; anchors.margins: 10; spacing: 12
                        CheckBox { checked: (prefs.applications||[]).indexOf(exe(modelData.path))>=0; onToggled: {var name=exe(modelData.path),list=(prefs.applications||[]).filter(function(p){return p!==name});if(checked)list.push(name);save({applications:list})} }
                        AppIcon { path: modelData.path; Layout.preferredWidth: 26; Layout.preferredHeight: 26 }
                        Label { textFormat: Text.PlainText; text: modelData.name; color: textColor; Layout.fillWidth: true; elide: Text.ElideRight }
                        Label { textFormat: Text.PlainText; text: exe(modelData.path); color: mutedColor; font.family: "Consolas"; font.pixelSize: 12; Layout.preferredWidth: 230; elide: Text.ElideMiddle }
                    }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: domainsForm.implicitHeight+40
                ColumnLayout { id: domainsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Исключения доменов из VPN","Domain VPN exceptions"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Label { textFormat: Text.PlainText; text: tr("Один домен или суффикс в строке: .ru, .su, example.com. Эти адреса идут напрямую.","One domain or suffix per line: .ru, .su, example.com. These addresses use the direct route."); color: mutedColor; Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12 }
                    ScrollView { Layout.fillWidth: true; Layout.preferredHeight: 105
                        TextArea { id: bypassInput; objectName: "domainExceptions"; placeholderText: "example.com\n*.example.org\n.ru"; text: (prefs.bypass_domains||[]).join("\n"); color: textColor; selectByMouse: true; wrapMode: TextEdit.Wrap; background: Rectangle { color: palette.base; border.color: borderColor; radius: 8 } }
                    }
                    ActionButton { text: tr("Сохранить исключения","Save exceptions"); onClicked: save({bypass_domains:bypassInput.text.split(/[\n,;]+/).map(function(v){return v.trim()}).filter(function(v){return v.length})}) }
                    Label { textFormat: Text.PlainText; text: tr("Дополнительный маршрут домена","Custom domain route"); color: textColor; font.bold: true }
                    RowLayout { Layout.fillWidth: true
                        Input { id: routeDomain; placeholderText: ".ru / example.com"; Layout.fillWidth: true }
                        Choice { id: routeAction; model: ["direct","proxy","reject"] }
                        ActionButton { text: "+"; onClicked: {var r=(prefs.routing_rules||[]).slice();r.push({enabled:true,kind:"domain",value:routeDomain.text.trim(),action:routeAction.currentText});save({routing_rules:r});routeDomain.clear()} }
                    }
                    Repeater { model: prefs.routing_rules||[]
                        delegate: RowLayout { required property var modelData; required property int index; Layout.fillWidth: true
                            Label { textFormat: Text.PlainText; text: modelData.value+" → "+modelData.action; color: textColor; Layout.fillWidth: true }
                            ActionButton { text: "×"; onClicked: {var r=(prefs.routing_rules||[]).slice();r.splice(index,1);save({routing_rules:r})} }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: networkPage
        ColumnLayout {
            id: networkView
            property int tab: 0
            spacing: 18
            RowLayout { Layout.fillWidth: true; spacing: 5
                Repeater { model: [tr("Подключение","Connection"),"DNS","MTU","SNI / TLS",tr("Транспорт","Transport"),tr("Служба TUN","TUN service"),tr("Ядра","Engines")]
                    delegate: ActionButton { required property int index; required property string modelData; Layout.fillWidth: true; Layout.preferredWidth: 1; implicitWidth: 70; leftPadding: 6; rightPadding: 6; font.pixelSize: 11; text: modelData; primary: networkView.tab===index; onClicked: networkView.tab=index }
                }
            }
            Loader { active: networkView.tab===6; visible: active; Layout.fillWidth: true; sourceComponent: enginesPage }
            ColumnLayout { visible: networkView.tab!==6; Layout.fillWidth: true; spacing: 18
                Card { visible: networkView.tab===0; Layout.fillWidth: true; implicitHeight: networkForm.implicitHeight+40
                    ColumnLayout { id: networkForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Label { textFormat: Text.PlainText; text: tr("Подключение","Connection"); color: textColor; font.pixelSize: 16; font.bold: true }
                        RowLayout { Layout.fillWidth: true
                            Label { textFormat: Text.PlainText; text: tr("Локальный порт прокси","Local proxy port"); color: mutedColor; Layout.fillWidth: true }
                            SpinBox { id: proxyPort; from: 1024; to: 65535; value: prefs.proxy_port||2080; editable: true }
                        }
                        HelpText { text: tr("Порт локального прокси на этом компьютере. Меняйте его, если стандартный порт занят; это не порт VPN-сервера.","The local proxy port on this computer. Change it if the default port is busy; it is not the VPN server port.") }
                        SettingRow { Layout.fillWidth: true; title: "Fake IP"; description: tr("Выдаёт временные адреса для сопоставления доменов в TUN. После изменения переоткройте соединения приложений.","Uses temporary addresses to match domains in TUN. Reopen application connections after changing this setting."); checked: !!prefs.fake_ip; onToggled: function(v){save({fake_ip:v})} }
                        Input { id: probeUrl; Layout.fillWidth: true; text: prefs.probe_url||"https://www.gstatic.com/generate_204"; placeholderText: "HTTPS probe URL" }
                        HelpText { text: tr("HTTPS-адрес для проверки доступности VPN и пинга. Выбирайте небольшой ресурс без авторизации; недоступный сайт может давать ошибку при рабочем VPN.","The HTTPS address used for VPN health and latency checks. Use a small resource without authentication; a blocked site can fail even when VPN works.") }
                        ActionButton { text: tr("Сохранить подключение","Save connection"); primary: true; onClicked: save({proxy_port:proxyPort.value,probe_url:probeUrl.text}) }
                    }
                }
                Card { visible: networkView.tab===1; Layout.fillWidth: true; implicitHeight: dnsForm.implicitHeight+40
                    ColumnLayout { id: dnsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Component.onCompleted: { dnsMode.currentIndex=Math.max(0,dnsMode.values.indexOf(prefs.dns?prefs.dns.mode:"system"));dnsServer.text=prefs.dns&&prefs.dns.server?prefs.dns.server:"1.1.1.1";dnsPath.text=prefs.dns&&prefs.dns.path?prefs.dns.path:"/dns-query";dnsRoute.currentIndex=prefs.dns_route==="vpn"?1:0 }
                        Label { textFormat: Text.PlainText; text: "DNS"; color: textColor; font.pixelSize: 16; font.bold: true }
                        HelpText { text: tr("DNS переводит имена сайтов в IP-адреса. Системный использует настройки Windows; UDP отправляет обычные запросы, TLS и HTTPS шифруют запросы до выбранного резолвера.","DNS translates site names to IP addresses. System uses Windows settings; UDP sends standard queries, while TLS and HTTPS encrypt queries to the resolver.") }
                        Choice { id: dnsMode; objectName: "dnsMode"; Layout.fillWidth: true; values: ["system","udp","tls","https"]; model: [tr("Системный DNS","System DNS"),"UDP","DNS over TLS","DNS over HTTPS"]; onActivated: {if(!dnsServer.text)dnsServer.text="1.1.1.1";if(currentText==="system")dnsRoute.currentIndex=0} }
                        Input { id: dnsServer; objectName: "dnsServer"; visible: dnsMode.selectedValue!=="system"; Layout.fillWidth: true; placeholderText: "1.1.1.1" }
                        Input { id: dnsPath; visible: dnsMode.selectedValue==="https"; Layout.fillWidth: true; placeholderText: "/dns-query" }
                        HelpText { text: tr("Адрес резолвера можно изменить. Для HTTPS путь обычно /dns-query. Маршрут DNS определяет путь запросов к резолверу и не меняет маршруты самих приложений.","You can change the resolver address. HTTPS usually uses /dns-query. The DNS route controls queries to the resolver, not application routing.") }
                        Choice { id: dnsRoute; Layout.fillWidth: true; model: [tr("DNS напрямую","DNS over direct route"),tr("DNS через VPN","DNS over VPN")]; enabled: dnsMode.selectedValue!=="system" }
                        ActionButton { objectName: "saveDns"; text: tr("Сохранить DNS","Save DNS"); primary: true; enabled: !savingPreferences; onClicked: {var d={mode:dnsMode.selectedValue};if(d.mode!=="system")d.server=dnsServer.text.trim()||"1.1.1.1";if(d.mode==="https")d.path=dnsPath.text||"/dns-query";save({dns:d,dns_route:d.mode==="system"?"direct":dnsRoute.currentIndex?"vpn":"direct"})} }
                    }
                }
                Card { visible: networkView.tab===2; Layout.fillWidth: true; implicitHeight: mtuForm.implicitHeight+40
                    ColumnLayout { id: mtuForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Label { textFormat: Text.PlainText; text: tr("MTU и пресеты","MTU and presets"); color: textColor; font.pixelSize: 16; font.bold: true }
                        HelpText { text: tr("MTU — максимальный размер IP-пакета виртуального адаптера. Слишком большой может вызывать фрагментацию или зависания, слишком маленький увеличивает служебные расходы. Большее число не всегда даёт большую скорость.","MTU is the largest IP packet on the virtual adapter. Too large can cause fragmentation or stalls; too small adds overhead. A larger number does not always improve speed.") }
                        RowLayout { Layout.fillWidth: true
                            Choice { id: mtuPreset; Layout.fillWidth: true; model: [tr("Сбалансированный · 1400","Balanced · 1400"),tr("Совместимость · 1280","Compatibility · 1280"),tr("Игровой · 1420","Gaming · 1420"),tr("Вручную","Custom")]; currentIndex: Math.max(0,["balanced","compatibility","games","custom"].indexOf(prefs.network_preset)); onActivated: {if(currentIndex<3)mtuValue.value=[1400,1280,1420][currentIndex]} }
                            SpinBox { id: mtuValue; from: 1280; to: 9000; value: prefs.mtu||1400; editable: true }
                            ActionButton { text: tr("Применить","Apply"); onClicked: save({mtu:mtuValue.value,network_preset:["balanced","compatibility","games","custom"][mtuPreset.currentIndex]}) }
                        }
                        Label { textFormat: Text.PlainText; text: tr("Автоподбор выбирает IPv4 текущего сервера; без доступного адреса использует 1.1.1.1. Проверяет пакеты с запретом фрагментации и отдельно — доступность VPN.","Auto selection uses the current server IPv4, falling back to 1.1.1.1. It tests packets with fragmentation disabled and checks VPN health separately."); color: mutedColor; Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12 }
                        ActionButton { text: tr("Автоподбор MTU","Find MTU"); enabled: !jobTimer.running; onClicked: beginJob("start_mtu",{target:""}) }
                        HelpText { text: tr("Пресеты — начальные значения: 1400 для баланса, 1280 для совместимости, 1420 для игрового трафика. Применяйте результат вручную и сравнивайте стабильность, пинг и Speed Test на одном сервере.","Presets are starting points: 1400 balanced, 1280 compatibility and 1420 gaming. Apply a result manually and compare stability, latency and Speed Test on the same server.") }
                        BusyIndicator { visible: !!job&&job.kind==="mtu"&&job.running; running: visible; implicitHeight: 28 }
                        Repeater { model: job&&job.kind==="mtu"?job.results:[]
                            delegate: ColumnLayout { required property var modelData; Layout.fillWidth: true
                                Label { textFormat: Text.PlainText; text: resultText(modelData); color: textColor; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                HelpText { text: tr("Измеренный MTU пути: ","Measured path MTU: ")+(modelData.path&&modelData.path.confirmed_mtu?modelData.path.confirmed_mtu:tr("не подтверждён","unconfirmed"))+tr(" · полезная нагрузка ICMP: "," · ICMP payload: ")+(modelData.max_icmp_payload||"—")+tr(" байт. IPv4 + ICMP добавляют 28 байт."," bytes. IPv4 + ICMP add 28 bytes.") }
                                HelpText { text: tr("Текущий MTU TUN: ","Current TUN MTU: ")+(modelData.current_mtu||prefs.mtu)+tr(" · рекомендуемый стартовый: "," · suggested starting value: ")+(modelData.suggested_mtu||"—")+tr(". Запас 80 байт — консервативная оценка для VPN, а не точное измерение его скорости.",". The 80-byte reserve is a conservative VPN estimate, not a measurement of VPN speed.") }
                                Label { textFormat: Text.PlainText; text: modelData.path?modelData.path.note||"":""; color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
                                ActionButton { text: tr("Применить рекомендацию","Apply recommendation"); enabled: !!modelData.suggested_mtu; onClicked: {mtuValue.value=modelData.suggested_mtu;mtuPreset.currentIndex=3;save({mtu:mtuValue.value,network_preset:"custom"})} }
                            }
                        }
                        Label { textFormat: Text.PlainText; visible: !!job&&job.kind==="mtu"&&!!job.error; text: job&&job.error?job.error.message:""; color: light?"#a32c43":"#ff9eae"; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                        Label { textFormat: Text.PlainText; text: tr("Подбор MTU по скорости и задержке","MTU comparison by speed and latency"); color: textColor; font.pixelSize: 15; font.bold: true; Layout.topMargin: 12 }
                        HelpText { text: tr("Ручной тест временно перебирает MTU на текущем TUN-соединении: загрузка, HTTPS-пинг, задержка под нагрузкой и джиттер. Несколько переподключений прервут соединения приложений; загрузочные данные — до 36 MiB плюс служебный трафик. После проверки возвращается исходный MTU. Это тест HTTPS, не измерение игрового UDP.","This manual test temporarily changes MTU on the current TUN connection and measures download, HTTPS latency, loaded latency and jitter. Reconnects interrupt application connections; download payload is limited to 36 MiB plus protocol overhead. The original MTU is restored afterward. It measures HTTPS, not gaming UDP.") }
                        Input { id: tuningUrl; Layout.fillWidth: true; text: "https://speed.cloudflare.com/__down?bytes=4194304"; placeholderText: "HTTPS download URL" }
                        RowLayout { Layout.fillWidth: true
                            Choice { id: tuningGoal; values: ["balanced","latency"]; model: [tr("Баланс скорости и задержки","Balance speed and latency"),tr("Приоритет задержки","Prioritize latency")]; Layout.fillWidth: true }
                            ActionButton { text: tr("Начать сравнение MTU","Start MTU comparison"); primary: true; enabled: !!snapshot.tun_active&&!jobTimer.running; onClicked: beginJob("start_mtu_tuning",{url:tuningUrl.text,goal:tuningGoal.selectedValue}) }
                            ActionButton { text: tr("Отменить","Cancel"); visible: job&&job.kind==="mtu_tuning"&&job.running; onClicked: call("cancel_diagnostic",{id:diagnosticId}) }
                        }
                        HelpText { visible: !snapshot.tun_active; text: tr("Для сравнения подключитесь к своему серверу в режиме TUN.","Connect to your server in TUN mode to compare candidates.") }
                        BusyIndicator { visible: job&&job.kind==="mtu_tuning"&&job.running; running: visible; implicitHeight: 28 }
                        Label { visible: job&&job.kind==="mtu_tuning"; text: job?job.completed+" / "+job.total:""; color: mutedColor }
                        Repeater { model: job&&job.kind==="mtu_tuning"?job.results:[]
                            delegate: ColumnLayout { required property var modelData; Layout.fillWidth: true
                                Label { textFormat: Text.PlainText; visible: modelData.mtu!==undefined; text: "MTU "+modelData.mtu+"  · ↓ "+(modelData.download_mbps===null?"—":Number(modelData.download_mbps).toFixed(2))+" Mbps  · "+tr("пинг ","latency ")+(modelData.latency_ms===null?"—":Number(modelData.latency_ms).toFixed(1))+" ms · "+tr("джиттер ","jitter ")+(modelData.jitter_ms===null?"—":Number(modelData.jitter_ms).toFixed(1))+" ms"; color: textColor; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                                HelpText { visible: modelData.mtu!==undefined; text: (modelData.phase==="control"?tr("Контроль исходного MTU","Original MTU control"):modelData.phase==="verify"?tr("Повторная проверка","Repeated verification"):tr("Проверка кандидата","Candidate trial"))+" · "+tr("задержка под нагрузкой: ","loaded latency: ")+(modelData.loaded_ms===null||modelData.loaded_ms===undefined?"—":Number(modelData.loaded_ms).toFixed(1)+" ms") }
                                HelpText { visible: !!modelData.error; text: modelData.error||"" }
                                HelpText { visible: !!modelData.comparison; text: modelData.comparison?(modelData.comparison.suggested_mtu?tr("Лучший повторяемый результат: MTU ","Best repeated result: MTU ")+modelData.comparison.suggested_mtu:tr("Убедительного улучшения нет — оставьте текущий MTU.","No reliable improvement — keep the current MTU.")):"" }
                                HelpText { visible: !!modelData.comparison; text: modelData.comparison?tuningReason(modelData.comparison.reason):"" }
                                ActionButton { visible: !!modelData.comparison&&!!modelData.comparison.suggested_mtu; text: tr("Применить выбранный MTU","Apply chosen MTU"); enabled: !jobTimer.running; onClicked: {mtuValue.value=modelData.comparison.suggested_mtu;save({mtu:mtuValue.value})} }
                            }
                        }
                        HelpText { visible: job&&job.kind==="mtu_tuning"&&!!job.error; text: job&&job.error?job.error.message:"" }

                    }
                }
                Card { visible: networkView.tab===3; Layout.fillWidth: true; implicitHeight: tlsForm.implicitHeight+40
                    ColumnLayout { id: tlsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Component.onCompleted: {var t=prefs.tls_defaults||{};sniMode.currentIndex=Math.max(0,sniMode.values.indexOf(t.sni_mode||"inherit"));sniName.text=t.server_name||"";httpHost.text=t.http_host||"";fingerprint.currentIndex=Math.max(0,fingerprint.values.indexOf(t.fingerprint||"profile"))}
                        Label { textFormat: Text.PlainText; text: "SNI / TLS"; color: textColor; font.pixelSize: 16; font.bold: true }
                        Label { textFormat: Text.PlainText; text: tr("TLS сохраняет проверку сертификата. Reality и транспорт проверяются перед применением.","TLS certificate validation remains enabled. Reality and transport compatibility are checked before applying."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                        Label { textFormat: Text.PlainText; text: tr("SNI — режим передачи имени сервера","SNI — server name mode"); color: textColor; font.bold: true }
                        Choice { id: sniMode; Layout.fillWidth: true; values: ["inherit","auto","custom","disabled"]; model: [tr("Как в профиле","Use profile settings"),tr("Автоматически по адресу сервера","Use server address"),tr("Своё имя SNI","Custom SNI name"),tr("Не передавать SNI","Do not send SNI")] }
                        HelpText { text: tr("SNI сообщает серверу имя сайта при установке TLS. Обычно оставляйте значение из профиля. Своё имя должно подходить сертификату сервера; Reality требует SNI.","SNI sends a hostname when TLS starts. Usually keep the profile value. A custom name must match the server certificate; Reality requires SNI.") }
                        Label { textFormat: Text.PlainText; visible: sniMode.selectedValue==="custom"||sniMode.selectedValue==="disabled"; text: tr("Имя сервера для SNI / проверки сертификата","Server name for SNI / certificate validation"); color: mutedColor; font.pixelSize: 12 }
                        Input { id: sniName; visible: sniMode.selectedValue==="custom"||sniMode.selectedValue==="disabled"; Layout.fillWidth: true; placeholderText: "Server name / SNI" }
                        Label { textFormat: Text.PlainText; text: tr("HTTP Host — имя узла HTTP (необязательно)","HTTP Host — HTTP hostname (optional)"); color: textColor; font.bold: true }
                        Input { id: httpHost; Layout.fillWidth: true; placeholderText: tr("HTTP Host (необязательно)","HTTP Host (optional)") }
                        HelpText { text: tr("Отдельный заголовок для WebSocket, XHTTP и HTTPUpgrade. Это не SNI. Оставьте пустым, чтобы использовать профиль; неверное имя может нарушить подключение.","A separate header for WebSocket, XHTTP and HTTPUpgrade. It is distinct from SNI. Leave empty to use the profile; an incorrect hostname can break the connection.") }
                        Label { textFormat: Text.PlainText; text: tr("TLS — отпечаток ClientHello (uTLS)","TLS — ClientHello fingerprint (uTLS)"); color: textColor; font.bold: true }
                        Choice { id: fingerprint; Layout.fillWidth: true; values: ["profile","disabled","chrome","firefox","edge","safari","ios","android","randomized"]; model: [tr("Как в профиле","Use profile settings"),tr("Без uTLS","Disable uTLS"),"Chrome","Firefox","Edge","Safari","iOS","Android",tr("Случайный","Randomized")] }
                        HelpText { text: tr("Отпечаток меняет вид TLS-рукопожатия под выбранный браузер. Проверка сертификата остаётся включённой. QUIC/Hysteria2 не используют этот параметр; Reality требует совместимый uTLS.","The fingerprint shapes the TLS handshake like the selected browser. Certificate verification stays enabled. This setting does not apply to QUIC/Hysteria2; Reality requires compatible uTLS.") }
                        ActionButton { text: tr("Сохранить SNI и TLS","Save SNI and TLS"); onClicked: save({tls_defaults:sniMode.selectedValue==="inherit"&&!httpHost.text&&fingerprint.currentIndex===0?null:{sni_mode:sniMode.selectedValue,server_name:sniName.text.trim(),http_host:httpHost.text.trim()||null,fingerprint:fingerprint.currentIndex?fingerprint.selectedValue:null}}) }
                    }
                }
                Card { visible: networkView.tab===4; Layout.fillWidth: true; implicitHeight: transportForm.implicitHeight+40
                    ColumnLayout { id: transportForm; anchors.fill: parent; anchors.margins: 20; spacing: 12
                        Component.onCompleted: {var f=prefs.fragmentation||{},m=prefs.multiplex||{};fragToggle.checked=!!f.enabled;fragPackets.text=f.packets||"tlshello";fragLength.text=f.length||"50-100";fragInterval.text=f.interval||"10-20";fragSplit.text=f.max_split||"100-200";muxToggle.checked=!!m.enabled;muxConcurrency.value=m.concurrency===undefined?8:m.concurrency;muxXudp.value=m.xudp_concurrency===undefined?8:m.xudp_concurrency;muxUdp.currentIndex=Math.max(0,muxUdp.model.indexOf(m.udp_443||"skip"))}
                        Label { textFormat: Text.PlainText; text: tr("Фрагментация и мультиплексор","Fragmentation and multiplexing"); color: textColor; font.pixelSize: 16; font.bold: true }
                        Label { textFormat: Text.PlainText; text: tr("Для поддерживаемых транспортов Xray. Несовместимые сочетания не применяются.","For supported Xray transports. Incompatible combinations are rejected."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                        SettingRow { id: fragToggle; Layout.fillWidth: true; title: tr("Фрагментация","Fragmentation"); description: tr("Разбивает первые данные соединения на части. Может помочь при некоторых ограничениях сети, но задержки между частями могут снизить скорость.","Splits initial connection data into smaller pieces. It may help with some network restrictions, but delays between pieces can reduce speed.") }
                        RowLayout { Layout.fillWidth: true
                            Input { id: fragPackets; Layout.fillWidth: true; placeholderText: "packets: tlshello"; ToolTip.text: "packets" }
                            Input { id: fragLength; Layout.fillWidth: true; placeholderText: "length: 50-100" }
                            Input { id: fragInterval; Layout.fillWidth: true; placeholderText: "interval: 10-20" }
                            Input { id: fragSplit; Layout.fillWidth: true; placeholderText: "max_split: 100-200" }
                        }
                        HelpText { text: tr("Слева направо: packets — TLS Hello или первые 1–3 пакета; length — размер частей в байтах; interval — пауза в мс; maxSplit — максимальное число частей Xray. Диапазон задаётся как минимум-максимум.","Left to right: packets selects TLS Hello or the first 1–3 packets; length is fragment size in bytes; interval is delay in ms; maxSplit limits the number of fragments. Ranges use minimum-maximum.") }
                        SettingRow { id: muxToggle; Layout.fillWidth: true; title: "Multiplex"; description: tr("Передаёт несколько соединений в одном канале. Поддерживается для VLESS/Trojan TCP и WebSocket без Vision; иногда ухудшает задержку при потерях.","Carries multiple connections over one channel. Supported for VLESS/Trojan TCP and WebSocket without Vision; loss can increase latency.") }
                        RowLayout { Layout.fillWidth: true
                            Label { textFormat: Text.PlainText; text: "Concurrency"; color: mutedColor }
                            SpinBox { id: muxConcurrency; from: -1; to: 128; editable: true }
                            Label { textFormat: Text.PlainText; text: "XUDP"; color: mutedColor }
                            SpinBox { id: muxXudp; from: -1; to: 128; editable: true }
                            Label { textFormat: Text.PlainText; text: "UDP 443"; color: mutedColor }
                            Choice { id: muxUdp; model: ["skip","reject","allow"]; Layout.fillWidth: true }
                        }
                        HelpText { text: tr("Concurrency — число TCP-потоков в канале; XUDP — UDP-потоков. UDP 443: skip — без мультиплексирования, reject — блокировать, allow — разрешить. Изменяйте эти параметры под возможности сервера.","Concurrency is TCP streams per channel; XUDP controls UDP streams. UDP 443: skip bypasses multiplexing, reject blocks it, allow permits it. Match these values to server capabilities.") }
                        ActionButton { text: tr("Сохранить транспорт","Save transport"); onClicked: save({fragmentation:{enabled:fragToggle.checked,packets:fragPackets.text,length:fragLength.text,interval:fragInterval.text,max_split:fragSplit.text},multiplex:{enabled:muxToggle.checked,concurrency:muxConcurrency.value,xudp_concurrency:muxXudp.value,udp_443:muxUdp.currentText}}) }
                    }
                }
                Card { visible: networkView.tab===5; Layout.fillWidth: true; implicitHeight: serviceForm.implicitHeight+40
                    ColumnLayout { id: serviceForm; anchors.fill: parent; anchors.margins: 20; spacing: 12
                        Label { textFormat: Text.PlainText; text: tr("Служба TUN","TUN service"); color: textColor; font.pixelSize: 16; font.bold: true }
                        Label { textFormat: Text.PlainText; text: modelData.service?(modelData.service.installed?tr("Установлена","Installed"):tr("Не установлена","Not installed"))+(modelData.service.running?tr(" · работает"," · running"):""):"—"; color: mutedColor }
                        Label { textFormat: Text.PlainText; text: tr("Установка службы требует подтверждения в стандартном окне Windows.","Service installation requires confirmation in the standard Windows dialog."); color: mutedColor; font.pixelSize: 12 }
                        RowLayout {
                            ActionButton { text: tr("Установить / восстановить","Install / repair"); enabled: !snapshot.engine_pid&&!busy("manage_service"); onClicked: call("manage_service",{install:true,upgrade:!!(modelData.service&&modelData.service.installed)}) }
                            ActionButton { text: tr("Удалить службу","Remove service"); enabled: !snapshot.engine_pid&&!busy("manage_service")&&!!(modelData.service&&modelData.service.installed); onClicked: call("manage_service",{install:false}) }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: diagnosticsPage
        ColumnLayout { spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: latencyForm.implicitHeight+40
                ColumnLayout { id: latencyForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Пинг","Ping"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Choice { id: latencyMethod; Layout.fillWidth: true; model: ["via Proxy GET","via Proxy HEAD","TCP","ICMP"]; currentIndex: 0 }
                    RowLayout { Layout.fillWidth: true
                        Choice { id: proxyPingMode; model: ["keepalive","double"]; currentIndex: prefs.ping_proxy_mode==="double"?1:0; onActivated: save({ping_proxy_mode:currentText}) }
                        Input { id: pingProbeUrl; Layout.fillWidth: true; text: prefs.probe_url||"https://www.gstatic.com/generate_204"; onEditingFinished: {if(text!==prefs.probe_url)save({probe_url:text})} }
                    }
                    Label { textFormat: Text.PlainText; text: tr("GET/HEAD проверяют HTTPS через профиль. Keepalive повторно использует соединение. TCP измеряет путь до сервера вне TUN; ICMP зависит от ответа узла.","GET/HEAD test HTTPS through the profile. Keepalive reuses the connection. TCP tests the server outside TUN; ICMP depends on the host replying."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        ActionButton { text: tr("Проверить выбранный","Test selected"); enabled: !!selected()&&!jobTimer.running&&!savingPreferences; primary: true; onClicked: beginJob("start_latency",{ids:[selected().id],method:["proxy_https_rtt","proxy_https_head_rtt","tcp_connect_rtt","icmp_rtt"][latencyMethod.currentIndex]}) }
                        ActionButton { text: tr("Проверить все","Test all"); enabled: profiles.length>0&&!jobTimer.running&&!savingPreferences; onClicked: beginJob("start_latency",{ids:profiles.map(function(p){return p.id}),method:["proxy_https_rtt","proxy_https_head_rtt","tcp_connect_rtt","icmp_rtt"][latencyMethod.currentIndex]}) }
                        ActionButton { text: tr("Лучший сервер","Best server"); enabled: profiles.length>0&&!jobTimer.running; onClicked: beginJob("start_best_server",{samples:prefs.selection_samples||5}) }
                    }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: speedForm.implicitHeight+40
                ColumnLayout { id: speedForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: "Speed Test"; color: textColor; font.pixelSize: 16; font.bold: true }
                    Label { textFormat: Text.PlainText; text: tr("Тест расходует трафик. Отправка данных включается отдельно.","The test uses traffic. Upload is enabled separately."); color: mutedColor; font.pixelSize: 12 }
                    Input { id: downloadUrl; Layout.fillWidth: true; text: "https://speed.cloudflare.com/__down?bytes=10000000"; placeholderText: "Download HTTPS URL" }
                    Input { id: uploadUrl; Layout.fillWidth: true; text: "https://speed.cloudflare.com/__up"; placeholderText: "Upload HTTPS URL" }
                    RowLayout { Layout.fillWidth: true
                        ActionButton { text: "Cloudflare Speed Test ↗"; onClicked: Qt.openUrlExternally("https://speed.cloudflare.com/") }
                        Choice { id: speedMode; model: ["vpn","direct","compare"]; Layout.fillWidth: true }
                        SpinBox { id: speedBudget; from: 1; to: 100; value: 10; editable: true }
                        Label { textFormat: Text.PlainText; text: "MiB"; color: mutedColor }
                        CheckBox { id: uploadAllowed; text: tr("Отправка","Upload") }
                    }
                    ActionButton { text: tr("Начать измерение","Start test"); primary: true; enabled: downloadUrl.text.length>0&&!jobTimer.running; onClicked: beginJob("start_speed",{request:{download_url:downloadUrl.text,upload_url:uploadAllowed.checked?uploadUrl.text:null,upload_endpoint_confirmed:uploadAllowed.checked,mode:speedMode.currentText,duration_seconds:10,max_payload_mib:speedBudget.value}}) }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: resultColumn.implicitHeight+40
                ColumnLayout { id: resultColumn; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Результаты измерений","Measurement results"); color: textColor; font.pixelSize: 16; font.bold: true; Layout.fillWidth: true }
                        ActionButton { text: tr("История","History"); onClicked: {historyDialog.open();historyDialog.load()} }
                        ActionButton { visible: jobTimer.running; text: tr("Отменить","Cancel"); onClicked: call("cancel_diagnostic",{id:diagnosticId}) }
                    }
                    BusyIndicator { visible: jobTimer.running; running: visible; implicitHeight: 28 }
                    Label { textFormat: Text.PlainText; text: !job?tr("Запустите проверку","Start a test"):(job.running?tr("Выполняется","Running"):job.cancelled?tr("Отменено","Cancelled"):tr("Завершено","Completed"))+" · "+job.completed+" / "+job.total; color: mutedColor }
                    Label { textFormat: Text.PlainText; visible: !!job&&!!job.error; text: job&&job.error?job.error.message:""; color: light?"#a32c43":"#ff9eae"; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    Repeater { model: job?job.results:[]
                        delegate: ColumnLayout { required property var modelData; readonly property var value: modelData.Ok||modelData; Layout.fillWidth: true; spacing: 6
                            Label { textFormat: Text.PlainText; text: {var p=profiles.find(function(p){return p.id===value.profile_id});return p?cleanName(p.name):value.mode||job.kind} color: textColor; font.bold: true }
                            Label { textFormat: Text.PlainText; text: resultText(value); color: textColor; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                            Label { textFormat: Text.PlainText; visible: !!value.error||!!modelData.Err; text: value.error?value.error.message:modelData.Err?modelData.Err.message:""; color: light?"#a32c43":"#ff9eae"; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                            Label { textFormat: Text.PlainText; visible: !!value.note; text: value.note||""; color: mutedColor; font.pixelSize: 11; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                        }
                    }
                    ActionButton { visible: !!job; text: tr("Подробности / копировать","Details / copy"); onClicked: editJson(tr("Измерение","Measurement"),JSON.stringify(job,null,2),"") }
                }
            }
        }
    }

    Component {
        id: playgroundPage
        ColumnLayout {
            id: processView
            property bool autoRefresh: true
            property var expanded: ({})
            property string search: ""
            spacing: 14
            function refreshProcesses() {
                if(busy("playground_processes"))return
                call("playground_processes",{},function(items){ processes=items||[] })
            }

            function rows() {
                var map={},result=[]
                processes.forEach(function(p){var key=exe(p.path);if((key+" "+p.pid).toLowerCase().indexOf(search.toLowerCase())<0)return;if(!map[key])map[key]=[];map[key].push(p)})
                Object.keys(map).sort().forEach(function(key){
                    var items=map[key],first=items[0],sockets=0,core=0,up=null,down=null,routes=[]
                    items.forEach(function(p){sockets+=p.sockets||0;core+=p.engine_connections||0;if(p.upload!==null)up=(up||0)+p.upload;if(p.download!==null)down=(down||0)+p.download;if(p.route&&routes.indexOf(p.route)<0)routes.push(p.route)})
                    var group=Object.assign({},first,{pid:items.length>1?null:first.pid,name:first.name+(items.length>1?" ("+items.length+")":"")})
                    result.push({key:key,name:group.name,pid:group.pid,path:first.path,sockets:sockets,engine_connections:core,upload:up,download:down,route:routes.join(" / "),child:false,count:items.length,process:group})
                    if(expanded[key]&&items.length>1)items.forEach(function(p){result.push(Object.assign({},p,{key:key,child:true,count:1,process:p}))})
                });return result
            }
            Component.onCompleted: { refreshProcesses();observe() }
            Timer { interval: 5000; running: processView.autoRefresh; repeat: true; onTriggered: {processView.refreshProcesses();if(!busy("connection_analysis"))observe()} }
            RowLayout { Layout.fillWidth: true
                Input { Layout.fillWidth: true; placeholderText: tr("Поиск процесса или PID…","Search process or PID…"); onTextChanged: processView.search=text }
                Label { textFormat: Text.PlainText; text: tr("Автообновление","Auto refresh"); color: mutedColor }
                Toggle { checked: processView.autoRefresh; onToggled: processView.autoRefresh=checked }
                ActionButton { text: tr("Обновить","Refresh"); onClicked: {processView.refreshProcesses();observe()} }
            }
            Rectangle { Layout.fillWidth: true; implicitHeight: 36; color: panelColor
                RowLayout { anchors.fill: parent; anchors.margins: 10
                    Label { textFormat: Text.PlainText; text: tr("Процесс","Process"); color: textColor; font.bold: true; Layout.fillWidth: true }
                    Label { textFormat: Text.PlainText; text: "EXE"; color: mutedColor; Layout.preferredWidth: 160 }
                    Label { textFormat: Text.PlainText; text: "PID"; color: mutedColor; Layout.preferredWidth: 64 }
                    Label { textFormat: Text.PlainText; text: tr("Сокеты / ядро","Sockets / core"); color: mutedColor; Layout.preferredWidth: 100; horizontalAlignment: Text.AlignRight }
                    Label { textFormat: Text.PlainText; text: tr("Трафик ядра ↓ / ↑","Core traffic ↓ / ↑"); color: mutedColor; Layout.preferredWidth: 155; horizontalAlignment: Text.AlignRight }
                }
            }
            ListView { id: processList; Layout.fillWidth: true; Layout.preferredHeight: 390; clip: true; model: processView.rows(); boundsBehavior: Flickable.StopAtBounds; ScrollBar.vertical: ScrollBar {}
                delegate: Rectangle { required property var modelData; width: processList.width; height: 36; color: observedProcess&&processKey(observedProcess)===processKey(modelData.process)?(light?"#e4ddff":"#30284d"):(index%2?panelColor:"transparent")
                    required property int index
                    RowLayout { anchors.fill: parent; anchors.leftMargin: modelData.child?46:10; anchors.rightMargin: 10; spacing: 8
                        ToolButton { visible: !modelData.child; text: modelData.count>1?(processView.expanded[modelData.key]?"▾":"▸"):""; implicitWidth: 20; enabled: modelData.count>1; onClicked: {var e=Object.assign({},processView.expanded);e[modelData.key]=!e[modelData.key];processView.expanded=e} }
                        AppIcon { path: modelData.path; Layout.preferredWidth: 22; Layout.preferredHeight: 22 }
                        Label { textFormat: Text.PlainText; text: modelData.name; color: textColor; Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 11 }
                        Label { textFormat: Text.PlainText; text: exe(modelData.path); color: mutedColor; Layout.preferredWidth: 160; elide: Text.ElideMiddle; font.pixelSize: 11 }
                        Label { textFormat: Text.PlainText; text: modelData.pid||"—"; color: mutedColor; Layout.preferredWidth: 64; font.pixelSize: 11 }
                        Label { textFormat: Text.PlainText; text: (modelData.sockets===null?"—":modelData.sockets)+" / "+modelData.engine_connections; color: textColor; Layout.preferredWidth: 100; horizontalAlignment: Text.AlignRight; font.pixelSize: 11 }
                        Label { textFormat: Text.PlainText; text: bytes(modelData.download)+" / "+bytes(modelData.upload); color: textColor; Layout.preferredWidth: 155; horizontalAlignment: Text.AlignRight; font.pixelSize: 11 }
                    }
                    MouseArea { anchors.fill: parent; anchors.leftMargin: 40; onClicked: {observedProcess=modelData.process;observe()} }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: observedProcess?observedProcess.name+(observedProcess.pid?" · PID "+observedProcess.pid:""):tr("Все соединения ядра","All engine connections"); color: textColor; Layout.fillWidth: true; font.bold: true }
                ActionButton { text: tr("Все соединения","All connections"); onClicked: {observedProcess=null;observe()} }
            }
            RowLayout { visible: !!observedProcess; Layout.fillWidth: true
                Choice { id: appRoute; currentIndex: {var name=observedProcess?exe(observedProcess.path):"",rule=(prefs.routing_rules||[]).find(function(r){return r.kind==="process"&&exe(r.value)===name});return rule?Math.max(0,values.indexOf(rule.action)):0} values: ["auto","proxy","direct","reject"]; model: [tr("По общим правилам","Use routing rules"),tr("Через VPN","Through VPN"),tr("Напрямую","Direct"),tr("Блокировать","Block")] }
                ActionButton { text: tr("Применить маршрут EXE","Apply EXE route"); onClicked: {if(!observedProcess)return;var name=exe(observedProcess.path),rules=(prefs.routing_rules||[]).filter(function(r){return !(r.kind==="process"&&exe(r.value)===name)});if(appRoute.selectedValue!=="auto")rules.push({enabled:true,kind:"process",value:name,action:appRoute.selectedValue});save({routing_rules:rules})} }
                ActionButton { text: tr("Маршрутизация…","Routing…"); onClicked: {appSearch=exe(observedProcess.path);showAddedApps=false;selectPage(2)} }
                ActionButton { text: tr("Сеть…","Network…"); onClicked: selectPage(3) }
            }
            Label { textFormat: Text.PlainText; text: tr("Счётчики трафика доступны для потоков, которые видит ядро NetPulse. Тире означает отсутствие данных.","Traffic counters are available for flows observed by NetPulse. A dash means unavailable data."); color: mutedColor; font.pixelSize: 11; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Label { textFormat: Text.PlainText; text: analysis?tr("Соединений: ","Connections: ")+analysis.connections.length+(analysis.truncated?tr(" · список ограничен"," · truncated"):""):tr("Выберите процесс","Choose a process"); color: mutedColor }
            Repeater { model: analysis?analysis.connections:[]
                delegate: Rectangle { required property var modelData; Layout.fillWidth: true; implicitHeight: 48; color: panelColor; radius: 6
                    ColumnLayout { anchors.fill: parent; anchors.margins: 8; spacing: 3
                        Label { textFormat: Text.PlainText; text: modelData.destination||modelData.local; color: textColor; Layout.fillWidth: true; elide: Text.ElideMiddle; font.pixelSize: 12 }
                        Label { textFormat: Text.PlainText; text: modelData.network.toUpperCase()+" · "+modelData.state+" · "+modelData.route+" · ↓ "+bytes(modelData.download)+" ↑ "+bytes(modelData.upload); color: mutedColor; font.pixelSize: 10 }
                    }
                }
            }
            Label { textFormat: Text.PlainText; visible: !!observedProcess&&analysis&&!analysis.process_alive; text: tr("Процесс завершён или недоступен","The process has exited or is unavailable"); color: mutedColor }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Журнал соединений выбранного приложения","Selected application connection log"); color: textColor; font.bold: true; Layout.fillWidth: true }
                Toggle { id: flowFollow; checked: true; text: tr("Следовать","Follow") }
                ActionButton { text: tr("Копировать","Copy"); onClicked: backend.copyText((flowRecords[processKey(observedProcess)]||[]).join("\n")) }
            }
            ScrollView { Layout.fillWidth: true; Layout.preferredHeight: 210; clip: true
                TextArea { id: flowConsole; readOnly: true; selectByMouse: true; wrapMode: TextEdit.NoWrap; font.family: "Consolas"; font.pixelSize: 11; color: textColor; padding: 12
                    text: (flowRecords[processKey(observedProcess)]||[]).join("\n")||tr("При наблюдении здесь появятся обнаруженные и закрытые соединения","Observed and closed connections appear here while monitoring")
                    onTextChanged: if(flowFollow.checked)Qt.callLater(function(){flowConsole.cursorPosition=flowConsole.length})
                    background: Rectangle { color: light?"#f9faff":"#0b0e16"; border.color: borderColor; radius: 8 }
                }
            }
            Repeater { model: analysis?analysis.errors:[]
                delegate: Label { required property string modelData; textFormat: Text.PlainText; text: modelData; color: light?"#a32c43":"#ff9eae"; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true; font.pixelSize: 11 }
            }
        }
    }

    Component {
        id: activityPage
        ColumnLayout {
            id: activityView
            property var days: []
            property var events: []
            property bool follow: true
            property int requestGeneration: 0
            property string plainText: events.map(function(e){return backend.readableText(logMessage(e))}).join("\n")
            spacing: 12
            function load() {
                var generation=++requestGeneration
                if(logCategory.currentIndex===5) {call("logs",{},function(e){if(generation===requestGeneration)events=(e||[]).slice().reverse()});return}
                if(!logDay.currentText){events=[];return}
                call("log_read",{day:logDay.currentText,category:["main","core","tunnel","subscriptions","service","events"][logCategory.currentIndex]},function(text){
                    if(generation!==requestGeneration)return
                    events=text.split("\n").filter(function(x){return x.trim().length}).map(function(line){try{return JSON.parse(line)}catch(e){return {timestamp:0,level:"INFO",subsystem:"Log",code:"",message:line}}})
                })
            }
            function discover() {call("log_days",{},function(value){var old=logDay.currentText;days=value||[];logDay.currentIndex=follow?0:Math.max(0,days.indexOf(old));load()})}
            function html(s) {return String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}
            function color(level) {if(level==="ERROR")return light?"#a62442":"#ff8e9f";if(level==="WARN"||level==="WARNING")return light?"#8b5800":"#e4be76";if(level==="DEBUG")return light?"#5d667c":"#909bb4";return light?"#156143":"#83dabb"}
            Component.onCompleted: discover()
            Timer { interval: 2500; running: activityView.follow; repeat: true; onTriggered: if(!busy("log_read")&&!busy("logs")&&!busy("log_days"))activityView.discover() }
            RowLayout { Layout.fillWidth: true
                Choice { id: logCategory; objectName: "logCategory"; model: [tr("Основной","Main"),tr("Ядро","Core"),tr("Туннель","Tunnel"),tr("Подписки","Subscriptions"),tr("Служба","Service"),tr("События","Events")]; onActivated: activityView.load() }
                Choice { id: logDay; objectName: "logDay"; Layout.fillWidth: true; model: activityView.days; enabled: logCategory.currentIndex!==5; onActivated: {activityView.follow=false;activityView.load()} }
                ActionButton { text: tr("Обновить","Refresh"); onClicked: activityView.discover() }
                ActionButton { text: tr("Папка","Folder"); onClicked: call("open_log_folder") }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Следовать за журналом","Follow latest log"); color: mutedColor }
                Toggle { checked: activityView.follow; onToggled: {activityView.follow=checked;if(checked)activityView.discover()} }
                Item { Layout.fillWidth: true }
                Label { textFormat: Text.PlainText; text: activityView.events.length+tr(" строк"," lines"); color: mutedColor; font.pixelSize: 11 }
                ActionButton { text: tr("Копировать журнал","Copy log"); enabled: activityView.events.length>0; onClicked: {backend.copyText(activityView.plainText);toast(tr("Скопировано","Copied"))} }
            }
            ScrollView { id: logScroll; Layout.fillWidth: true; Layout.preferredHeight: Math.max(450,win.height-330); clip: true
                TextArea { id: consoleText; objectName: "logConsole"; readOnly: true; selectByMouse: true; textFormat: TextEdit.RichText; wrapMode: TextEdit.NoWrap; font.family: "Consolas"; font.pixelSize: 12; color: textColor; padding: 14
                    text: activityView.events.length?activityView.events.map(function(e){return '<span style="color:'+activityView.color(e.level)+'">'+activityView.html(backend.readableText(logMessage(e)))+'</span>'}).join("<br>"):activityView.html(tr("В выбранном журнале пока нет записей","No entries in this journal yet"))
                    onTextChanged: if(activityView.follow)Qt.callLater(function(){consoleText.cursorPosition=consoleText.length})
                    background: Rectangle { color: light?"#f9faff":"#0b0e16"; border.color: borderColor; radius: 10 }
                    TapHandler { acceptedButtons: Qt.RightButton; onTapped: consoleMenu.popup() }
                    Menu { id: consoleMenu
                        MenuItem { text: tr("Копировать выделенное","Copy selection"); enabled: consoleText.selectedText.length>0; onTriggered: consoleText.copy() }
                        MenuItem { text: tr("Копировать всё","Copy all"); onTriggered: backend.copyText(activityView.plainText) }
                        MenuItem { text: tr("Выделить всё","Select all"); onTriggered: consoleText.selectAll() }
                    }
                }
            }
        }
    }

    Component {
        id: settingsPage
        ColumnLayout {
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: aboutColumn.implicitHeight+40
                ColumnLayout { id: aboutColumn; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("О клиенте","About NetPulse"); color: textColor; font.pixelSize: 17; font.bold: true }
                    Label { textFormat: Text.PlainText; text: "NetPulse 1.0.0 beta · Windows x64"; color: textColor }
                    Label { textFormat: Text.PlainText; text: "Qt Quick · Rust · sing-box "+(modelData.engine_version||"—")+" · Xray "+(modelData.xray_version||"—"); color: mutedColor; font.pixelSize: 12 }
                    ActionButton { text: "GitHub · NetPulse ↗"; onClicked: Qt.openUrlExternally("https://github.com/HACKYASHER/NetPulse") }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: updateColumn.implicitHeight+40
                ColumnLayout { id: updateColumn; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("Обновление клиента","Client updates"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Label { textFormat: Text.PlainText; text: tr("Проверка при запуске и каждые 6 часов, включая предварительные выпуски. При наличии обновления кнопка появится слева внизу.","Checks run on startup and every 6 hours, including prereleases. An update button appears at the bottom left when a new version is available."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    Label { textFormat: Text.PlainText; text: updateStatus; visible: text.length>0; color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    ActionButton { text: tr("Проверить обновления","Check for updates"); enabled: !busy("check_client_update"); onClicked: checkUpdate(true) }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: appearanceForm.implicitHeight + 40
                ColumnLayout { id: appearanceForm; anchors.fill: parent; anchors.margins: 20; spacing: 18
                    Label { textFormat: Text.PlainText; text: tr("Внешний вид", "Appearance"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Тема", "Theme"); color: mutedColor; Layout.fillWidth: true }
                        Choice { model: ["dark", "light", "system"]; currentIndex: Math.max(0, model.indexOf(prefs.theme)); onActivated: save({theme: currentText}) }
                    }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Язык", "Language"); color: mutedColor; Layout.fillWidth: true }
                        Choice { model: ["Русский", "English"]; currentIndex: prefs.language === "en" ? 1 : 0; onActivated: save({language: currentIndex ? "en" : "ru"}) }
                    }
                    SettingRow { Layout.fillWidth: true; title: tr("Запускать с Windows", "Start with Windows"); description: tr("Открывает NetPulse в трее после входа в Windows. Сам по себе не включает VPN — за подключение отвечает автовосстановление.", "Starts NetPulse in the tray after Windows sign-in. It does not enable VPN by itself; auto reconnect controls that."); checked: !!prefs.autostart; onToggled: function(v) { save({autostart: v}) } }
                    SettingRow { Layout.fillWidth: true; title: tr("Закрывать в трей", "Close to tray"); description: tr("Кнопка закрытия скрывает окно; выход доступен в меню значка", "Closing hides the window; Quit is available in the tray menu"); checked: !!prefs.close_to_tray; onToggled: function(v) { save({close_to_tray: v}) } }
                    SettingRow { Layout.fillWidth: true; title: tr("Автовосстановление", "Auto reconnect"); description: tr("При запуске пытается восстановить выбранное соединение, если сохранён режим подключения. При проблемах с сервером запуск интерфейса остаётся доступным.", "Attempts to restore the selected connection on startup when a connection mode is saved. The interface remains available if the server cannot be reached."); checked: !!prefs.auto_reconnect; onToggled: function(v) { save({auto_reconnect: v}) } }
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
                    HelpText { text: tr("Временные архивы ядер очищаются после установки и при следующем запуске. Незавершённые загрузки и установщики уже установленной версии тоже удаляются; предыдущий комплект ядер остаётся для отката.","Temporary engine archives are cleared after installation and on startup. Interrupted downloads and installers for the installed version are also removed; the previous engine bundle remains for rollback.") }
                    ActionButton { text: tr("Редактор всех параметров…", "Edit all preferences…"); onClicked: editJson(tr("Параметры NetPulse", "NetPulse preferences"), JSON.stringify(prefs, null, 2), "save_preferences") }
                    Label { textFormat: Text.PlainText; text: "NetPulse 1.0.0 beta  ·  C++ / Qt Quick  ·  Rust NetworkCore"; color: mutedColor; font.pixelSize: 11 }
                }
            }
        }
    }

    Component {
        id: enginesPage
        ColumnLayout {
            id: engineView
            property string engine: "xray"
            property var releases: []
            readonly property var release: releasePicker.currentIndex>=0&&releasePicker.currentIndex<releases.length?releases[releasePicker.currentIndex]:null
            function load() { var name=engine;call("list_core_releases",{engine:name},function(items){if(engine===name){releases=items;releasePicker.currentIndex=items.length?0:-1}}) }
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: coreForm.implicitHeight+40
                ColumnLayout { id: coreForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Ядра и обновления","Engines and updates"); color: textColor; font.pixelSize: 17; font.bold: true }
                    RowLayout {
                        ActionButton { text: "Xray · "+(win.modelData.xray_version||"—"); primary: engineView.engine==="xray"; onClicked: {engineView.engine="xray";engineView.releases=[]} }
                        ActionButton { text: "sing-box · "+(win.modelData.engine_version||"—"); primary: engineView.engine==="sing-box"; onClicked: {engineView.engine="sing-box";engineView.releases=[]} }
                        ActionButton { text: tr("Проверить выпуски","Check releases"); enabled: !busy("list_core_releases"); onClicked: engineView.load() }
                    }
                    Label { textFormat: Text.PlainText; text: (engineView.engine==="xray"?win.modelData.xray_ready:win.modelData.engine_ready)?tr("● Установленное ядро проверено","● Installed engine verified"):tr("● Ядро требует проверки","● Engine needs attention"); color: light?"#17694f":"#70dbb1" }
                    RowLayout {
                        ActionButton { text: "GitHub · XTLS/Xray-core ↗"; onClicked: Qt.openUrlExternally("https://github.com/XTLS/Xray-core/releases") }
                        ActionButton { text: "GitHub · SagerNet/sing-box ↗"; onClicked: Qt.openUrlExternally("https://github.com/SagerNet/sing-box/releases") }
                    }
                    Label { textFormat: Text.PlainText; text: tr("Показаны стабильные и предварительные выпуски. Перед заменой проверяются официальный SHA-256 и конфигурация; предыдущий комплект сохраняется.","Stable and prerelease versions are included. Official SHA-256 and configuration are checked before replacing an engine; the previous bundle is retained."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    BusyIndicator { visible: busy("list_core_releases")||busy("update_core"); running: visible; implicitHeight: 28 }
                    Choice { id: releasePicker; visible: engineView.releases.length>0; Layout.fillWidth: true; model: engineView.releases.map(function(r){return r.tag+(r.prerelease?" · pre-release":" · stable")}) }
                    Card { visible: !!engineView.release; Layout.fillWidth: true; implicitHeight: releaseDetails.implicitHeight+28
                        ColumnLayout { id: releaseDetails; anchors.fill: parent; anchors.margins: 14; spacing: 12
                            Label { textFormat: Text.PlainText; text: engineView.release?engineView.release.asset:""; color: textColor }
                            Label { textFormat: Text.PlainText; text: "SHA-256: "+(engineView.release?engineView.release.archive_sha256:""); color: mutedColor; font.family: "Consolas"; font.pixelSize: 11; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                            RowLayout {
                                ActionButton { text: tr("Страница выпуска ↗","Release page ↗"); onClicked: if(engineView.release)call("open_core_page",{url:engineView.release.page}) }
                                ActionButton { text: tr("Скачать и установить","Download and install"); primary: true; enabled: !snapshot.engine_pid&&!busy("update_core"); onClicked: if(engineView.release)call("update_core",{release:engineView.release}) }
                            }
                        }
                    }
                    RowLayout {
                        ActionButton { text: tr("Откатить комплект","Roll back bundle"); enabled: !snapshot.engine_pid&&!busy("activate_core_bundle"); onClicked: call("activate_core_bundle",{rollback:true}) }
                        ActionButton { text: tr("Восстановить встроенные ядра","Restore bundled engines"); enabled: !snapshot.engine_pid&&!busy("activate_core_bundle"); onClicked: call("activate_core_bundle",{rollback:false}) }
                    }
                    Label { textFormat: Text.PlainText; text: tr("Обновление и откат доступны после отключения VPN. Защищённые копии службы обновляются с обычным UAC.","Updates and rollback are available after disconnecting VPN. Protected service copies are updated with normal UAC."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                }
            }
        }
    }

    Menu {
        id: serverMenu
        MenuItem { text: tr("Проверить пинг","Test latency"); enabled: !jobTimer.running; onTriggered: beginJob("start_latency",{ids:[serverContext.id],method:"proxy_https_rtt"}) }
        MenuItem { text: pinned(serverContext.id)?tr("Открепить","Unpin"):tr("Закрепить","Pin"); onTriggered: call("quick_ui",{patch:{favorite_id:serverContext.id}}) }
        MenuItem { text: tr("Настройка конфига","Configure server"); enabled: !subscriptions.some(function(s){return s.id===serverContext.subscription_id&&s.config_locked}); onTriggered: call("profile_json",{id:serverContext.id},function(json){editJson(cleanName(serverContext.name),json,"save_profile_json",{id:serverContext.id})}) }
        MenuSeparator {}
        MenuItem { text: tr("Удалить сервер","Delete server"); onTriggered: backend.confirmDelete("delete_profile",{id:serverContext.id},tr("Удалить сервер ","Delete server ")+cleanName(serverContext.name)+"?") }
    }
    Menu {
        id: subscriptionContext
        property var sub: ({})
        MenuItem { text: tr("Настройки подписки","Subscription settings"); onTriggered: editSubscription(subscriptionContext.sub) }
        MenuItem { text: tr("Обновить","Refresh"); onTriggered: call("update_subscription",{id:subscriptionContext.sub.id}) }
        MenuItem { text: tr("Копировать ссылку","Copy link"); enabled: !subscriptionContext.sub.url_protected; onTriggered: call("copy_subscription_url",{id:subscriptionContext.sub.id}) }
        MenuSeparator {}
        MenuItem { text: tr("Удалить подписку","Delete subscription"); onTriggered: backend.confirmDelete("delete_subscription",{id:subscriptionContext.sub.id},tr("Удалить подписку и её серверы?","Delete this subscription and its servers?")) }
    }
    Dialog {
        id: modeDialog; objectName: "connectionModeDialog"; anchors.centerIn: parent; width: 580; modal: true
        title: tr("Выберите режим подключения","Choose connection mode")
        function start(tun,proxy) {
            var p=selected();if(!p){selectPage(1);importDialog.open();return}
            function applyAndConnect(){call("quick_network",{tun:tun,system_proxy:proxy},function(){modeDialog.close();call("connect",{id:p.id})})}
            function connectNow(){if(snapshot.engine_pid)call("disconnect",{},applyAndConnect);else applyAndConnect()}
            if(tun&&!(modelData.service&&modelData.service.installed))call("manage_service",{install:true,upgrade:false},connectNow)
            else connectNow()
        }
        ColumnLayout { anchors.fill: parent; spacing: 18
            Label { textFormat: Text.PlainText; text: tr("System Proxy — приложения с поддержкой системного прокси. TUN — весь трафик с правилами приложений.","System Proxy serves apps that use the Windows proxy. TUN captures traffic and applies application rules."); color: mutedColor; Layout.fillWidth: true; wrapMode: Text.WordWrap }
            Label { textFormat: Text.PlainText; visible: !(modelData.service&&modelData.service.installed); text: tr("При выборе TUN будет установлена служба с подтверждением UAC.","Choosing TUN will install the service with Windows UAC confirmation."); color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
            RowLayout { Layout.fillWidth: true
                ActionButton { text: "System Proxy"; Layout.fillWidth: true; enabled: !busy("quick_network")&&!busy("manage_service"); onClicked: modeDialog.start(false,true) }
                ActionButton { text: "TUN"; primary: true; Layout.fillWidth: true; enabled: !busy("quick_network")&&!busy("manage_service"); onClicked: modeDialog.start(true,false) }
            }
            ActionButton { text: tr("Отмена","Cancel"); Layout.alignment: Qt.AlignRight; onClicked: modeDialog.close() }
        }
    }
    Dialog {
        id: ipDialog; anchors.centerIn: parent; width: 620; modal: true; title: tr("Публичные IP-адреса","Public IP addresses")
        ColumnLayout { anchors.fill: parent; spacing: 16
            BusyIndicator { visible: busy("public_ips"); running: visible; implicitHeight: 30 }
            Repeater { model: [{name:tr("Через VPN","Through VPN"),key:"vpn"},{name:tr("Напрямую / без NetPulse VPN","Direct / without NetPulse VPN"),key:"direct"}]
                delegate: Card { required property var modelData; Layout.fillWidth: true; implicitHeight: ipColumn.implicitHeight+30
                    ColumnLayout { id: ipColumn; anchors.fill: parent; anchors.margins: 15; spacing: 8
                        Label { textFormat: Text.PlainText; text: modelData.name; color: mutedColor; font.pixelSize: 12 }
                        RowLayout { Layout.fillWidth: true
                            TextField { text: ipResult[modelData.key]||"—"; readOnly: true; selectByMouse: true; color: textColor; font.family: "Consolas"; font.pixelSize: 21; Layout.fillWidth: true; background: null }
                            ActionButton { text: tr("Копировать","Copy"); enabled: !!ipResult[modelData.key]; onClicked: {backend.copyText(ipResult[modelData.key]);toast(tr("IP скопирован","IP copied"))} }
                        }
                        Label { textFormat: Text.PlainText; text: ipResult[modelData.key+"_error"]?ipResult[modelData.key+"_error"].message:modelData.key==="vpn"&&!snapshot.engine_pid?tr("Подключите VPN для проверки","Connect VPN to test this route"):""; visible: text.length>0; color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { text: tr("Обновить","Refresh"); enabled: !busy("public_ips"); onClicked: call("public_ips",{},function(r){ipResult=r}) }
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Закрыть","Close"); onClicked: ipDialog.close() }
            }
        }
    }
    Dialog {
        id: historyDialog; anchors.centerIn: parent; width: Math.min(850,win.width-100); height: win.height-140; modal: true; title: tr("История измерений","Measurement history")
        property var records: []
        function load(){var kind=["latency","speed","mtu","mtu_tuning"][historyKind.currentIndex];call("diagnostic_history",{kind:kind},function(items){records=items||[]})}
        ColumnLayout { anchors.fill: parent; spacing: 14
            Choice { id: historyKind; Layout.fillWidth: true; model: [tr("Пинг","Latency"),"Speed Test","MTU",tr("Сравнение MTU","MTU comparison")]; onActivated: historyDialog.load() }
            Label { textFormat: Text.PlainText; visible: historyDialog.records.length===0; text: tr("В этом разделе ещё нет измерений","No measurements in this section yet"); color: mutedColor; Layout.fillWidth: true }
            ListView { id: historyList; Layout.fillWidth: true; Layout.fillHeight: true; clip: true; model: historyDialog.records; spacing: 8; ScrollBar.vertical: ScrollBar {}
                delegate: Card { required property var modelData; width: historyList.width; implicitHeight: historyRow.implicitHeight+24
                    ColumnLayout { id: historyRow; anchors.fill: parent; anchors.margins: 12; spacing: 8
                        Label { textFormat: Text.PlainText; text: modelData.timestamp?new Date(modelData.timestamp*1000).toLocaleString():"MTU"; color: mutedColor; font.pixelSize: 11 }
                        Label { textFormat: Text.PlainText; text: resultText(modelData); color: textColor; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                        Label { textFormat: Text.PlainText; visible: !!modelData.error; text: modelData.error?modelData.error.message:""; color: light?"#a32c43":"#ff9eae"; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { text: tr("Копировать","Copy"); onClicked: backend.copyText(JSON.stringify(historyDialog.records,null,2)) }
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Закрыть","Close"); onClicked: historyDialog.close() }
            }
        }
    }
    Dialog {
        id: updateDialog; anchors.centerIn: parent; width: 600; modal: true; title: tr("Обновление NetPulse","NetPulse update")
        ColumnLayout { anchors.fill: parent; spacing: 16
            Label { textFormat: Text.PlainText; text: clientUpdate?tr("Доступна версия ","Available version ")+clientUpdate.tag:updateStatus; color: textColor; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Label { textFormat: Text.PlainText; text: tr("Установщик проверяется по SHA-256 из официального выпуска. Установка отключит VPN и завершит клиент.","The installer is checked against the official release SHA-256. Installation disconnects VPN and closes the client."); color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
            BusyIndicator { visible: busy("download_client_update")||busy("install_client_update"); running: visible; implicitHeight: 30 }
            RowLayout { Layout.fillWidth: true
                ActionButton { text: tr("Страница выпуска ↗","Release page ↗"); enabled: !!clientUpdate; onClicked: if(clientUpdate)Qt.openUrlExternally(clientUpdate.page) }
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Закрыть","Close"); onClicked: updateDialog.close() }
                ActionButton { text: updateDownloaded?tr("Установить","Install"):tr("Скачать","Download"); primary: true; enabled: !!clientUpdate&&!busy("download_client_update")&&!busy("install_client_update"); onClicked: {if(updateDownloaded)call("install_client_update",{release:clientUpdate},function(){backend.quit()});else call("download_client_update",{release:clientUpdate},function(){updateDownloaded=true;toast(tr("Установщик проверен","Installer verified"))})} }
            }
        }
    }
    Dialog {
        id: importDialog; objectName: "addConnectionDialog"; anchors.centerIn: parent; width: 650; modal: true
        title: tr("Добавить подключение", "Add connection"); standardButtons: Dialog.NoButton
        ColumnLayout { anchors.fill: parent; spacing: 14
            Choice { id: importKind; Layout.fillWidth: true; model: [tr("Подписка HTTPS","HTTPS subscription"),tr("Ключ или конфигурация","Link or configuration")] }
            Input { id: importName; visible: importKind.currentIndex===0; Layout.fillWidth: true; placeholderText: tr("Название подписки","Subscription name") }
            Label { textFormat: Text.PlainText; text: importKind.currentIndex===0?tr("Ссылка хранится в зашифрованном виде","The URL is stored encrypted"):tr("Вставьте ключ или конфигурацию","Paste a connection link or configuration"); color: mutedColor; font.pixelSize: 12 }
            ScrollView { Layout.fillWidth: true; Layout.preferredHeight: 150
                TextArea { id: importText; placeholderText: importKind.currentIndex===0?"https://…":"vless://…  trojan://…  ss://…"; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; color: textColor; font.pixelSize: 13; background: Rectangle { color: palette.base; border.color: borderColor; radius: 9 } }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { text: tr("Буфер","Clipboard"); onClicked: importText.text=backend.clipboardText() }
                ActionButton { text: tr("Файл","File"); onClicked: {call("import_file",{qr:false});importDialog.close()} }
                ActionButton { text: "QR"; onClicked: {call("import_file",{qr:true});importDialog.close()} }
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Отмена","Cancel"); onClicked: {importText.clear();importDialog.close()} }
                ActionButton { text: tr("Добавить","Add"); primary: true; enabled: importText.text.trim().length>0&&!busy("import_text")&&!busy("add_subscription"); onClicked: {var done=function(){importText.clear();importName.clear();importDialog.close()};if(importKind.currentIndex===0)call("add_subscription",{name:importName.text||tr("Подписка","Subscription"),url:importText.text.trim(),protected:true},done);else call("import_text",{text:importText.text},done)} }
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
                ActionButton { text: tr("Копировать","Copy"); onClicked: backend.copyText(jsonEditor.text) }
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
            CheckBox { id: subEditProtect; text: tr("Запретить копирование ссылки","Protect subscription URL") }
            CheckBox { id: subEditLock; enabled: !subEdit.sub.config_locked; text: tr("Запретить редактирование профилей", "Lock profile configuration") }
            Label { textFormat: Text.PlainText; text: tr("Блокировку конфигурации после сохранения снять нельзя. Обновления от провайдера продолжат работать.","Configuration locking is permanent. Provider updates will continue to work."); color: mutedColor; font.pixelSize: 11; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            RowLayout { Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                ActionButton { text: tr("Отмена", "Cancel"); onClicked: subEdit.close() }
                ActionButton { text: tr("Сохранить", "Save"); primary: true; onClicked: call("save_subscription", {id: subEdit.sub.id, name: subEditName.text, interval_hours: subEditHours.value, protected: subEditProtect.checked, lock_configuration: subEditLock.checked}, function() { subEdit.close() }) }
            }
        }
    }
}
