import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import "LogText.js" as LogText

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
    property bool diagnosticRunning: jobTimer.running
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
    property int smokeCoreRows: 0
    property bool smokePendingCore: false
    property string smokeError: ""
    readonly property string displayError: backend.errorMessage || (smokeMode ? smokeError : "")
    readonly property bool activityFitsViewport: page !== 6 || (!!pageLoader.item && pageLoader.item.consoleFitsViewport && Math.abs(bodyScroll.contentHeight-bodyScroll.availableHeight)<1)
    property var smokeSequence: [{page:0,name:"overview"},{page:1,name:"servers"},{page:2,name:"routing"},{page:3,tab:0,name:"connection"},{page:3,tab:1,name:"dns"},{page:3,tab:2,name:"mtu"},{page:3,tab:3,name:"tls"},{page:3,tab:4,name:"transport"},{page:3,tab:5,name:"service"},{page:3,tab:6,name:"engines"},{page:3,tab:7,name:"sniffing"},{page:4,name:"diagnostics"},{page:5,name:"playground"},{page:6,name:"activity"},{page:6,category:1,name:"core-log"},{page:6,name:"activity-small-error",error:true,width:1000,height:700},{page:6,name:"activity-large-error",error:true,width:1920,height:1000},{page:7,name:"settings"},{page:0,dialog:"server",name:"server-picker"},{page:0,dialog:"mode",name:"connection-mode"},{page:1,dialog:"import",name:"add-connection"},{page:4,dialog:"history",name:"measurement-history"}]
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
    function connectTun(){setModes(!prefs.tun,!!prefs.system_proxy)}
    function connectProxy(){setModes(!!prefs.tun,!prefs.system_proxy)}
    function beginJob(method, params) { call(method, params, function(id) { diagnosticId = id; job = null; jobTimer.start() }) }
    function pingMethod() { return prefs.ping_method || "tcp_connect_rtt" }
    function pingLabel() { return ({proxy_https_rtt:"via Proxy GET",proxy_https_head_rtt:"via Proxy HEAD",tcp_connect_rtt:"TCP",icmp_rtt:"ICMP"})[pingMethod()] || "TCP" }
    function testServers(ids) { if (!ids.length || jobTimer.running || savingPreferences) return; beginJob("start_latency",{ids:ids,method:pingMethod()}) }
    function setModes(tun, proxy) {
        if (busy("quick_network")||busy("manage_service")||savingPreferences||diagnosticRunning)return
        if (snapshot.engine_pid) {
            if(tun&&!(modelData.service&&modelData.service.installed))call("manage_service",{install:true,upgrade:false},function(){call("quick_network",{tun:tun,system_proxy:proxy})})
            else call("quick_network",{tun:tun,system_proxy:proxy})
        } else save({tun:tun,system_proxy:proxy})
    }
    function applyNetworkPreset(preset,mtu) {
        var patch={network_preset:preset}
        if(preset!=="games")patch.mtu=mtu
        if(preset!=="custom") {patch.dns={mode:"https",server:"1.1.1.1",path:"/dns-query"};patch.dns_route=preset==="compatibility"?"direct":"vpn"}
        if(preset==="games")patch.tun=true
        if(preset==="games"&&snapshot.engine_pid&&!(modelData.service&&modelData.service.installed))call("manage_service",{install:true,upgrade:false},function(){save(patch)})
        else save(patch)
    }
    function subscriptionName(profile) { var sub=subscriptions.find(function(x){return profile && x.id===profile.subscription_id});return sub?sub.name:"" }
    function pingValue() {
        var p=displayedServer();if(!p)return "—"
        var cached=profiles.find(function(x){return x.id===p.id})
        if(cached&&cached.latency_method===pingMethod())return cached.latency_ms===null||cached.latency_ms===undefined?tr("нет ответа","no response"):Math.round(cached.latency_ms)+" ms"
        return activeProbe.id===p.id&&activeProbe.method===pingMethod()?(activeProbe.ms===null?tr("нет ответа","no response"):Math.round(activeProbe.ms)+" ms"):"—"
    }

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
        var groups=subscriptions.map(function(x){ return Object.assign({},x,{items:[],total_count:0}) })
        groups.push({id:"",name:tr("Отдельные серверы","Standalone servers"),items:[],total_count:0})
        profiles.forEach(function(p){
            var g=groups.find(function(g){return g.id===(p.subscription_id||"")});if(!g)return
            g.total_count++
            if ((cleanName(p.name)+" "+p.host+" "+p.protocol).toLowerCase().indexOf(profileSearch.toLowerCase())<0) return
            if(sourceFilter!=="all" && (p.subscription_id||"")!==sourceFilter) return
            g.items.push(p)
        })
        groups.forEach(function(g){g.items.sort(function(a,b){
            var pins=Number(pinned(b.id))-Number(pinned(a.id));if(pins)return pins
            if(serverSort===1)return (a.latency_ms===null?1e9:a.latency_ms)-(b.latency_ms===null?1e9:b.latency_ms)
            return cleanName(a.name).localeCompare(cleanName(b.name))
        })})
        return groups.filter(function(g){return g.id ? sourceFilter==="all"||sourceFilter===g.id : g.items.length>0})
    }
    function refreshPing() {
        var p=displayedServer(); if(!p || jobTimer.running || savingPreferences)return
        activeProbe={id:p.id,ms:null,method:pingMethod()}; testServers([p.id])
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
    function logMessage(event) {
        var message=backend.readableText(event.message)
        var hint=(event.level==="ERROR"||event.level==="WARNING"||event.level==="WARN")?LogText.hint(message,prefs.language!=="en"):""
        return new Date(event.timestamp*1000).toLocaleString()+"  ["+event.level+"]  "+event.subsystem+" / "+event.code+"  "+message+(hint?"\n    ↳ "+hint:"")
    }
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
    Timer { interval: 6*60*60*1000; running: backend.ready && !smokeMode; repeat: true; onTriggered: checkUpdate(false) }
    Timer { id: toastTimer; interval: 4500; onTriggered: notice = "" }
    Timer {
        id: jobTimer; interval: 1000; repeat: true
        onTriggered: {
            if (!diagnosticId || busy("diagnostic_status")) return
            call("diagnostic_status", {id: diagnosticId}, function(result) {
                job = result
                if (!result.running) {
                    if(result.kind==="latency") (result.results||[]).forEach(function(r){if(r.id===activeProbe.id || r.profile_id===activeProbe.id)activeProbe={id:activeProbe.id,ms:r.avg_ms===undefined?null:r.avg_ms,method:r.method||pingMethod()}})
                    jobTimer.stop(); backend.refresh()
                }
            })
        }
    }
    Timer { interval: 1000; running: observing; repeat: true; onTriggered: { observationSeconds++; if (observationSeconds >= 5 && !busy("connection_analysis")) { observationSeconds = 0; observe() } } }
    Timer { id: smokeTimer; interval: 750; repeat: true; onTriggered: {
        if(pageLoader.status!==Loader.Ready||smokeCaptured!==smokeFrame)return
        smokeCaptured=""
        modeDialog.close();importDialog.close();historyDialog.close()
        if(pageLoader.item&&pageLoader.item.closeServerPicker)pageLoader.item.closeServerPicker()
        smokeStep++
        if(smokeStep>=smokeSequence.length){stop();page=0;smokePages=8;smokeFinished=true;return}
        var step=smokeSequence[smokeStep];page=step.page
        smokeError=step.error ? tr("Файлы службы недоступны. Проверьте отчёт антивируса; обновление остановлено.","Service components are unavailable. Check the antivirus report; maintenance stopped.") : ""
        width=step.width||1220;height=step.height||840
        Qt.callLater(function(){
            if(step.tab!==undefined&&pageLoader.item)pageLoader.item.tab=step.tab
            if(step.dialog==="server"&&pageLoader.item)pageLoader.item.openServerPicker()
            if(step.dialog==="mode")modeDialog.open()
            if(step.dialog==="import")importDialog.open()
            if(step.dialog==="history"){historyDialog.open();historyDialog.load()}
            if(step.category!==undefined&&pageLoader.item){smokePendingCore=true;pageLoader.item.selectCategory(step.category)}
            else smokeFrame=step.name
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
                        Layout.fillWidth: true; implicitHeight: 45; topPadding: 7; bottomPadding: 7; leftPadding: 12; rightPadding: 12
                        contentItem: RowLayout { spacing: 13
                            Item { Layout.preferredWidth: 24; Layout.preferredHeight: 28; Layout.alignment: Qt.AlignVCenter
                                Label { anchors.centerIn: parent; text: ["◉", "⇄", "⑂", "⌘", "⌁", "◎", "≡", "⚙"][index]; color: page === index ? accentColor : mutedColor; font.pixelSize: 20; horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter }
                            }
                            Label { textFormat: Text.PlainText; text: modelData; color: page === index ? textColor : mutedColor; font.pixelSize: 13; font.weight: page === index ? Font.DemiBold : Font.Normal; Layout.fillWidth: true }
                        }
                        background: Rectangle { color: page === index ? (light ? "#e0dcfb" : "#2a2544") : hovered ? (light ? "#e5e8f1" : "#1c2030") : "transparent"; radius: 10 }
                        onClicked: selectPage(index)
                    }
                }
                Item { Layout.fillHeight: true }
                Rectangle {
                    Layout.fillWidth: true; implicitHeight: 34; radius: 10; color: snapshot.state === "connected" ? (light ? "#def4eb" : "#17392f") : panelColor; border.color: borderColor
                    RowLayout { anchors.centerIn: parent; spacing: 7
                        Rectangle { width: 6; height: 6; radius: 3; color: snapshot.state === "connected" ? (light?"#17694f":"#70dbb1") : mutedColor }
                        Label { text: stateLabel(); color: snapshot.state === "connected" ? (light?"#17694f":"#70dbb1") : mutedColor; font.pixelSize: 11 }
                    }
                }
                ActionButton { appWindow: win; Layout.fillWidth: true; visible: !!clientUpdate; text: tr("Обновить клиент","Update client"); primary: true; onClicked: updateDialog.open() }
                Card {
                    Layout.fillWidth: true; implicitHeight: 64
                    RowLayout { anchors.fill: parent; anchors.margins: 12; spacing: 9
                        Rectangle { width: 7; height: 7; radius: 4; color: modelData.engine_ready ? "#6bd9b0" : "#e0b86d" }
                        Column { spacing: 3; Label { textFormat: Text.PlainText; text: modelData.engine_ready ? tr("Ядра готовы", "Engines ready") : modelData.runtime_preparing ? tr("Подготовка ядер…", "Preparing engines…") : tr("Проверьте ядра", "Check engines"); font.pixelSize: 11; color: textColor } Label { textFormat: Text.PlainText; text: "NetPulse 1.0.7"; font.pixelSize: 10; color: mutedColor } }
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
            }
            Rectangle { Layout.fillWidth: true; Layout.preferredHeight: 1; color: borderColor }
            Rectangle {
                visible: displayError.length > 0
                Layout.fillWidth: true; Layout.leftMargin: 30; Layout.rightMargin: 30; Layout.topMargin: 18
                implicitHeight: errorRow.implicitHeight + 24; radius: 10; color: light ? "#ffe6e9" : "#39232d"
                RowLayout { id: errorRow; anchors.fill: parent; anchors.margins: 12
                    Label { textFormat: Text.PlainText; text: displayError; wrapMode: Text.WordWrap; Layout.fillWidth: true; color: light ? "#9c3646" : "#ffb0bd"; font.pixelSize: 12 }
                    ToolButton { text: "×"; onClicked: backend.clearError() }
                }
            }
            ScrollView {
                id: bodyScroll
                Layout.fillWidth: true; Layout.fillHeight: true; Layout.margins: 30; Layout.topMargin: 24
                contentWidth: availableWidth; contentHeight: page === 6 ? availableHeight : pageLoader.height; clip: true; rightPadding: 14
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded; width: 8; anchors.right: parent.right; anchors.top: parent.top; anchors.bottom: parent.bottom }
                ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
                Loader {
                    id: pageLoader
                    width: bodyScroll.availableWidth
                    height: page === 6 ? bodyScroll.availableHeight : implicitHeight
                    sourceComponent: pages[page]
                }
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
            function openServerPicker(){quickServer.clicked()}
            function closeServerPicker(){quickServer.closePicker()}
            spacing: 18
            Card {
                Layout.fillWidth: true; implicitHeight: heroContents.implicitHeight+42
                border.color: light ? "#bfb4f2" : "#5b4c8b"
                gradient: Gradient { GradientStop { position: 0; color: light ? "#eeeaff" : "#211d34" } GradientStop { position: 1; color: panelColor } }
                ColumnLayout {
                    id: heroContents; anchors.fill: parent; anchors.margins: 21; spacing: 20
                    Label { text: "●  "+stateLabel(); color: snapshot.engine_pid ? (light?"#17694f":"#70dbb1") : mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        Layout.fillWidth: true; spacing: 20
                        ColumnLayout { Layout.fillWidth: true; spacing: 9
                            Label { text: tr("Выбранный сервер","Selected server"); color: mutedColor; font.pixelSize: 12 }
                            RowLayout { Layout.fillWidth: true; spacing: 8
                                Flag { visible: !!displayedServer(); name: displayedServer()?displayedServer().name:""; Layout.preferredWidth: 26; Layout.preferredHeight: 18 }
                                Label { textFormat: Text.PlainText; text: displayedServer()?cleanName(displayedServer().name):tr("Добавьте сервер или подписку","Add a server or subscription"); color: textColor; Layout.fillWidth: true; elide: Text.ElideRight; font.pixelSize: 26; font.weight: Font.DemiBold }
                            }
                            Label { textFormat: Text.PlainText; text: displayedServer()?displayedServer().protocol.toUpperCase()+" · "+displayedServer().host+":"+displayedServer().port:""; color: mutedColor; font.pixelSize: 12; Layout.fillWidth: true; elide: Text.ElideRight }
                            Label { textFormat: Text.PlainText; visible: text.length>0; text: subscriptionName(displayedServer()); color: mutedColor; font.pixelSize: 11; padding: 6; background: Rectangle { color: "transparent"; border.color: borderColor; radius: 6 } }
                        }
                        Button {
                            id: powerButton; objectName: "overviewPower"; Layout.preferredWidth: 166; Layout.preferredHeight: 94
                            enabled: backend.ready&&!busy("select_profile")&&!busy("connect")&&!busy("disconnect")&&!busy("quick_network")&&!savingPreferences&&!(job&&job.running&&job.kind==="mtu_tuning")
                            contentItem: Column { spacing: 5; anchors.centerIn: parent
                                Label { anchors.horizontalCenter: parent.horizontalCenter; text: "⏻"; font.pixelSize: 34; color: powerButton.enabled ? "#111122" : mutedColor }
                                Label { anchors.horizontalCenter: parent.horizontalCenter; text: snapshot.engine_pid?tr("Отключиться","Disconnect"):tr("Подключиться","Connect"); font.pixelSize: 13; font.bold: true; color: powerButton.enabled ? "#111122" : mutedColor }
                            }
                            background: Rectangle { radius: 15; color: !powerButton.enabled ? (light?"#ddd9ed":"#343043") : powerButton.down ? "#7762e7" : powerButton.hovered ? "#a798ff" : "#927fff" }
                            onClicked: connectSelected()
                        }
                    }
                    Rectangle { Layout.fillWidth: true; height: 1; color: borderColor }
                    RowLayout { Layout.fillWidth: true; spacing: 12
                        ServerPicker {
                            id: quickServer; objectName: "overviewServerPicker"
                            appWindow: win; profiles: win.profiles; selectedProfile: displayedServer()
                            Layout.preferredWidth: 280; Layout.minimumWidth: 200; Layout.maximumWidth: 280; Layout.alignment: Qt.AlignBottom
                            busy: win.busy("select_profile")||win.busy("connect")||win.busy("disconnect")||win.busy("quick_network")||savingPreferences||!!(job&&job.running)
                            onChosen: function(id){call("select_profile",{id:id},function(){quickServer.closePicker()})}
                            onManageRequested: showServers()
                        }
                        ColumnLayout { spacing: 4; Layout.preferredWidth: 170
                            Label { text: tr("Ядро","Engine"); color: mutedColor; font.pixelSize: 11 }
                            Choice { appWindow: win; id: overviewEngine; objectName: "overviewEngine"; implicitHeight: 44; Layout.fillWidth: true; Layout.maximumWidth: 170; implicitWidth: 170; model: [tr("Авто","Auto"),"sing-box","Xray"]; values: ["auto","sing-box","xray"]; currentIndex: Math.max(0,values.indexOf(prefs.transport_engine||"auto")); enabled: !busy("quick_network")&&!busy("select_profile")&&!busy("connect")&&!busy("disconnect")&&!savingPreferences&&!(job&&job.running); onActivated: { call("quick_network",{transport_engine:selectedValue});currentIndex=Qt.binding(function(){return Math.max(0,overviewEngine.values.indexOf(prefs.transport_engine||"auto"))}) } }
                        }
                        Item { Layout.fillWidth: true }
                        Label { Layout.alignment: Qt.AlignBottom; Layout.bottomMargin: 15; text: pingLabel()+" · "+(jobTimer.running&&job&&job.kind==="latency"?tr("проверка…","testing…"):pingValue()); color: mutedColor; font.pixelSize: 11 }
                        ActionButton { appWindow: win; objectName: "overviewPingRefresh"; Layout.alignment: Qt.AlignBottom; Layout.bottomMargin: 3; implicitHeight: 38; text: "↻"; implicitWidth: 38; enabled: !!displayedServer()&&!jobTimer.running&&!savingPreferences; ToolTip.text: tr("Обновить пинг. Тип проверки задаётся в Диагностике.","Refresh latency. Select the method in Diagnostics."); ToolTip.visible: hovered; onClicked: refreshPing() }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true; spacing: 14
                Metric { Layout.fillWidth: true; Layout.preferredWidth: 1; symbol: "↓"; tint: light?"#6047ce":"#a18dff"; label: tr("Загрузка","Download"); value: snapshot.engine_pid&&snapshot.metrics?bytes(snapshot.metrics.download_bps)+"/s":"—"; hint: tr("Всего: ","Total: ")+bytes(snapshot.metrics?snapshot.metrics.download_total:null) }
                Metric { Layout.fillWidth: true; Layout.preferredWidth: 1; symbol: "↑"; tint: light?"#16824d":"#35d58b"; label: tr("Отправка","Upload"); value: snapshot.engine_pid&&snapshot.metrics?bytes(snapshot.metrics.upload_bps)+"/s":"—"; hint: tr("Всего: ","Total: ")+bytes(snapshot.metrics?snapshot.metrics.upload_total:null) }
                Metric { Layout.fillWidth: true; Layout.preferredWidth: 1; symbol: "⇄"; tint: light?"#2467bd":"#62afff"; label: tr("Активные соединения","Active connections"); value: snapshot.engine_pid&&snapshot.metrics&&snapshot.metrics.connections!==undefined?String(snapshot.metrics.connections):"—"; hint: snapshot.engine_pid?tr("Соединения текущего ядра","Current engine connections"):tr("Подключение не активно","Connection is inactive") }
            }
            Card { Layout.fillWidth: true; implicitHeight: modesContents.implicitHeight+36
                ColumnLayout { id: modesContents; anchors.fill: parent; anchors.margins: 18; spacing: 12
                    Label { text: tr("Режим подключения","Connection mode"); color: textColor; font.bold: true; font.pixelSize: 15 }
                    ConnectionModes { appWindow: win; Layout.fillWidth: true; tun: !!prefs.tun; proxy: !!prefs.system_proxy; tunActive: !!snapshot.tun_active; proxyActive: !!snapshot.system_proxy_active; interactive: !busy("quick_network")&&!busy("manage_service")&&!savingPreferences&&!(job&&job.running); onChanged: function(tun,proxy){setModes(tun,proxy)} }
                    HelpText { text: snapshot.engine_pid?tr("Изменения применяются сразу. Отключение обоих режимов завершает VPN-соединение.","Changes apply immediately. Turning both modes off disconnects VPN."):tr("Можно включить оба режима. При подключении подтвердите выбранные режимы.","Both modes can be enabled together. Confirm your choices when connecting.") }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { text: tr("Серверов: ", "Servers: ") + profiles.length + tr("    Подписок: ", "    Subscriptions: ") + subscriptions.length; color: mutedColor; font.pixelSize: 12; Layout.fillWidth: true }
                ActionButton { appWindow: win; text: tr("Проверить IP", "Check IP"); enabled: !busy("public_ips"); onClicked: showPublicIps() }
            }
        }
    }

    Component { id: serversPage; ServersView { appWindow: win; onAddRequested: importDialog.open() } }

    Component {
        id: routingPage
        ColumnLayout { spacing: 16
            Card { Layout.fillWidth: true; implicitHeight: routingForm.implicitHeight+40
                ColumnLayout { id: routingForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Режим маршрутизации","Routing mode"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Choice { appWindow: win; Layout.fillWidth: true; model: [tr("Весь трафик через VPN","All traffic over VPN"),tr("Только выбранные приложения","Selected applications only"),tr("Кроме выбранных приложений","Except selected applications")]; currentIndex: ["global","whitelist","blacklist"].indexOf(prefs.route_mode||"global"); onActivated: call("quick_network",{route_mode:["global","whitelist","blacklist"][currentIndex]}) }
                    Label { textFormat: Text.PlainText; text: tr("Правила совпадают по имени EXE и применяются в TUN. Список сохраняется в routing-apps.json.","Rules match EXE names and apply in TUN. The list is saved in routing-apps.json."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        ActionButton { appWindow: win; text: tr("Импорт правил","Import rules"); onClicked: call("import_routing",{},function(r){if(r)save({route_mode:r.route_mode,applications:r.applications,application_groups:r.application_groups,bypass_domains:r.bypass_domains,routing_rules:r.routing_rules,sniff_excluded_apps:r.sniff_excluded_apps})}) }
                        ActionButton { appWindow: win; text: tr("Экспорт правил","Export rules"); onClicked: call("export_routing",{routing:routingDocument()}) }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true
                Label { textFormat: Text.PlainText; text: tr("Приложения","Applications")+" · "+(prefs.applications||[]).length; color: textColor; font.pixelSize: 17; font.bold: true; Layout.fillWidth: true }
                ActionButton { appWindow: win; text: showAddedApps?tr("Показать все найденные","Show all discovered"):tr("Показать добавленные","Show added"); onClicked: showAddedApps=!showAddedApps }
                ActionButton { appWindow: win; text: tr("Сканировать","Scan"); enabled: !busy("scan_applications"); onClicked: call("scan_applications",{},function(items){applications=items;showAddedApps=false;call("save_application_catalog",{applications:items})}) }
                ActionButton { appWindow: win; text: "+ EXE"; onClicked: call("choose_exe",{},function(item){if(item){applications=applications.concat([item]);call("save_application_catalog",{applications:applications});var list=(prefs.applications||[]).slice();if(list.indexOf(exe(item.path))<0)list.push(exe(item.path));save({applications:list})}}) }
            }
            Input { Layout.fillWidth: true; placeholderText: tr("Поиск приложения…","Search applications…"); text: appSearch; onTextChanged: appSearch=text }
            Label { textFormat: Text.PlainText; visible: routingApps().length===0; text: tr("Добавьте EXE или запустите сканирование","Add an EXE or scan installed applications"); color: mutedColor }
            RoutingApplications { Layout.fillWidth: true; appWindow: win }
            Card { Layout.fillWidth: true; implicitHeight: domainsForm.implicitHeight+40
                ColumnLayout { id: domainsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Исключения доменов из VPN","Domain VPN exceptions"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Label { textFormat: Text.PlainText; text: tr("Один домен или суффикс в строке: .ru, .su, example.com. Эти адреса идут напрямую.","One domain or suffix per line: .ru, .su, example.com. These addresses use the direct route."); color: mutedColor; Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12 }
                    ScrollView { Layout.fillWidth: true; Layout.preferredHeight: 105
                        TextArea { id: bypassInput; objectName: "domainExceptions"; placeholderText: "example.com\n*.example.org\n.ru"; text: (prefs.bypass_domains||[]).join("\n"); color: textColor; selectByMouse: true; wrapMode: TextEdit.Wrap; background: Rectangle { color: palette.base; border.color: borderColor; radius: 8 } }
                    }
                    ActionButton { appWindow: win; text: tr("Сохранить исключения","Save exceptions"); onClicked: save({bypass_domains:bypassInput.text.split(/[\n,;]+/).map(function(v){return v.trim()}).filter(function(v){return v.length})}) }
                    Label { textFormat: Text.PlainText; text: tr("Дополнительный маршрут домена","Custom domain route"); color: textColor; font.bold: true }
                    RowLayout { Layout.fillWidth: true
                        Input { id: routeDomain; placeholderText: ".ru / example.com"; Layout.fillWidth: true }
                        Choice { appWindow: win; id: routeAction; model: ["direct","proxy","reject"] }
                        ActionButton { appWindow: win; text: "+"; onClicked: {var r=(prefs.routing_rules||[]).slice();r.push({enabled:true,kind:"domain",value:routeDomain.text.trim(),action:routeAction.currentText});save({routing_rules:r});routeDomain.clear()} }
                    }
                    Repeater { model: prefs.routing_rules||[]
                        delegate: RowLayout { required property var modelData; required property int index; Layout.fillWidth: true
                            Label { textFormat: Text.PlainText; text: modelData.value+" → "+modelData.action; color: textColor; Layout.fillWidth: true }
                            ActionButton { appWindow: win; text: "×"; onClicked: {var r=(prefs.routing_rules||[]).slice();r.splice(index,1);save({routing_rules:r})} }
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
                Repeater { model: [tr("Подключение","Connection"),"DNS","MTU","SNI / TLS",tr("Транспорт","Transport"),tr("Служба TUN","TUN service"),tr("Ядра","Engines"),"Sniffing"]
                    delegate: ActionButton { appWindow: win; required property int index; required property string modelData; Layout.fillWidth: true; Layout.preferredWidth: 1; implicitWidth: 70; leftPadding: 6; rightPadding: 6; font.pixelSize: 11; text: modelData; primary: networkView.tab===index; onClicked: networkView.tab=index }
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
                        HelpText { text: tr("Системным прокси Windows управляет один клиент. Если другой VPN-клиент уже включил его, NetPulse покажет конфликт. Для двух клиентов нужны разные локальные порты; смена порта сама по себе не передаёт управление системным прокси.","One client controls the Windows system proxy. NetPulse reports a conflict if another VPN client already enabled it. Two clients need different local ports; changing a port alone does not transfer control of the system proxy.") }
                        ActionButton { appWindow:win; text:tr("Параметры прокси Windows","Windows proxy settings"); onClicked:backend.openProxySettings() }
                        Label { text: tr("IPv6 в TUN","TUN IPv6"); color: mutedColor }
                        Choice { appWindow: win; objectName: "tunIpv6Mode"; Layout.fillWidth: true; model: [tr("Автоматически","Automatic"),tr("Только IPv4","IPv4 only"),tr("IPv4 и IPv6","IPv4 and IPv6")]; values: ["auto","ipv4_only","dual_stack"]; currentIndex: Math.max(0,values.indexOf(prefs.ipv6_mode||"auto")); enabled: !savingPreferences&&!busy("select_profile")&&!busy("quick_network"); onActivated: save({ipv6_mode:selectedValue}) }
                        HelpText { text: tr("В режиме только выбранных приложений Авто включает IPv6 в TUN, когда у физического подключения есть IPv6-маршрут. Это предотвращает ложный IPv6-доступ у остальных приложений. Для IPv6 через сервер можно явно выбрать IPv4 и IPv6; сервер должен его поддерживать. Весь трафик через VPN сохраняет IPv6 в режиме Авто.","In selected-app mode, Auto enables TUN IPv6 when the physical connection has an IPv6 route, preventing unusable IPv6 for other applications. Select IPv4 and IPv6 explicitly for server-side IPv6; the server must support it. Global VPN keeps IPv6 in Auto mode.") }
                        SettingRow { Layout.fillWidth: true; title: "Fake IP"; description: tr("Выдаёт временные адреса для сопоставления доменов в TUN. После изменения переоткройте соединения приложений.","Uses temporary addresses to match domains in TUN. Reopen application connections after changing this setting."); checked: !!prefs.fake_ip; onToggled: function(v){save({fake_ip:v})} }
                        Input { id: probeUrl; Layout.fillWidth: true; text: prefs.probe_url||"https://www.gstatic.com/generate_204"; placeholderText: "HTTPS probe URL" }
                        HelpText { text: tr("HTTPS-адрес для проверки доступности VPN и пинга. Выбирайте небольшой ресурс без авторизации; недоступный сайт может давать ошибку при рабочем VPN.","The HTTPS address used for VPN health and latency checks. Use a small resource without authentication; a blocked site can fail even when VPN works.") }
                        ActionButton { appWindow: win; text: tr("Сохранить подключение","Save connection"); primary: true; onClicked: save({proxy_port:proxyPort.value,probe_url:probeUrl.text}) }
                    }
                }
                Card { visible: networkView.tab===1; Layout.fillWidth: true; implicitHeight: dnsForm.implicitHeight+40
                    ColumnLayout { id: dnsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Component.onCompleted: { dnsMode.currentIndex=Math.max(0,dnsMode.values.indexOf(prefs.dns?prefs.dns.mode:"system"));dnsServer.text=prefs.dns&&prefs.dns.server?prefs.dns.server:"1.1.1.1";dnsPath.text=prefs.dns&&prefs.dns.path?prefs.dns.path:"/dns-query";dnsRoute.currentIndex=prefs.dns_route==="vpn"?1:0 }
                        Label { textFormat: Text.PlainText; text: "DNS"; color: textColor; font.pixelSize: 16; font.bold: true }
                        HelpText { text: tr("DNS переводит имена сайтов в IP-адреса. Системный использует настройки Windows; UDP отправляет обычные запросы, TLS и HTTPS шифруют запросы до выбранного резолвера.","DNS translates site names to IP addresses. System uses Windows settings; UDP sends standard queries, while TLS and HTTPS encrypt queries to the resolver.") }
                        Choice { appWindow: win; id: dnsMode; objectName: "dnsMode"; Layout.fillWidth: true; values: ["system","udp","tls","https"]; model: [tr("Системный DNS","System DNS"),"UDP","DNS over TLS","DNS over HTTPS"]; onActivated: {if(!dnsServer.text)dnsServer.text="1.1.1.1";if(selectedValue==="system")dnsRoute.currentIndex=0} }
                        Input { id: dnsServer; objectName: "dnsServer"; visible: dnsMode.selectedValue!=="system"; Layout.fillWidth: true; placeholderText: "1.1.1.1" }
                        Input { id: dnsPath; visible: dnsMode.selectedValue==="https"; Layout.fillWidth: true; placeholderText: "/dns-query" }
                        HelpText { text: tr("Адрес резолвера можно изменить. Для HTTPS путь обычно /dns-query. Маршрут DNS определяет путь запросов к резолверу и не меняет маршруты самих приложений.","You can change the resolver address. HTTPS usually uses /dns-query. The DNS route controls queries to the resolver, not application routing.") }
                        Choice { appWindow: win; id: dnsRoute; Layout.fillWidth: true; model: [tr("DNS напрямую","DNS over direct route"),tr("DNS через VPN","DNS over VPN")]; enabled: dnsMode.selectedValue!=="system" }
                        ActionButton { appWindow: win; objectName: "saveDns"; text: tr("Сохранить DNS","Save DNS"); primary: true; enabled: !savingPreferences; onClicked: {var d={mode:dnsMode.selectedValue};if(d.mode!=="system")d.server=dnsServer.text.trim()||"1.1.1.1";if(d.mode==="https")d.path=dnsPath.text||"/dns-query";save({dns:d,dns_route:d.mode==="system"?"direct":dnsRoute.currentIndex?"vpn":"direct"})} }
                    }
                }
                Card { visible: networkView.tab===2; Layout.fillWidth: true; implicitHeight: mtuForm.implicitHeight+40
                    ColumnLayout { id: mtuForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Label { text: tr("MTU и пресеты","MTU and presets"); color: textColor; font.pixelSize: 16; font.bold: true }
                        HelpText { text: tr("MTU — максимальный размер IP-пакета адаптера NetPulse. Слишком большой размер может вызвать потери, слишком маленький увеличивает служебные расходы. Пресет — отправная точка для проверки на вашей сети.","MTU is the largest IP packet on the NetPulse adapter. Too large can cause loss; too small adds overhead. Presets are starting points to test on your network.") }
                        RowLayout { Layout.fillWidth: true
                            Choice { appWindow: win; id: mtuPreset; Layout.fillWidth: true; model: [tr("Сбалансированный · 1500","Balanced · 1500"),tr("Совместимость · 1400","Compatibility · 1400"),tr("Игры — TUN","Gaming — TUN"),tr("Вручную","Custom")]; currentIndex: Math.max(0,["balanced","compatibility","games","custom"].indexOf(prefs.network_preset)); onActivated: {if(currentIndex<2)mtuValue.value=[1500,1400][currentIndex]} }
                            SpinBox { id: mtuValue; from: 1280; to: 9000; value: prefs.mtu||1500; editable: true; enabled: mtuPreset.currentIndex!==2&&!jobTimer.running }
                            ActionButton { appWindow: win; text: tr("Применить","Apply"); enabled: !jobTimer.running&&!savingPreferences; onClicked: {
                                applyNetworkPreset(["balanced","compatibility","games","custom"][mtuPreset.currentIndex],mtuValue.value)
                            } }
                        }
                        HelpText { text: mtuPreset.currentIndex===2
                            ? tr("Игровой пресет включает TUN и защищённый DNS Cloudflare (1.1.1.1) через VPN. Сохраняет ваш MTU, System Proxy и правила маршрутизации. Для игры нужен сервер с поддержкой UDP; после применения перезапустите игру и её загрузчик.","Gaming enables TUN and Cloudflare secure DNS (1.1.1.1) through VPN. Your MTU, System Proxy and routing rules are preserved. Games need a server with UDP support; restart the game and launcher after applying.")
                            : mtuPreset.currentIndex===0 ? tr("MTU 1500 и DoH Cloudflare через VPN. Доступность зависит от провайдера и сервера.","MTU 1500 and Cloudflare DoH through VPN. Compatibility depends on your network and server.")
                            : mtuPreset.currentIndex===1 ? tr("MTU 1400 и DoH Cloudflare напрямую. Более низкий MTU может помочь при потерях; это не обещание роста скорости.","MTU 1400 and direct Cloudflare DoH. Lower MTU may help with loss; a speed improvement is not guaranteed.")
                            : tr("Применяется только указанный MTU; DNS и режимы подключения сохраняются.","Applies the entered MTU while preserving DNS and connection modes.") }
                        Rectangle { Layout.fillWidth: true; height: 1; color: borderColor }
                        Label { text: tr("Сравнение MTU по задержке и потерям","Compare MTU by latency and loss"); color: textColor; font.pixelSize: 15; font.bold: true }
                        HelpText { text: tr("Тест перебирает каждое значение с шагом 1 и выполняет до 5 проверок на MTU. Сначала и в конце проверяется исходный MTU; лучший кандидат проверяется повторно. При нестабильной сети рекомендация не выдаётся. Изменения временные, после теста возвращается исходный MTU.","Tests every value in steps of 1 with up to 5 probes per MTU. The original MTU is checked before and after the scan, and the leading candidate is repeated. Unstable results produce no recommendation. Changes are temporary; the original MTU is restored afterward.") }
                        Label { text: tr("Метод и узел проверки","Probe method and target"); color: mutedColor; font.pixelSize: 12 }
                        RowLayout { Layout.fillWidth: true
                            Choice { appWindow: win; id: tuningMethod; values: ["https","icmp"]; model: ["HTTPS","ICMP"]; implicitWidth: 120; onActivated: tuningTarget.text=selectedValue==="icmp"?"1.1.1.1":"https://www.gstatic.com/generate_204" }
                            Input { id: tuningTarget; Layout.fillWidth: true; text: "https://www.gstatic.com/generate_204"; placeholderText: tuningMethod.selectedValue==="icmp"?"1.1.1.1 / google.com":"https://example.com/" }
                        }
                        HelpText { text: tuningMethod.selectedValue==="https" ? tr("HTTPS проверяет реальные запросы через TUN. Сравнивает задержку, потери и джиттер, но не измеряет скорость скачивания. Для скорости используйте Speed Test после применения выбранного MTU.","HTTPS tests real requests through TUN. It compares latency, loss and jitter, but does not measure download speed. Use Speed Test after applying an MTU to compare throughput.") : tr("ICMP может не проходить через VLESS, Trojan и Hysteria2. Если узел не отвечает через TUN, тест остановится без изменения MTU. Для этих протоколов используйте HTTPS.","ICMP may not pass through VLESS, Trojan or Hysteria2. If the target does not reply through TUN, the test stops before changing MTU. Use HTTPS for these protocols.") }
                        RowLayout { Layout.fillWidth: true
                            Label { text: tr("От","From"); color: mutedColor }
                            SpinBox { id: tuningMinimum; from: 1280; to: 1500; value: 1440; editable: true }
                            Label { text: tr("До","To"); color: mutedColor }
                            SpinBox { id: tuningMaximum; from: tuningMinimum.value; to: Math.min(1500,tuningMinimum.value+64); value: 1472; editable: true }
                            Label { text: tr("Проверок","Probes"); color: mutedColor }
                            SpinBox { id: tuningSamples; from: 1; to: 5; value: 5; editable: true }
                        }
                        RowLayout { Layout.fillWidth: true
                            ActionButton { appWindow: win; text: tr("Начать сравнение MTU","Start MTU comparison"); primary: true; enabled: !!snapshot.tun_active&&!jobTimer.running&&!savingPreferences; onClicked: beginJob("start_mtu_tuning",{target:tuningTarget.text.trim(),method:tuningMethod.selectedValue,minimum:tuningMinimum.value,maximum:tuningMaximum.value,samples:tuningSamples.value}) }
                            ActionButton { appWindow: win; text: tr("Отменить","Cancel"); visible: !!job&&job.kind==="mtu_tuning"&&job.running; enabled: !busy("cancel_diagnostic"); onClicked: call("cancel_diagnostic",{id:diagnosticId}) }
                            Label { visible: !!job&&job.kind==="mtu_tuning"; text: job?job.completed+" / "+job.total:""; color: mutedColor }
                        }
                        HelpText { visible: !snapshot.tun_active; text: tr("Подключитесь к серверу в режиме TUN. Тест может прерывать соединения приложений и занять несколько минут.","Connect to your server in TUN mode. This test can interrupt application connections and take several minutes.") }
                        ProgressBar { visible: !!job&&job.kind==="mtu_tuning"&&job.running; Layout.fillWidth: true; from: 0; to: job?Math.max(1,job.total):1; value: job?job.completed:0 }
                        Repeater { model: job&&job.kind==="mtu_tuning"?job.results:[]
                            delegate: ColumnLayout { required property var modelData; Layout.fillWidth: true
                                Label { textFormat: Text.PlainText; visible: modelData.mtu!==undefined; text: "MTU "+modelData.mtu+" · "+(modelData.phase==="baseline"?tr("исходный","baseline"):modelData.phase==="control"?tr("контроль","control"):modelData.phase==="verify"?tr("повтор","repeat"):tr("кандидат","candidate"))+" · "+(modelData.median_ms===null||modelData.median_ms===undefined?"—":Number(modelData.median_ms).toFixed(1)+" ms")+" · "+tr("потери ","loss ")+(modelData.loss_percent===undefined?"—":Number(modelData.loss_percent).toFixed(0)+"%")+" · "+tr("джиттер ","jitter ")+(modelData.jitter_ms===null||modelData.jitter_ms===undefined?"—":Number(modelData.jitter_ms).toFixed(1)+" ms"); color: textColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
                                HelpText { visible: !!modelData.error; text: modelData.error||"" }
                                HelpText { visible: !!modelData.comparison; text: modelData.comparison?(modelData.comparison.suggested_mtu?tr("Рекомендация этого теста: MTU ","Recommendation from this test: MTU ")+modelData.comparison.suggested_mtu:tr("Убедительного улучшения нет. Сохраните исходный MTU.","No reliable improvement. Keep the original MTU.")):"" }
                                HelpText { visible: !!modelData.comparison; text: modelData.comparison?tuningReason(modelData.comparison.reason):"" }
                                ActionButton { appWindow: win; visible: !!modelData.comparison&&!!modelData.comparison.suggested_mtu; text: tr("Применить рекомендацию","Apply recommendation"); enabled: !jobTimer.running&&!savingPreferences&&!job.error; onClicked: {mtuValue.value=modelData.comparison.suggested_mtu;mtuPreset.currentIndex=3;save({mtu:mtuValue.value,network_preset:"custom"})} }
                            }
                        }
                        HelpText { visible: !!job&&job.kind==="mtu_tuning"&&!!job.error; text: job&&job.error?job.error.message:"" }
                    }
                }

                Card { visible: networkView.tab===3; Layout.fillWidth: true; implicitHeight: tlsForm.implicitHeight+40
                    ColumnLayout { id: tlsForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Component.onCompleted: {var t=prefs.tls_defaults||{};sniMode.currentIndex=Math.max(0,sniMode.values.indexOf(t.sni_mode||"inherit"));sniName.text=t.server_name||"";httpHost.text=t.http_host||"";fingerprint.currentIndex=Math.max(0,fingerprint.values.indexOf(t.fingerprint||"profile"))}
                        Label { textFormat: Text.PlainText; text: "SNI / TLS"; color: textColor; font.pixelSize: 16; font.bold: true }
                        Label { textFormat: Text.PlainText; text: tr("TLS сохраняет проверку сертификата. Reality и транспорт проверяются перед применением.","TLS certificate validation remains enabled. Reality and transport compatibility are checked before applying."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                        Label { textFormat: Text.PlainText; text: tr("SNI — режим передачи имени сервера","SNI — server name mode"); color: textColor; font.bold: true }
                        Choice { appWindow: win; id: sniMode; Layout.fillWidth: true; values: ["inherit","auto","custom","disabled"]; model: [tr("Как в профиле","Use profile settings"),tr("Автоматически по адресу сервера","Use server address"),tr("Своё имя SNI","Custom SNI name"),tr("Не передавать SNI","Do not send SNI")] }
                        HelpText { text: tr("SNI сообщает серверу имя сайта при установке TLS. Обычно оставляйте значение из профиля. Своё имя должно подходить сертификату сервера; Reality требует SNI.","SNI sends a hostname when TLS starts. Usually keep the profile value. A custom name must match the server certificate; Reality requires SNI.") }
                        Label { textFormat: Text.PlainText; visible: sniMode.selectedValue==="custom"||sniMode.selectedValue==="disabled"; text: tr("Имя сервера для SNI / проверки сертификата","Server name for SNI / certificate validation"); color: mutedColor; font.pixelSize: 12 }
                        Input { id: sniName; visible: sniMode.selectedValue==="custom"||sniMode.selectedValue==="disabled"; Layout.fillWidth: true; placeholderText: "Server name / SNI" }
                        Label { textFormat: Text.PlainText; text: tr("HTTP Host — имя узла HTTP (необязательно)","HTTP Host — HTTP hostname (optional)"); color: textColor; font.bold: true }
                        Input { id: httpHost; Layout.fillWidth: true; placeholderText: tr("HTTP Host (необязательно)","HTTP Host (optional)") }
                        HelpText { text: tr("Отдельный заголовок для WebSocket, XHTTP и HTTPUpgrade. Это не SNI. Оставьте пустым, чтобы использовать профиль; неверное имя может нарушить подключение.","A separate header for WebSocket, XHTTP and HTTPUpgrade. It is distinct from SNI. Leave empty to use the profile; an incorrect hostname can break the connection.") }
                        Label { textFormat: Text.PlainText; text: tr("TLS — отпечаток ClientHello (uTLS)","TLS — ClientHello fingerprint (uTLS)"); color: textColor; font.bold: true }
                        Choice { appWindow: win; id: fingerprint; Layout.fillWidth: true; values: ["profile","disabled","chrome","firefox","edge","safari","ios","android","randomized"]; model: [tr("Как в профиле","Use profile settings"),tr("Без uTLS","Disable uTLS"),"Chrome","Firefox","Edge","Safari","iOS","Android",tr("Случайный","Randomized")] }
                        HelpText { text: tr("Отпечаток меняет вид TLS-рукопожатия под выбранный браузер. Проверка сертификата остаётся включённой. QUIC/Hysteria2 не используют этот параметр; Reality требует совместимый uTLS.","The fingerprint shapes the TLS handshake like the selected browser. Certificate verification stays enabled. This setting does not apply to QUIC/Hysteria2; Reality requires compatible uTLS.") }
                        ActionButton { appWindow: win; text: tr("Сохранить SNI и TLS","Save SNI and TLS"); onClicked: save({tls_defaults:sniMode.selectedValue==="inherit"&&!httpHost.text&&fingerprint.currentIndex===0?null:{sni_mode:sniMode.selectedValue,server_name:sniName.text.trim(),http_host:httpHost.text.trim()||null,fingerprint:fingerprint.currentIndex?fingerprint.selectedValue:null}}) }
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
                            Choice { appWindow: win; id: muxUdp; model: ["skip","reject","allow"]; Layout.fillWidth: true }
                        }
                        HelpText { text: tr("Concurrency — число TCP-потоков в канале; XUDP — UDP-потоков. UDP 443: skip — без мультиплексирования, reject — блокировать, allow — разрешить. Изменяйте эти параметры под возможности сервера.","Concurrency is TCP streams per channel; XUDP controls UDP streams. UDP 443: skip bypasses multiplexing, reject blocks it, allow permits it. Match these values to server capabilities.") }
                        ActionButton { appWindow: win; text: tr("Сохранить транспорт","Save transport"); onClicked: save({fragmentation:{enabled:fragToggle.checked,packets:fragPackets.text,length:fragLength.text,interval:fragInterval.text,max_split:fragSplit.text},multiplex:{enabled:muxToggle.checked,concurrency:muxConcurrency.value,xudp_concurrency:muxXudp.value,udp_443:muxUdp.currentText}}) }
                    }
                }
                Card { visible: networkView.tab===7; Layout.fillWidth: true; implicitHeight: sniffForm.implicitHeight+40
                    ColumnLayout { id: sniffForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                        Component.onCompleted: {var f=prefs.sniffing||{};sniffEnabled.checked=f.enabled!==false;sniffHttp.checked=f.http!==false;sniffTls.checked=f.tls!==false;sniffQuic.checked=f.quic!==false;sniffTimeout.value=f.timeout_ms||300}
                        Label { text: "Sniffing"; color: textColor; font.pixelSize: 16; font.bold: true }
                        SettingRow { id: sniffEnabled; Layout.fillWidth: true; title: tr("Определять домен в TUN","Inspect TUN hostnames"); description: tr("Ядро читает начало TUN-соединения для правил доменов, без расшифровки TLS. System Proxy использует имя из запроса приложения и не ждёт первых данных. При отключении Sniffing правила зависят от имени, известного из DNS.","The engine inspects initial TUN metadata for domain rules without decrypting TLS. System Proxy uses the application request hostname and does not wait for initial data. When Sniffing is off, domain rules depend on hostnames known from DNS.") }
                        SettingRow { id: sniffHttp; Layout.fillWidth: true; enabled: sniffEnabled.checked; title: "HTTP"; description: tr("Определять домен по заголовку Host HTTP.","Read the HTTP Host header.") }
                        SettingRow { id: sniffTls; Layout.fillWidth: true; enabled: sniffEnabled.checked; title: "TLS"; description: tr("Определять домен из SNI в начале TLS-соединения. При ECH имя может быть недоступно.","Read SNI from the TLS handshake. ECH can hide this hostname.") }
                        SettingRow { id: sniffQuic; Layout.fillWidth: true; enabled: sniffEnabled.checked; title: "QUIC"; description: tr("Определять домен из начальных пакетов QUIC, например HTTP/3.","Inspect the initial QUIC packets, for example HTTP/3.") }
                        RowLayout { Layout.fillWidth: true
                            Label { text: tr("Ожидание данных, мс","Metadata timeout, ms"); color: mutedColor; Layout.fillWidth: true }
                            SpinBox { id: sniffTimeout; from: 10; to: 1000; value: 300; editable: true; enabled: sniffEnabled.checked }
                        }
                        HelpText { text: tr("Меньшее время сокращает ожидание, но ядро может не успеть определить домен. Исключения для приложений задаются в Маршрутизации.","A shorter timeout reduces waiting but may miss the hostname. Application exceptions are configured in Routing.") }
                        ActionButton { appWindow: win; text: tr("Сохранить Sniffing","Save Sniffing"); enabled: !savingPreferences&&!jobTimer.running&&!busy("quick_network")&&!busy("select_profile")&&!busy("connect"); onClicked: save({sniffing:{enabled:sniffEnabled.checked,http:sniffHttp.checked,tls:sniffTls.checked,quic:sniffQuic.checked,timeout_ms:sniffTimeout.value}}) }
                    }
                }
                Card { visible: networkView.tab===5; Layout.fillWidth: true; implicitHeight: serviceForm.implicitHeight+40
                    ColumnLayout { id: serviceForm; anchors.fill: parent; anchors.margins: 20; spacing: 12
                        Label { textFormat: Text.PlainText; text: tr("Служба TUN","TUN service"); color: textColor; font.pixelSize: 16; font.bold: true }
                        Label { textFormat: Text.PlainText; text: modelData.service?(modelData.service.error?tr("Требует внимания","Needs attention"):modelData.service.installed?tr("Установлена","Installed"):tr("Не установлена","Not installed"))+(modelData.service.running?tr(" · работает"," · running"):""):"—"; color: mutedColor }
                        HelpText { visible:!!(modelData.service&&modelData.service.error); text:modelData.service&&modelData.service.error?modelData.service.error.message:"" }
                        HelpText { text: tr("Установка требует подтверждения Windows. Обновление меняет только отличающиеся файлы после остановки службы. Если антивирус удалил компоненты, операция прерывается: сначала проверьте его отчёт и результат проверки выпуска.","Installation requires Windows confirmation. Updates replace only changed files after the service stops. If antivirus software removes components, maintenance stops: check its report and the release analysis before retrying.") }
                        RowLayout {
                            ActionButton { appWindow: win; text: tr("Установить / восстановить","Install / repair"); enabled: !snapshot.engine_pid&&!busy("manage_service")&&!(modelData.service&&modelData.service.error&&["ServiceRegistrationDamaged","ServiceRestartRequired"].indexOf(modelData.service.error.code)>=0); onClicked: call("manage_service",{install:true,upgrade:!!(modelData.service&&modelData.service.installed)}) }
                            ActionButton { appWindow: win; text: tr("Удалить службу","Remove service"); enabled: !snapshot.engine_pid&&!busy("manage_service")&&!!(modelData.service&&modelData.service.installed)&&!(modelData.service.error&&["ServiceRegistrationDamaged","ServiceRestartRequired"].indexOf(modelData.service.error.code)>=0); onClicked: call("manage_service",{install:false}) }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: diagnosticsPage
        ColumnLayout {
            spacing: 18
            Card {
                Layout.fillWidth: true
                implicitHeight: latencyForm.implicitHeight + 40
                ColumnLayout {
                    id: latencyForm
                    anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { text: tr("Пинг", "Ping"); color: textColor; font.pixelSize: 16; font.bold: true }
                    HelpText { text: tr("Тип проверки сохраняется и используется кнопкой обновления пинга в обзоре, меню сервера и проверкой подписки.", "The chosen method is saved and used by Overview refresh, server actions and subscription tests.") }
                    Label { text: tr("Тип проверки", "Test method"); color: mutedColor; font.pixelSize: 12 }
                    ButtonGroup { id: latencyGroup }
                    ColumnLayout {
                        Layout.fillWidth: true; spacing: 1
                        Repeater {
                            model: [
                                {value:"proxy_https_rtt",label:"via Proxy GET",description:tr("HTTPS-запрос через выбранный профиль: проверяет реальное прохождение запроса, TLS и авторизацию.","HTTPS through the selected profile: checks a real request, TLS and authentication.")},
                                {value:"proxy_https_head_rtt",label:"via Proxy HEAD",description:tr("HTTPS через профиль без загрузки тела ответа. Некоторые сайты не поддерживают HEAD.","HTTPS through the profile without a response body. Some sites do not support HEAD.")},
                                {value:"tcp_connect_rtt",label:"TCP",description:tr("Время TCP-соединения до сервера вне TUN NetPulse. Не проверяет VPN, TLS и UDP; сервер Hysteria2 может не отвечать на TCP.","TCP connection time to the server outside NetPulse TUN. Does not test VPN, TLS or UDP; a Hysteria2 server may not answer TCP.")},
                                {value:"icmp_rtt",label:"ICMP",description:tr("ICMP echo до адреса сервера по текущим маршрутам Windows. Сервер или сеть могут блокировать такие ответы.","ICMP echo to the server over current Windows routes. The server or network may block these replies.")}
                            ]
                            delegate: RadioButton {
                                id: methodRadio
                                required property var modelData
                                Layout.fillWidth: true
                                implicitHeight: 46
                                leftPadding: 14; rightPadding: 14
                                hoverEnabled: true
                                text: modelData.label
                                ButtonGroup.group: latencyGroup
                                checked: (prefs.ping_method || "tcp_connect_rtt") === modelData.value
                                onClicked: save({ping_method:modelData.value})
                                indicator: Rectangle {
                                    x: 16; anchors.verticalCenter: parent.verticalCenter
                                    width: 16; height: 16; radius: 8
                                    color: "transparent"
                                    border.width: methodRadio.checked ? 2 : 1
                                    border.color: methodRadio.checked ? accentColor : mutedColor
                                    Rectangle { anchors.centerIn: parent; width: 8; height: 8; radius: 4; visible: methodRadio.checked; color: accentColor }
                                }
                                contentItem: Text {
                                    text: methodRadio.text
                                    font.pixelSize: 13
                                    color: textColor
                                    leftPadding: 30
                                    verticalAlignment: Text.AlignVCenter
                                }
                                background: Rectangle {
                                    radius: 7
                                    color: methodRadio.checked ? (light ? "#ece8fc" : "#252139") : methodRadio.hovered ? (light ? "#e5e8f3" : "#252a3b") : panelColor
                                    border.color: methodRadio.checked ? accentColor : borderColor
                                }
                                ToolTip.visible: hovered
                                ToolTip.delay: 400
                                ToolTip.text: modelData.description
                                Accessible.description: modelData.description
                            }
                        }
                    }
                    GridLayout {
                        Layout.fillWidth: true; columns: 2; columnSpacing: 20; rowSpacing: 8
                        Label { text: tr("Режим (via Proxy)", "Mode (via Proxy)"); color: mutedColor; font.pixelSize: 12 }
                        Label { text: tr("Тестовый HTTPS URL (via Proxy)", "Test HTTPS URL (via Proxy)"); color: mutedColor; font.pixelSize: 12 }
                        Choice { appWindow: win;
                            id: proxyPingMode
                            Layout.fillWidth: true
                            model: ["double", "keepalive"]
                            currentIndex: prefs.ping_proxy_mode === "double" ? 0 : 1
                            enabled: String(prefs.ping_method || "tcp_connect_rtt").indexOf("proxy_https") === 0
                            onActivated: save({ping_proxy_mode:currentText})
                        }
                        Input {
                            id: pingProbeUrl
                            Layout.fillWidth: true
                            text: prefs.probe_url || "https://www.gstatic.com/generate_204"
                            placeholderText: "https://www.gstatic.com/generate_204"
                            enabled: proxyPingMode.enabled
                            onEditingFinished: if (text !== prefs.probe_url) save({probe_url:text})
                        }
                    }
                    HelpText { text: tr("Показывается средняя задержка успешных запросов. double: два отдельных HTTPS-соединения; keepalive: повторный запрос использует установленное соединение, если сервер его сохраняет. Для проверки выберите короткий ответ, например generate_204.", "The result is the average of successful requests. double uses two separate HTTPS connections; keepalive reuses an established connection if the server keeps it open. Choose a small response such as generate_204.") }
                    HelpText { text: tr("TCP проверяет доступность порта сервера вне TUN, ICMP использует маршруты Windows. Эти методы сами по себе не подтверждают работу VPN. Для проверки запроса через профиль выберите GET или HEAD.", "TCP tests the server port outside TUN; ICMP uses Windows routes. These methods alone do not confirm that the VPN works. Choose GET or HEAD to test a request through the profile.") }
                }
            }
            Card {
                Layout.fillWidth: true
                implicitHeight: speedForm.implicitHeight + 40
                ColumnLayout {
                    id: speedForm
                    anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { text: tr("Тест скорости", "Speed test"); color: textColor; font.pixelSize: 16; font.bold: true }
                    HelpText { text: tr("Последовательная загрузка повторяется до выбранного времени или лимита трафика. Это даёт более длительное измерение, чем загрузка одного файла. На быстром соединении лимит может закончиться раньше.", "Downloads repeat until the selected duration or traffic limit is reached. This measures sustained transfers rather than a single file. A fast connection can reach the limit earlier.") }
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: tr("Сервис измерения", "Measurement service"); color: mutedColor; font.pixelSize: 12; Layout.fillWidth: true }
                        ActionButton { appWindow: win; text: "Cloudflare Speed Test ↗"; onClicked: Qt.openUrlExternally("https://speed.cloudflare.com/") }
                    }
                    Label { text: tr("URL загрузки (HTTPS)", "Download URL (HTTPS)"); color: mutedColor; font.pixelSize: 12 }
                    Input { id: downloadUrl; Layout.fillWidth: true; text: "https://speed.cloudflare.com/__down?bytes=8388608"; placeholderText: "https://speed.cloudflare.com/__down?bytes=8388608" }
                    GridLayout {
                        Layout.fillWidth: true; columns: 3; columnSpacing: 16; rowSpacing: 8
                        Label { text: tr("Маршрут", "Route"); color: mutedColor; font.pixelSize: 12 }
                        Label { text: tr("Время на маршрут, сек", "Time per route, sec"); color: mutedColor; font.pixelSize: 12 }
                        Label { text: tr("Общий лимит, MiB", "Total limit, MiB"); color: mutedColor; font.pixelSize: 12 }
                        Choice { appWindow: win;
                            id: speedMode
                            Layout.fillWidth: true
                            model: [tr("Через VPN", "Via VPN"),tr("Напрямую", "Direct"),tr("Сравнить маршруты", "Compare routes")]
                            values: ["vpn", "direct", "compare"]
                            currentIndex: snapshot.engine_pid ? 0 : 1
                        }
                        SpinBox { id: speedDuration; Layout.fillWidth: true; from: 2; to: 120; value: 30; editable: true }
                        SpinBox { id: speedBudget; Layout.fillWidth: true; from: 1; to: 1024; value: 100; editable: true }
                    }
                    HelpText { text: tr("Сравнение запускает два измерения по очереди: время указано для каждого маршрута, общий лимит делится между ними. Лимит учитывает измеряемую загрузку и отправку без служебных данных. «Напрямую» обходит NetPulse; другой VPN в Windows может влиять на маршрут.", "Comparison tests both routes in sequence: time is per route, and the total limit is split between them. The limit covers measured download and upload payload, excluding protocol overhead. Direct bypasses NetPulse; another Windows VPN can still affect the route.") }
                    SettingRow {
                        id: uploadAllowed
                        Layout.fillWidth: true
                        title: tr("Измерять отправку", "Measure upload")
                        description: tr("Включите, если указанный URL принимает тестовые POST-запросы. Время делится поровну между загрузкой и отправкой; трафик остаётся в общем лимите.", "Enable only for an endpoint that accepts test POST requests. Time is split equally between download and upload, within the same total traffic limit.")
                        checked: false
                    }
                    Label { text: tr("URL отправки (HTTPS)", "Upload URL (HTTPS)"); color: mutedColor; font.pixelSize: 12; visible: uploadAllowed.checked }
                    Input { id: uploadUrl; Layout.fillWidth: true; text: "https://speed.cloudflare.com/__up"; visible: uploadAllowed.checked; enabled: uploadAllowed.checked }
                    ActionButton { appWindow: win;
                        text: tr("Начать измерение", "Start test")
                        primary: true
                        enabled: downloadUrl.text.length > 0 && !jobTimer.running && !busy("start_speed") && (speedMode.selectedValue === "direct" || !!snapshot.engine_pid)
                        onClicked: beginJob("start_speed", {request:{download_url:downloadUrl.text,upload_url:uploadAllowed.checked?uploadUrl.text:null,upload_endpoint_confirmed:uploadAllowed.checked,mode:speedMode.selectedValue,duration_seconds:speedDuration.value,max_payload_mib:speedBudget.value}})
                    }
                    HelpText { visible: speedMode.selectedValue !== "direct" && !snapshot.engine_pid; text: tr("Для VPN и сравнения сначала подключитесь к серверу.", "Connect to a server before testing VPN or comparing routes.") }
                }
            }
            Card {
                Layout.fillWidth: true
                implicitHeight: resultColumn.implicitHeight + 40
                ColumnLayout {
                    id: resultColumn
                    anchors.fill: parent; anchors.margins: 20; spacing: 12
                    readonly property var speedProgress: job && job.kind === "speed" ? job.progress : null
                    function phaseName(value) { return value === "idle" ? tr("Задержка без нагрузки", "Idle latency") : value === "upload" ? tr("Отправка", "Upload") : tr("Загрузка", "Download") }
                    function routeName(value) { return value === "vpn" ? tr("Через VPN", "Via VPN") : value === "direct" ? tr("Напрямую", "Direct") : value }
                    function milliseconds(value) { return value === null || value === undefined ? "—" : Number(value).toFixed(1) + " ms" }
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: tr("Результаты измерений", "Measurement results"); color: textColor; font.pixelSize: 16; font.bold: true; Layout.fillWidth: true }
                        ActionButton { appWindow: win; text: tr("История", "History"); onClicked: { historyDialog.open(); historyDialog.load() } }
                        ActionButton { appWindow: win; visible: jobTimer.running; text: tr("Отменить", "Cancel"); enabled: !busy("cancel_diagnostic"); onClicked: call("cancel_diagnostic", {id:diagnosticId}) }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        BusyIndicator { visible: jobTimer.running; running: visible; implicitHeight: 28; implicitWidth: 28 }
                        Label { text: !job ? tr("Запустите проверку", "Start a test") : (job.running ? tr("Выполняется", "Running") : job.cancelled ? tr("Отменено", "Cancelled") : tr("Завершено", "Completed")) + " · " + job.completed + " / " + job.total; color: mutedColor; Layout.fillWidth: true }
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        visible: !!resultColumn.speedProgress && !!job && job.running
                        spacing: 8
                        Label {
                            Layout.fillWidth: true; color: textColor; font.pixelSize: 13
                            text: resultColumn.speedProgress ? resultColumn.routeName(resultColumn.speedProgress.mode) + " · " + resultColumn.phaseName(resultColumn.speedProgress.direction) + " · " + (resultColumn.speedProgress.elapsed_ms / 1000).toFixed(1) + tr(" сек", " sec") : ""
                        }
                        ProgressBar {
                            Layout.fillWidth: true
                            from: 0; to: resultColumn.speedProgress ? Math.max(1,resultColumn.speedProgress.payload_limit_bytes) : 1
                            value: resultColumn.speedProgress ? resultColumn.speedProgress.download_bytes + resultColumn.speedProgress.upload_bytes : 0
                            indeterminate: resultColumn.speedProgress && resultColumn.speedProgress.direction === "idle"
                        }
                        HelpText { text: resultColumn.speedProgress ? "↓ " + bytes(resultColumn.speedProgress.download_bytes) + "   ↑ " + bytes(resultColumn.speedProgress.upload_bytes) + "   / " + bytes(resultColumn.speedProgress.payload_limit_bytes) + tr(" на текущий маршрут", " on this route") : "" }
                    }
                    Label { textFormat: Text.PlainText; visible: !!job && !!job.error; text: job && job.error ? backend.readableText(job.error.message) : ""; color: light ? "#a32c43" : "#ff9eae"; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    Repeater {
                        model: job ? job.results : []
                        delegate: ColumnLayout {
                            required property var modelData
                            readonly property var value: modelData.Ok || modelData
                            Layout.fillWidth: true; spacing: 7
                            Label { textFormat: Text.PlainText; text: { var p = profiles.find(function(p){return p.id === value.profile_id}); return p ? cleanName(p.name) : resultColumn.routeName(value.mode || job.kind) } color: textColor; font.bold: true }
                            Label { textFormat: Text.PlainText; text: resultText(value); color: textColor; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                            HelpText { visible: value.download_mbps !== undefined; text: "↓ " + bytes(value.download_bytes) + "   ↑ " + bytes(value.upload_bytes) + "   · " + ((value.duration_ms || 0) / 1000).toFixed(1) + tr(" сек", " sec") + (value.payload_limit_reached ? tr(" · достигнут лимит трафика", " · traffic limit reached") : "") }
                            HelpText { visible: value.download_mbps !== undefined; text: tr("Задержка без нагрузки: ", "Idle latency: ") + resultColumn.milliseconds(value.idle_https_ms) + tr(" · под нагрузкой: ", " · loaded: ") + resultColumn.milliseconds(value.loaded_latency_ms) + tr(" · джиттер: ", " · jitter: ") + resultColumn.milliseconds(value.jitter_ms) }
                            Label { textFormat: Text.PlainText; visible: !!value.error || !!modelData.Err; text: value.error ? backend.readableText(value.error.message) : modelData.Err ? backend.readableText(modelData.Err.message) : ""; color: light ? "#a32c43" : "#ff9eae"; Layout.fillWidth: true; wrapMode: Text.WordWrap }
                            HelpText { visible: !!value.note; text: value.note || "" }
                        }
                    }
                    ActionButton { appWindow: win; visible: !!job; text: tr("Подробности / копировать", "Details / copy"); onClicked: editJson(tr("Измерение", "Measurement"), JSON.stringify(job,null,2), "") }
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
                ActionButton { appWindow: win; text: tr("Обновить","Refresh"); onClicked: {processView.refreshProcesses();observe()} }
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
                ActionButton { appWindow: win; text: tr("Все соединения","All connections"); onClicked: {observedProcess=null;observe()} }
            }
            RowLayout { visible: !!observedProcess; Layout.fillWidth: true
                Choice { appWindow: win; id: appRoute; currentIndex: {var name=observedProcess?exe(observedProcess.path):"",rule=(prefs.routing_rules||[]).find(function(r){return r.kind==="process"&&exe(r.value)===name});return rule?Math.max(0,values.indexOf(rule.action)):0} values: ["auto","proxy","direct","reject"]; model: [tr("По общим правилам","Use routing rules"),tr("Через VPN","Through VPN"),tr("Напрямую","Direct"),tr("Блокировать","Block")] }
                ActionButton { appWindow: win; text: tr("Применить маршрут EXE","Apply EXE route"); onClicked: {if(!observedProcess)return;var name=exe(observedProcess.path),rules=(prefs.routing_rules||[]).filter(function(r){return !(r.kind==="process"&&exe(r.value)===name)});if(appRoute.selectedValue!=="auto")rules.push({enabled:true,kind:"process",value:name,action:appRoute.selectedValue});save({routing_rules:rules})} }
                ActionButton { appWindow: win; text: tr("Маршрутизация…","Routing…"); onClicked: {appSearch=exe(observedProcess.path);showAddedApps=false;selectPage(2)} }
                ActionButton { appWindow: win; text: tr("Сеть…","Network…"); onClicked: selectPage(3) }
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
                ActionButton { appWindow: win; text: tr("Копировать","Copy"); onClicked: backend.copyText((flowRecords[processKey(observedProcess)]||[]).join("\n")) }
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

    Component { id: activityPage; ActivityView {
        appWindow: win
        onLoaded: function(source,count){if(smokeMode&&smokePendingCore&&source.indexOf("core:")===0){smokeCoreRows=count;smokePendingCore=false;smokeFrame="core-log"}}
    } }

    Component {
        id: settingsPage
        ColumnLayout {
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: aboutColumn.implicitHeight+40
                ColumnLayout { id: aboutColumn; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("О клиенте","About NetPulse"); color: textColor; font.pixelSize: 17; font.bold: true }
                    Label { textFormat: Text.PlainText; text: "NetPulse 1.0.7 · Windows x64"; color: textColor }
                    Label { textFormat: Text.PlainText; text: "Qt Quick · Rust · sing-box "+(modelData.engine_version||"—")+" · Xray "+(modelData.xray_version||"—"); color: mutedColor; font.pixelSize: 12 }
                    ActionButton { appWindow: win; text: "GitHub · NetPulse ↗"; onClicked: Qt.openUrlExternally("https://github.com/HACKYASHER/NetPulse") }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: updateColumn.implicitHeight+40
                ColumnLayout { id: updateColumn; anchors.fill: parent; anchors.margins: 20; spacing: 12
                    Label { textFormat: Text.PlainText; text: tr("Обновление клиента","Client updates"); color: textColor; font.pixelSize: 16; font.bold: true }
                    Label { textFormat: Text.PlainText; text: tr("Проверка при запуске и каждые 6 часов, включая предварительные выпуски. При наличии обновления кнопка появится слева внизу.","Checks run on startup and every 6 hours, including prereleases. An update button appears at the bottom left when a new version is available."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    Label { textFormat: Text.PlainText; text: updateStatus; visible: text.length>0; color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    ActionButton { appWindow: win; text: tr("Проверить обновления","Check for updates"); enabled: !busy("check_client_update"); onClicked: checkUpdate(true) }
                }
            }
            Card { Layout.fillWidth: true; implicitHeight: appearanceForm.implicitHeight + 40
                ColumnLayout { id: appearanceForm; anchors.fill: parent; anchors.margins: 20; spacing: 18
                    Label { textFormat: Text.PlainText; text: tr("Внешний вид", "Appearance"); color: textColor; font.pixelSize: 16; font.weight: Font.DemiBold }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Тема", "Theme"); color: mutedColor; Layout.fillWidth: true }
                        Choice { appWindow: win; model: ["dark", "light", "system"]; currentIndex: Math.max(0, model.indexOf(prefs.theme)); onActivated: save({theme: currentText}) }
                    }
                    RowLayout { Layout.fillWidth: true
                        Label { textFormat: Text.PlainText; text: tr("Язык", "Language"); color: mutedColor; Layout.fillWidth: true }
                        Choice { appWindow: win; model: ["Русский", "English"]; currentIndex: prefs.language === "en" ? 1 : 0; onActivated: save({language: currentIndex ? "en" : "ru"}) }
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
                    Label { textFormat: Text.PlainText; text: tr("Приложение, ядра, служба и личные данные находятся в выбранной при установке папке NetPulse. У каждой учётной записи Windows свой защищённый профиль в подпапке data. Обновление и удаление сохраняют личные данные.", "The application, engines, service and personal data share the folder selected during installation. Each Windows account has a private profile in the data subfolder. Updates and uninstall preserve personal data."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    Label { textFormat: Text.PlainText; text: tr("Приложение: ", "Application: ")+backend.installationDirectory+"\n"+tr("Личные данные: ", "Personal data: ")+backend.dataDirectory; Layout.fillWidth: true; wrapMode: Text.WrapAnywhere; color: mutedColor; font.pixelSize: 12 }
                    ActionButton { appWindow: win; text: tr("Папка NetPulse", "NetPulse folder"); onClicked: backend.openInstallationDirectory() }
                    RowLayout {
                        ActionButton { appWindow: win; text: tr("Создать копию", "Create backup"); onClicked: call("export_data", {kind: "backup"}) }
                        ActionButton { appWindow: win; text: tr("Восстановить", "Restore"); onClicked: call("restore_data", {backup: true}) }
                        ActionButton { appWindow: win; text: tr("Настройки JSON", "Settings JSON"); onClicked: call("export_data", {kind: "settings"}) }
                        ActionButton { appWindow: win; text: tr("Папка данных", "Data folder"); onClicked: backend.openDataDirectory() }
                    }
                    HelpText { text: tr("Временные архивы ядер очищаются после установки и при следующем запуске. Незавершённые загрузки и установщики уже установленной версии тоже удаляются; предыдущий комплект ядер остаётся для отката.","Temporary engine archives are cleared after installation and on startup. Interrupted downloads and installers for the installed version are also removed; the previous engine bundle remains for rollback.") }
                    ActionButton { appWindow: win; text: tr("Редактор всех параметров…", "Edit all preferences…"); onClicked: editJson(tr("Параметры NetPulse", "NetPulse preferences"), JSON.stringify(prefs, null, 2), "save_preferences") }
                    Label { textFormat: Text.PlainText; text: "NetPulse 1.0.7  ·  C++ / Qt Quick  ·  Rust NetworkCore"; color: mutedColor; font.pixelSize: 11 }
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
            function load() { backend.clearError();var name=engine;call("list_core_releases",{engine:name},function(items){if(engine===name){releases=items;releasePicker.currentIndex=items.length?0:-1}}) }
            spacing: 18
            Card { Layout.fillWidth: true; implicitHeight: coreForm.implicitHeight+40
                ColumnLayout { id: coreForm; anchors.fill: parent; anchors.margins: 20; spacing: 14
                    Label { textFormat: Text.PlainText; text: tr("Ядра и обновления","Engines and updates"); color: textColor; font.pixelSize: 17; font.bold: true }
                    HelpText { text: tr("В комплект входит sing-box 1.14.4-netpulse.1: отдельная сборка NetPulse на основе 1.14.4 с исправленными зависимостями. Xray 26.10.21 — официальный выпуск. Восстановление встроенных ядер возвращает этот комплект.","The bundle includes sing-box 1.14.4-netpulse.1, a separate NetPulse build based on 1.14.4 with patched dependencies. Xray 26.10.21 is the official release. Restoring bundled engines restores this set.") }
                    RowLayout {
                        ActionButton { appWindow: win; text: "Xray · "+(win.modelData.xray_version||"—"); primary: engineView.engine==="xray"; onClicked: {engineView.engine="xray";engineView.releases=[]} }
                        ActionButton { appWindow: win; text: "sing-box · "+(win.modelData.engine_version||"—"); primary: engineView.engine==="sing-box"; onClicked: {engineView.engine="sing-box";engineView.releases=[]} }
                        ActionButton { appWindow: win; text: tr("Проверить выпуски","Check releases"); enabled: !busy("list_core_releases"); onClicked: engineView.load() }
                    }
                    Label { textFormat: Text.PlainText; text: (engineView.engine==="xray"?win.modelData.xray_ready:win.modelData.engine_ready)?tr("● Установленное ядро проверено","● Installed engine verified"):tr("● Ядро требует проверки","● Engine needs attention"); color: light?"#17694f":"#70dbb1" }
                    RowLayout {
                        ActionButton { appWindow: win; text: "GitHub · XTLS/Xray-core ↗"; onClicked: Qt.openUrlExternally("https://github.com/XTLS/Xray-core/releases") }
                        ActionButton { appWindow: win; text: "GitHub · SagerNet/sing-box ↗"; onClicked: Qt.openUrlExternally("https://github.com/SagerNet/sing-box/releases") }
                    }
                    Label { textFormat: Text.PlainText; text: tr("Показаны стабильные и предварительные выпуски. Перед заменой проверяются официальный SHA-256 и конфигурация; предыдущий комплект сохраняется.","Stable and prerelease versions are included. Official SHA-256 and configuration are checked before replacing an engine; the previous bundle is retained."); Layout.fillWidth: true; wrapMode: Text.WordWrap; color: mutedColor; font.pixelSize: 12 }
                    RowLayout {
                        BusyIndicator { visible: busy("list_core_releases")||busy("update_core"); running: visible; implicitHeight: 28 }
                        Label { visible: busy("list_core_releases")||busy("update_core"); text: busy("update_core")?tr("Загрузка и проверка ядра…","Downloading and validating engine…"):tr("Проверка GitHub, до 12 секунд…","Checking GitHub, up to 12 seconds…"); color: mutedColor; font.pixelSize: 12 }
                    }
                    Choice { appWindow: win; id: releasePicker; visible: engineView.releases.length>0; Layout.fillWidth: true; model: engineView.releases.map(function(r){return r.tag+(r.prerelease?" · pre-release":" · stable")}) }
                    Card { visible: !!engineView.release; Layout.fillWidth: true; implicitHeight: releaseDetails.implicitHeight+28
                        ColumnLayout { id: releaseDetails; anchors.fill: parent; anchors.margins: 14; spacing: 12
                            Label { textFormat: Text.PlainText; text: engineView.release?engineView.release.asset:""; color: textColor }
                            HelpText { visible: engineView.engine==="sing-box"&&engineView.release&&engineView.release.tag==="v1.14.4"; text: tr("Официальный sing-box 1.14.4 не содержит дополнительных исправлений сборки NetPulse. Рекомендуется оставить встроенную 1.14.4-netpulse.1.","Official sing-box 1.14.4 does not contain the additional NetPulse fixes. Keeping the bundled 1.14.4-netpulse.1 is recommended.") }
                            Label { textFormat: Text.PlainText; text: "SHA-256: "+(engineView.release?engineView.release.archive_sha256:""); color: mutedColor; font.family: "Consolas"; font.pixelSize: 11; wrapMode: Text.WrapAnywhere; Layout.fillWidth: true }
                            RowLayout {
                                ActionButton { appWindow: win; text: tr("Страница выпуска ↗","Release page ↗"); onClicked: if(engineView.release)call("open_core_page",{url:engineView.release.page}) }
                                ActionButton { appWindow: win; text: tr("Скачать и установить","Download and install"); primary: true; enabled: !snapshot.engine_pid&&!busy("update_core"); onClicked: if(engineView.release){backend.clearError();call("update_core",{release:engineView.release},function(r){toast(tr("Установлено: ","Installed: ")+r.engine+" "+r.installed)})} }
                            }
                        }
                    }
                    RowLayout {
                        ActionButton { appWindow: win; text: tr("Откатить комплект","Roll back bundle"); enabled: !snapshot.engine_pid&&!busy("activate_core_bundle"); onClicked: call("activate_core_bundle",{rollback:true}) }
                        ActionButton { appWindow: win; text: tr("Восстановить встроенные ядра","Restore bundled engines"); enabled: !snapshot.engine_pid&&!busy("activate_core_bundle"); onClicked: call("activate_core_bundle",{rollback:false}) }
                    }
                    Label { textFormat: Text.PlainText; text: tr("Обновление и откат доступны после отключения VPN. Если второго ядра не хватает, выбранная установка добавит отсутствующие компоненты из проверенного встроенного комплекта. Защищённые копии службы обновляются с обычным UAC.","Updates and rollback are available after disconnecting VPN. If the companion engine is missing, the selected installation adds absent components from the verified bundled set. Protected service copies are updated with normal UAC."); color: mutedColor; font.pixelSize: 12; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                }
            }
        }
    }

    Dialog {
        id: modeDialog; objectName: "connectionModeDialog"; anchors.centerIn: parent; width: 700; modal: true
        title: tr("Выберите режим подключения","Choose connection mode")
        function start(tun,proxy) {
            if(!tun&&!proxy)return
            var p=selected();if(!p){selectPage(1);importDialog.open();return}
            function applyAndConnect(){call("quick_network",{tun:tun,system_proxy:proxy},function(){modeDialog.close();call("connect",{id:p.id})})}
            function connectNow(){if(snapshot.engine_pid)call("disconnect",{},applyAndConnect);else applyAndConnect()}
            if(tun&&!(modelData.service&&modelData.service.installed))call("manage_service",{install:true,upgrade:false},connectNow)
            else connectNow()
        }
        property bool draftTun: false
        property bool draftProxy: false
        onOpened: { draftTun=!!prefs.tun;draftProxy=!!prefs.system_proxy }
        ColumnLayout { anchors.fill: parent; spacing: 16
            HelpText { text: tr("Включите TUN, System Proxy или оба режима, затем нажмите «Подключиться».","Enable TUN, System Proxy, or both, then choose Connect.") }
            ConnectionModes { appWindow: win; Layout.fillWidth: true; tun: modeDialog.draftTun; proxy: modeDialog.draftProxy; showActual: false; interactive: !busy("quick_network")&&!busy("manage_service"); onChanged: function(tun,proxy){modeDialog.draftTun=tun;modeDialog.draftProxy=proxy} }
            HelpText { visible: modeDialog.draftTun&&!(modelData.service&&modelData.service.installed); text: tr("Для TUN будет установлена служба: Windows запросит подтверждение UAC.","TUN requires service installation. Windows will ask for UAC confirmation.") }
            RowLayout { Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                ActionButton { appWindow: win; text: tr("Отмена","Cancel"); onClicked: modeDialog.close() }
                ActionButton { appWindow: win; objectName: "confirmConnectionModes"; text: tr("Подключиться","Connect"); primary: true; enabled: (modeDialog.draftTun||modeDialog.draftProxy)&&!savingPreferences&&!busy("quick_network")&&!busy("manage_service")&&!busy("connect"); onClicked: modeDialog.start(modeDialog.draftTun,modeDialog.draftProxy) }
            }
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
                            ActionButton { appWindow: win; text: tr("Копировать","Copy"); enabled: !!ipResult[modelData.key]; onClicked: {backend.copyText(ipResult[modelData.key]);toast(tr("IP скопирован","IP copied"))} }
                        }
                        Label { textFormat: Text.PlainText; text: ipResult[modelData.key+"_error"]?ipResult[modelData.key+"_error"].message:modelData.key==="vpn"&&!snapshot.engine_pid?tr("Подключите VPN для проверки","Connect VPN to test this route"):""; visible: text.length>0; color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
                    }
                }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { appWindow: win; text: tr("Обновить","Refresh"); enabled: !busy("public_ips"); onClicked: call("public_ips",{},function(r){ipResult=r}) }
                Item { Layout.fillWidth: true }
                ActionButton { appWindow: win; text: tr("Закрыть","Close"); onClicked: ipDialog.close() }
            }
        }
    }
    Dialog {
        id: historyDialog; anchors.centerIn: parent; width: Math.min(850,win.width-100); height: win.height-140; modal: true; title: tr("История измерений","Measurement history")
        property var records: []
        function load(){var kind=["latency","speed","mtu","mtu_tuning"][historyKind.currentIndex];call("diagnostic_history",{kind:kind},function(items){records=items||[]})}
        ColumnLayout { anchors.fill: parent; spacing: 14
            Choice { appWindow: win; id: historyKind; Layout.fillWidth: true; model: [tr("Пинг","Latency"),"Speed Test","MTU",tr("Сравнение MTU","MTU comparison")]; onActivated: historyDialog.load() }
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
                ActionButton { appWindow: win; text: tr("Копировать","Copy"); onClicked: backend.copyText(JSON.stringify(historyDialog.records,null,2)) }
                Item { Layout.fillWidth: true }
                ActionButton { appWindow: win; text: tr("Закрыть","Close"); onClicked: historyDialog.close() }
            }
        }
    }
    Dialog {
        id: updateDialog; anchors.centerIn: parent; width: 600; modal: true; title: tr("Обновление NetPulse","NetPulse update")
        ColumnLayout { anchors.fill: parent; spacing: 16
            Label { textFormat: Text.PlainText; text: clientUpdate?tr("Доступна версия ","Available version ")+clientUpdate.tag:updateStatus; color: textColor; wrapMode: Text.WordWrap; Layout.fillWidth: true }
            Label { textFormat: Text.PlainText; text: tr("Установщик проверяется по SHA-256 официального выпуска. После подтверждения Windows защищённый помощник повторно загрузит и проверит его. Установка отключит VPN и завершит клиент.","The installer is checked against the official release SHA-256. After Windows confirmation, the protected helper downloads and verifies it again. Installation disconnects VPN and closes the client."); color: mutedColor; wrapMode: Text.WordWrap; Layout.fillWidth: true; font.pixelSize: 12 }
            BusyIndicator { visible: busy("download_client_update")||busy("install_client_update"); running: visible; implicitHeight: 30 }
            RowLayout { Layout.fillWidth: true
                ActionButton { appWindow: win; text: tr("Страница выпуска ↗","Release page ↗"); enabled: !!clientUpdate; onClicked: if(clientUpdate)Qt.openUrlExternally(clientUpdate.page) }
                Item { Layout.fillWidth: true }
                ActionButton { appWindow: win; text: tr("Закрыть","Close"); onClicked: updateDialog.close() }
                ActionButton { appWindow: win; text: updateDownloaded?tr("Установить","Install"):tr("Скачать","Download"); primary: true; enabled: !!clientUpdate&&!busy("download_client_update")&&!busy("install_client_update"); onClicked: {if(updateDownloaded)call("install_client_update",{release:clientUpdate},function(){backend.quit()});else call("download_client_update",{release:clientUpdate},function(){updateDownloaded=true;toast(tr("Установщик проверен","Installer verified"))})} }
            }
        }
    }
    Dialog {
        id: importDialog; objectName: "addConnectionDialog"; anchors.centerIn: parent; width: 650; modal: true
        title: tr("Добавить подключение", "Add connection"); standardButtons: Dialog.NoButton
        ColumnLayout { anchors.fill: parent; spacing: 14
            Choice { appWindow: win; id: importKind; Layout.fillWidth: true; model: [tr("Подписка HTTPS","HTTPS subscription"),tr("Ключ или конфигурация","Link or configuration")] }
            Input { id: importName; visible: importKind.currentIndex===0; Layout.fillWidth: true; placeholderText: tr("Название подписки","Subscription name") }
            Label { textFormat: Text.PlainText; text: importKind.currentIndex===0?tr("Ссылка хранится в зашифрованном виде","The URL is stored encrypted"):tr("Вставьте ключ или конфигурацию","Paste a connection link or configuration"); color: mutedColor; font.pixelSize: 12 }
            ScrollView { Layout.fillWidth: true; Layout.preferredHeight: 150
                TextArea { id: importText; placeholderText: importKind.currentIndex===0?"https://…":"vless://…  trojan://…  ss://…"; selectByMouse: true; wrapMode: TextEdit.WrapAnywhere; color: textColor; font.pixelSize: 13; background: Rectangle { color: palette.base; border.color: borderColor; radius: 9 } }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { appWindow: win; text: tr("Буфер","Clipboard"); onClicked: importText.text=backend.clipboardText() }
                ActionButton { appWindow: win; text: tr("Файл","File"); onClicked: {call("import_file",{qr:false});importDialog.close()} }
                ActionButton { appWindow: win; text: "QR"; onClicked: {call("import_file",{qr:true});importDialog.close()} }
                Item { Layout.fillWidth: true }
                ActionButton { appWindow: win; text: tr("Отмена","Cancel"); onClicked: {importText.clear();importDialog.close()} }
                ActionButton { appWindow: win; text: tr("Добавить","Add"); primary: true; enabled: importText.text.trim().length>0&&!busy("import_text")&&!busy("add_subscription"); onClicked: {var done=function(){importText.clear();importName.clear();importDialog.close()};if(importKind.currentIndex===0)call("add_subscription",{name:importName.text||tr("Подписка","Subscription"),url:importText.text.trim(),protected:true},done);else call("import_text",{text:importText.text},done)} }
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
                ActionButton { appWindow: win; text: tr("Копировать","Copy"); onClicked: backend.copyText(jsonEditor.text) }
                ActionButton { appWindow: win; text: tr("Закрыть", "Close"); onClicked: { jsonEditor.clear(); jsonDialog.close() } }
                ActionButton { appWindow: win; text: tr("Сохранить", "Save"); primary: true; visible: jsonDialog.saveMethod !== ""; enabled: !busy(jsonDialog.saveMethod); onClicked: {
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
                ActionButton { appWindow: win; text: tr("Отмена", "Cancel"); onClicked: subEdit.close() }
                ActionButton { appWindow: win; text: tr("Сохранить", "Save"); primary: true; onClicked: call("save_subscription", {id: subEdit.sub.id, name: subEditName.text, interval_hours: subEditHours.value, protected: subEditProtect.checked, lock_configuration: subEditLock.checked}, function() { subEdit.close() }) }
            }
        }
    }
}
