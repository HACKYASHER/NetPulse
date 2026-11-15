import QtQuick
import QtQuick.Controls
import "Motion.js" as Motion

Menu {
    id: control
    popupType: Popup.Item
    property var appWindow: ApplicationWindow.window
    enter: Transition { enabled: Motion.enabled(control.appWindow); NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 145 } }
    exit: Transition { enabled: Motion.enabled(control.appWindow); NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 95 } }
    readonly property bool lightTheme: !!appWindow && appWindow.light
    padding: 5
    implicitWidth: Math.max(230, contentItem.implicitWidth + leftPadding + rightPadding)
    background: Rectangle {
        radius: 10
        color: control.lightTheme ? "#ffffff" : "#191d2a"
        border.color: control.lightTheme ? "#c9c3dc" : "#47405e"
    }
}
