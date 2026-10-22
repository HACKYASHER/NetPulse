import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml" as Client

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
    property color accentColor: "#8775ff"
    property var prefs: ({applications:[]})
    function exe(path) { return path }
    function save(patch) { prefs=Object.assign({},prefs,patch) }
    function routingApps() { var rows=[];for(var i=0;i<50;++i)rows.push({name:"App "+i,path:"app"+i+".exe"});return rows }
    function tr(ru,en) { return ru }
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
    SignalSpy { id: activated; target: choice; signalName: "activated" }
    TestCase {
        name: "ClientInteractions"
        when: windowShown
        function test_01_engine_picker_mouse() {
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
