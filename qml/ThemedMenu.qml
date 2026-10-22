import QtQuick
import QtQuick.Controls

Menu {
    id: control
    popupType: Popup.Item
    property var appWindow: ApplicationWindow.window
    readonly property bool lightTheme: !!appWindow && appWindow.light
    padding: 5
    implicitWidth: Math.max(230, contentItem.implicitWidth + leftPadding + rightPadding)
    background: Rectangle {
        radius: 10
        color: control.lightTheme ? "#ffffff" : "#191d2a"
        border.color: control.lightTheme ? "#c9c3dc" : "#47405e"
    }
}
