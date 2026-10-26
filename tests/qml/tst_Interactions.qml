import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml" as Client
import "../../qml/LogText.js" as LogText

Item {
    width: 1000; height: 800
ApplicationWindow {
    id: win
    width: 1000; height: 800; visible: true
    property bool light: false
    property color panelColor: light ? "#ffffff" : "#191c28"
    property color borderColor: light ? "#dfe2ed" : "#2a2e3e"
    property color textColor: light ? "#222536" : "#edf0fa"
    property color mutedColor: light ? "#536078" : "#a7b1c8"
    property color bgColor: light ? "#f5f6fa" : "#10121c"
    property color accentColor: "#8775ff"
    property var prefs: ({applications:[]})
    property int nextRequest: 1
    function busy(method) { return false }
    function logMessage(event) { return event.message }
    function toast(message) {}
    function call(method,params,done) {
        var id=String(nextRequest++)
        Qt.callLater(function(){
            if(!done)return
            if(method==="log_days")done(["2026-10-25"])
            else if(method==="log_read")done(JSON.stringify({id:params.category,level:"INFO",message:params.category+" entry"}))
            else if(method==="logs")done([{id:"events",level:"INFO",message:"events entry"}])
        })
        return id
    }
    QtObject { id: mockBridge; signal completed(string id,string method,var result,var error); function copyText(text){} }
    Loader { id: activityLoader; x:20; y:85; width:900; height:650; z:20; active:false; sourceComponent: Component { Client.ActivityView { appWindow:win;bridge:mockBridge } } }
    function exe(path) { return path }
    function save(patch) { prefs=Object.assign({},prefs,patch) }
    function routingApps() { var rows=[];for(var i=0;i<50;++i)rows.push({name:"App "+i,path:"app"+i+".exe"});return rows }
    function tr(ru,en) { return ru }
    function cleanName(name) { return name }
    function bytes(value) { return String(value)+" B" }
    Client.ServerPicker { id: serverPicker; x: 260; y: 20; appWindow: win; profiles: [{id:"fi",name:"Finland",host:"fi.example",port:443,protocol:"vless"},{id:"de",name:"Germany",host:"de.example",port:443,protocol:"vless"}]; onChosen: function(id){selectedProfile=profiles.find(function(p){return p.id===id});closePicker()} }
    Client.SubscriptionUsage { id: usage; x: 570; y: 20; width: 300; appWindow: win }
    Client.Choice { id: choice; x: 20; y: 20; width: 210; model: ["Авто","sing-box","Xray"]; values: ["auto","sing-box","xray"] }
    Client.ConnectionModes { id: modes; x: 20; y: 100; width: 900; onChanged: function(tun,proxy){modes.tun=tun;modes.proxy=proxy} }
    Client.ConsoleView { id: logView; x: 20; y: 330; width: 800; height: 350; appWindow: win; sourceKey: "test" }
    Flickable { id: outer; x: 840; y: 330; width: 140; height: 300; contentWidth: width; contentHeight: 1000; clip: true
        Client.RoutingApplications { id: routing; appWindow: win; width: 140; height: 250 }
    }
    Client.ThemedMenu { id: themeMenu; parent: win.contentItem; appWindow: win
        Client.ThemedMenuItem { id: themeItem; appWindow: win; text: "Настройки подписки" }
        Client.ThemedMenuItem { id: protectedItem; appWindow: win; text: "Копировать ссылку"; visible: false }
    }
    SignalSpy { id: copied; target: logView; signalName: "copyRequested" }
    SignalSpy { id: activated; target: choice; signalName: "activated" }
    TestCase {
        name: "ClientInteractions"
        when: windowShown
        function test_01_engine_picker_mouse() {
            var arrow=findChild(choice,"choiceIndicator")
            verify(arrow.width>0&&arrow.height>0)
            tryCompare(arrow,"status",Image.Ready)
            verify(Math.abs(arrow.y+arrow.height/2-choice.height/2)<=0.5)
            mouseClick(choice,100,20)
            tryCompare(choice.popup,"visible",true)
            var list=choice.popup.contentItem
            tryVerify(function(){return list.itemAtIndex(2)!==null})
            mouseClick(list.itemAtIndex(2),60,20)
            tryCompare(choice,"selectedValue","xray")
            compare(activated.count,1)
            mouseClick(choice,100,20)
            tryCompare(choice.popup,"visible",true)
            tryVerify(function(){return list.itemAtIndex(1)!==null})
            mouseClick(list.itemAtIndex(1),60,20)
            tryCompare(choice,"selectedValue","sing-box")
            compare(activated.count,2)
            win.requestActivate()
            choice.forceActiveFocus()
            tryCompare(choice,"activeFocus",true)
            choice.popup.close()
        }
        function test_02_both_connection_modes() {
            var tun=findChild(modes,"tunModeSwitch"),proxy=findChild(modes,"proxyModeSwitch")
            verify(tun!==null);verify(proxy!==null)
            var tunCard=findChild(modes,"tunModeCard"),proxyCard=findChild(modes,"proxyModeCard")
            compare(tunCard.width,proxyCard.width);compare(tunCard.height,proxyCard.height)
            compare(tun.mapToItem(modes,0,0).y,proxy.mapToItem(modes,0,0).y)
            modes.width=650;wait(30)
            compare(tunCard.width,proxyCard.width);compare(tunCard.height,proxyCard.height)
            compare(tun.mapToItem(modes,0,0).y,proxy.mapToItem(modes,0,0).y)
            modes.width=900;wait(30)
            mouseClick(tun,20,15);compare(modes.tun,true);compare(modes.proxy,false)
            mouseClick(proxy,20,15);compare(modes.tun,true);compare(modes.proxy,true)
            mouseClick(tun,20,15);compare(modes.tun,false);compare(modes.proxy,true)
        }
        function test_03_console_preserves_scroll_and_selection() {
            var rows=[]
            for(var i=0;i<180;++i)rows.push({key:String(i),level:i%3?"INFO":"ERROR",text:i+" "+"long line ".repeat(40)})
            logView.follow=false;logView.rows=rows
            var viewport=findChild(logView,"logViewport"),text=findChild(logView,"logConsole")
            tryVerify(function(){return viewport.contentHeight>viewport.height*2&&viewport.contentWidth>viewport.width})
            viewport.contentY=200;viewport.contentX=120;text.select(3,20)
            var selected=text.selectedText
            rows=rows.slice();rows.push({key:"180",level:"WARNING",text:"new entry"});logView.rows=rows
            wait(30)
            compare(viewport.contentY,200);compare(viewport.contentX,120);compare(text.selectedText,selected)
            logView.follow=true
            tryVerify(function(){return Math.abs(viewport.contentY-(viewport.contentHeight-viewport.height))<2})
            rows=rows.slice();rows.push({key:"181",level:"INFO",text:"latest"});logView.rows=rows
            wait(30)
            verify(Math.abs(viewport.contentY-(viewport.contentHeight-viewport.height))<2)
            logView.follow=false
            var horizontal=findChild(logView,"logHorizontalScrollBar"),vertical=findChild(logView,"logVerticalScrollBar")
            verify(horizontal.size<1);verify(vertical.size<1)
            viewport.contentX=0
            mouseClick(horizontal,horizontal.width-15,horizontal.height/2)
            tryVerify(function(){return viewport.contentX>0})
            viewport.contentY=viewport.contentHeight-viewport.height
            mousePress(vertical,vertical.width/2,vertical.height-15)
            mouseMove(vertical,vertical.width/2,30,80)
            mouseRelease(vertical,vertical.width/2,30)
            tryVerify(function(){return viewport.contentY<viewport.contentHeight-viewport.height-10})
        }
        function test_06_console_edge_selection_and_plain_copy() {
            var row="select text ".repeat(80)+"LAST COLUMN"
            logView.follow=false;logView.rows=[{key:"copy",level:"INFO",text:row}]
            var viewport=findChild(logView,"logViewport"),text=findChild(logView,"logConsole")
            viewport.contentX=0;viewport.contentY=0
            wait(40)
            mousePress(viewport,15,20)
            mouseMove(viewport,viewport.width+25,20,120)
            tryVerify(function(){return viewport.contentX>40},2000)
            var before=viewport.contentX
            logView.rows=[{key:"copy",level:"INFO",text:row},{key:"append",level:"WARNING",text:"appended while selecting"}]
            wait(150)
            tryVerify(function(){return viewport.contentX>before},2000)
            verify(logView.documentPending)
            mouseRelease(viewport,viewport.width+25,20)
            tryCompare(logView,"documentPending",false)
            verify(text.selectedText.length>60)
            text.selectAll();logView.copySelection()
            compare(copied.signalArguments[copied.count-1][0],row+"\nappended while selecting")
            text.deselect()
            var copiesBefore=copied.count
            text.forceActiveFocus();text.select(0,11);keyClick(Qt.Key_C,Qt.ControlModifier)
            compare(copied.count,copiesBefore+1)
            compare(copied.signalArguments[copied.count-1][0],"select text")
            text.deselect()
            viewport.contentX=300
            mousePress(viewport,viewport.width-40,20)
            mouseMove(viewport,-15,20,100)
            tryVerify(function(){return viewport.contentX<200},2000)
            mouseRelease(viewport,-15,20)
            verify(text.selectedText.length>0)
        }
        function test_07_console_new_source_starts_at_left_edge() {
            var viewport=findChild(logView,"logViewport"),text=findChild(logView,"logConsole")
            logView.follow=false;logView.sourceKey="main";logView.rows=[{key:"main",level:"INFO",text:"wide ".repeat(300)}]
            wait(40);viewport.contentX=800;text.select(2,20)
            logView.sourceKey="core";logView.rows=[{key:"core",level:"INFO",text:"Core connected"}]
            wait(40);compare(viewport.contentX,0);compare(text.selectedText,"")
            verify(text.text.indexOf("Core")>=0)
        }
        function test_07_quick_server_search_and_select() {
            mouseClick(serverPicker,100,20)
            var dialog=findChild(serverPicker,"quickServerDialog"),search=findChild(serverPicker,"quickServerSearch"),list=findChild(serverPicker,"quickServerList")
            tryCompare(dialog,"visible",true)
            search.text="Germany"
            tryCompare(list,"count",1)
            tryVerify(function(){return list.itemAtIndex(0)!==null})
            mouseClick(list.itemAtIndex(0),100,25)
            tryCompare(dialog,"visible",false)
            compare(serverPicker.selectedProfile.id,"de")
            var group=findChild(serverPicker,"serverPickerNameGroup"),name=findChild(serverPicker,"serverPickerName")
            for(var width of [280,200,350]) {
                serverPicker.width=width;wait(20)
                var center=group.mapToItem(serverPicker,group.width/2,group.height/2)
                verify(Math.abs(center.x-serverPicker.width/2)<0.6)
                verify(Math.abs(center.y-serverPicker.height/2)<0.6)
                compare(name.verticalAlignment,Text.AlignVCenter)
            }
            serverPicker.width=280
        }
        function test_10_activity_switches_all_sources_with_follow_paused() {
            activityLoader.active=true
            tryCompare(activityLoader,"status",Loader.Ready)
            var view=activityLoader.item,category=findChild(view,"logCategory"),day=findChild(view,"logDay")
            tryVerify(function(){return view.events.length===1})
            compare(day.currentText,"2026-10-25")
            view.follow=false
            for(var index=0;index<6;index++) {
                category.currentIndex=index;category.activated(index)
                tryVerify(function(){return !view.loading&&view.events.length===1&&view.events[0].id===view.categories[index]})
                verify(findChild(view,"activityConsole").plainText.indexOf(view.categories[index]+" entry")>=0)
            }
            activityLoader.active=false
        }
        function test_09_log_error_hints_keep_unknown_errors_visible() {
            for(var message of ["network is unreachable","lookup disabled.invalid NXDOMAIN","DNS SERVFAIL","local error: tls: bad record MAC","http2: ClientConn.Close","DNS packet unexpected EOF","i/o timeout","address already in use"]) {
                verify(LogText.hint(message,true).length>0)
                verify(LogText.hint(message,false).length>0)
            }
            compare(LogText.hint("unknown engine failure",true),"")
        }
        function test_11_activity_console_fits_available_height() {
            activityLoader.active=true
            tryCompare(activityLoader,"status",Loader.Ready)
            var view=activityLoader.item,console=findChild(view,"activityConsole")
            view.follow=false
            for(var available of [650,500,300,230]) {
                activityLoader.height=available
                tryVerify(function(){return Math.abs(console.y+console.height-view.height)<1})
                verify(console.height>0)
                view.loadError="Ошибка службы: файлы недоступны. ".repeat(10)
                tryVerify(function(){return Math.abs(console.y+console.height-view.height)<1})
                verify(console.height>=0)
                view.loadError=""
            }
            activityLoader.height=650;activityLoader.active=false
        }
        function test_08_subscription_quota_and_expiry() {
            usage.usage={upload:100,download:200,total:1000,expire:1}
            compare(usage.ratio,0.3);compare(usage.expired,true)
            verify(usage.expiryText.indexOf("Истёк")===0)
            usage.usage={download:400,total:0,expire:4000000000}
            compare(usage.ratio,0);verify(usage.trafficText.indexOf("∞")>0)
            verify(usage.expiryText.indexOf("Действует до")===0)
            usage.usage=null
            verify(usage.expiryText.indexOf("не передан")>0)
        }
        function test_04_nested_routing_wheel_stays_in_application_list() {
            var list=findChild(routing,"routingApplications")
            mouseWheel(list,65,120,0,-120)
            tryVerify(function(){return list.contentY>0})
            compare(outer.contentY,0)
            list.contentY=list.contentHeight-list.height
            mouseWheel(list,65,120,0,-120)
            wait(30);compare(outer.contentY,0)
        }
        function test_05_light_menu_hover_and_hidden_actions() {
            win.light=true;themeMenu.popup(250,250)
            tryCompare(themeMenu,"visible",true)
            wait(50)
            mouseMove(themeItem,40,15)
            tryCompare(themeItem,"hovered",true)
            compare(themeItem.lightTheme,true)
            verify(themeItem.background.color!=="transparent")
            compare(protectedItem.height,0)
            themeMenu.close()
        }
        function test_04_light_theme_choices() {
            choice.popup.close()
            wait(30)
            win.light=true
            mouseClick(choice,100,20)
            tryCompare(choice.popup,"visible",true)
            tryVerify(function(){return choice.popup.contentItem.itemAtIndex(2)!==null})
            var option=choice.popup.contentItem.itemAtIndex(2)
            mouseMove(option,30,15)
            tryCompare(option,"hovered",true)
            verify(option.contentItem.color!==option.background.color)
            mouseClick(option,30,15)
            compare(choice.selectedValue,"xray")
        }
    }
}

}
