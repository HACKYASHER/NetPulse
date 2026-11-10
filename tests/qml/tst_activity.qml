import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml" as NetPulse

Item {
    id: harness
    width: 1000; height: 720
    property bool light: false
    property color borderColor: "#2a2e3e"
    property color textColor: "#edf0fa"
    property color mutedColor: "#a7b1c8"
    property var prefs: ({logging_level: "INFO"})
    property bool savingPreferences: false
    property bool rejectRequest: false
    property int saved: 0
    property int serial: 0
    property var requests: ({})
    function tr(ru,en) {return en}
    function busy(method) {return false}
    function logMessage(event) {return event.message}
    function toast(text) {}
    function save(patch) {saved++}
    function call(method,params,callback) {
        if (rejectRequest) return ""
        var id=String(++serial)
        requests[id]={method:method,params:params,callback:callback}
        return id
    }
    function latest(method) {
        var ids=Object.keys(requests).filter(function(id){return requests[id].method===method})
        return ids.length ? ids[ids.length-1] : ""
    }
    QtObject {
        id: fakeBridge
        signal completed(string id,string method,var result,var error)
        function copyText(text) {}
        function openLogDirectory() {}
    }
    NetPulse.ActivityView {
        id: activity
        anchors.fill: parent
        appWindow: harness
        bridge: fakeBridge
        follow: false
    }
    TestCase {
        name: "ActivityRequests"
        when: windowShown
        function initTestCase() {wait(30)}
        function init() {
            harness.width=1000
            activity.follow=false
            activity.requestGeneration++
            activity.displayedSource=""
            activity.events=[]
            activity.olderCursor=""
            activity.levelFilter="ALL"
            harness.rejectRequest=false
            harness.requests=({})
            activity.days=["2026-11-04"]
            findChild(activity,"logDay").currentIndex=0
            wait(1)
        }
        function test_controlsFitWithoutHorizontalClipping() {
            for(var width of [720,1000,1400]) {
                harness.width=width
                wait(30)
                for(var name of ["logCategory","logDay","logRefresh","logFolder","runtimeLogLevel","logLatest","logCopy"]) {
                    var control=findChild(activity,name),point=control.mapToItem(activity,0,0)
                    verify(point.x>=-1&&point.x+control.width<=activity.width+1,name+" clipped at "+width)
                }
            }
        }
        function test_eventsErrorStopsLoading() {
            activity.selectCategory(5)
            var id=harness.latest("logs")
            verify(id.length>0)
            verify(activity.loading)
            fakeBridge.completed(id,"logs",null,{code:"FixtureFailure",message:"Read failed"})
            compare(activity.loading,false)
            compare(activity.loadError,"Read failed")
        }
        function test_fileErrorStopsLoadingAndCanRetry() {
            activity.selectCategory(0)
            var id=harness.latest("log_page")
            verify(id.length>0)
            fakeBridge.completed(id,"log_page",null,{code:"FixtureFailure",message:"Read failed"})
            compare(activity.loading,false)
            activity.load()
            compare(activity.loadError,"")
            id=harness.latest("log_page")
            harness.requests[id].callback({text:'{"id":"owned-fixture","level":"INFO","message":"Owned event"}',next:null})
            compare(activity.loading,false)
            compare(activity.events.length,1)
            verify(findChild(activity,"activityConsole").plainText.indexOf("Owned event")>=0)
        }
        function test_filterDoesNotSavePreferencesOrRestart() {
            var saved=harness.saved
            activity.selectCategory(0)
            var choice=findChild(activity,"runtimeLogLevel")
            compare(choice.values[0],"ALL")
            choice.currentIndex=5;choice.activated(5)
            compare(activity.levelFilter,"ERROR")
            compare(harness.saved,saved)
            verify(harness.latest("save_preferences")==="")
            activity.levelFilter="ALL"
        }
        function row(id) {return {id:String(id),level:"INFO",message:"entry "+id}}
        function lines(from,to) {
            var rows=[]
            for(var i=from;i<=to;++i)rows.push(JSON.stringify(row(i)))
            return rows.join("\n")
        }
        function test_historyIsPrependedInOneContinuousJournal() {
            activity.selectCategory(0)
            activity.follow=true
            harness.requests[harness.latest("log_page")].callback({text:lines(50,100),next:"owned-cursor"})
            wait(30)
            var console=findChild(activity,"activityConsole"),viewport=findChild(console,"logViewport"),text=findChild(console,"logConsole")
            activity.follow=false
            viewport.contentY=150
            text.select(3,15)
            var selected=text.selectedText, oldY=viewport.contentY,oldHeight=viewport.contentHeight
            activity.loadOlder()
            compare(activity.follow,false)
            compare(harness.requests[harness.latest("log_page")].params.cursor,"owned-cursor")
            harness.requests[harness.latest("log_page")].callback({text:lines(1,49),next:null})
            wait(30)
            compare(activity.events.length,100)
            compare(activity.events[0].id,"1")
            compare(activity.events[99].id,"100")
            compare(text.selectedText,selected)
            verify(Math.abs(viewport.contentY-(oldY+viewport.contentHeight-oldHeight))<2)
            compare(activity.olderCursor,"")
            compare(console.lineCount,100)
        }
        function test_scrollingToTopLoadsEarlierEntriesWithoutButtons() {
            activity.selectCategory(0)
            activity.follow=true
            harness.requests[harness.latest("log_page")].callback({text:lines(50,100),next:"earlier"})
            wait(30)
            var console=findChild(activity,"activityConsole"),viewport=findChild(console,"logViewport")
            activity.follow=false
            viewport.contentY=0
            tryVerify(function(){return harness.requests[harness.latest("log_page")].params.cursor==="earlier"})
        }
        function test_pollingKeepsHistoryAndCatchesUpAcrossTransportChunks() {
            activity.selectCategory(0)
            activity.follow=true
            harness.requests[harness.latest("log_page")].callback({text:lines(1,51),next:"history"})
            activity.load()
            harness.requests[harness.latest("log_page")].callback({text:lines(60,65),next:"burst"})
            compare(activity.loading,true)
            compare(harness.requests[harness.latest("log_page")].params.cursor,"burst")
            harness.requests[harness.latest("log_page")].callback({text:lines(50,59),next:"older"})
            compare(activity.events.length,65)
            for(var i=0;i<65;++i)compare(activity.events[i].id,String(i+1))
            compare(activity.olderCursor,"history")
            var console=findChild(activity,"activityConsole")
            verify(console.recentStart>0)
            verify(findChild(console,"recentLogHighlight").visible)
            var recent=console.recentStart
            activity.load()
            harness.requests[harness.latest("log_page")].callback({text:lines(60,65),next:"anything"})
            compare(activity.events.length,65)
            compare(console.recentStart,recent)
        }
        function test_obsoleteHistoryCannotReplaceAnotherCategory() {
            activity.selectCategory(0)
            activity.follow=true
            harness.requests[harness.latest("log_page")].callback({text:lines(1,51),next:"earlier"})
            activity.loadOlder()
            var older=harness.latest("log_page")
            activity.selectCategory(1)
            harness.requests[harness.latest("log_page")].callback({text:lines(80,81),next:null})
            harness.requests[older].callback({text:lines(1,2),next:null})
            compare(activity.events.length,2)
            compare(activity.events[0].id,"80")
        }
        function test_rejectedRequestDoesNotLeaveAnInfiniteSpinner() {
            harness.rejectRequest=true
            activity.selectCategory(5)
            compare(activity.loading,false)
            verify(activity.loadError.length>0)
        }
        function test_obsoleteDayDiscoveryCannotReplaceTheNewCategory() {
            activity.discover()
            var id=harness.latest("log_days")
            verify(id.length>0)
            activity.selectCategory(5)
            var generation=activity.requestGeneration
            harness.requests[id].callback(["1999-01-01"])
            wait(1)
            compare(activity.days[0],"2026-11-04")
            compare(activity.requestGeneration,generation)
        }
    }
}
