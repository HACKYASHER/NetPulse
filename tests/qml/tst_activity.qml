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
        requests[id]={method:method,callback:callback}
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
            activity.follow=false
            harness.rejectRequest=false
            harness.requests=({})
            activity.days=["2026-11-04"]
            findChild(activity,"logDay").currentIndex=0
            wait(1)
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
        function test_olderPageStopsFollowingAndKeepsTheCursor() {
            activity.selectCategory(0)
            harness.requests[harness.latest("log_page")].callback({text:"first",next:"owned-cursor"})
            activity.olderPage()
            compare(activity.pageNumber,2)
            compare(activity.follow,false)
            compare(activity.pageCursors[1],"owned-cursor")
            harness.requests[harness.latest("log_page")].callback({text:"older",next:null})
            compare(activity.events[0].message,"older")
            activity.newerPage();compare(activity.pageNumber,1)
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
