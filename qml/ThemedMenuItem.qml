import QtQuick
import QtQuick.Controls

MenuItem {
    id: control
    property var appWindow: ApplicationWindow.window
    property bool danger: false
    readonly property bool lightTheme: !!appWindow && appWindow.light
    implicitHeight: visible ? 39 : 0
    height: visible ? implicitHeight : 0
    implicitWidth: Math.max(220, label.implicitWidth + 28)
    hoverEnabled: true
    leftPadding: 12
    rightPadding: 12
    opacity: enabled ? 1 : 0.45
    contentItem: Text {
        id: label
        text: control.text
        textFormat: Text.PlainText
        font.pixelSize: 13
        color: control.danger ? (control.lightTheme ? "#a92a45" : "#ffa0b3")
             : control.highlighted || control.hovered ? (control.lightTheme ? "#38266c" : "#f1ecff")
             : control.lightTheme ? "#222536" : "#edf0fa"
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle {
        radius: 6
        color: control.down ? (control.lightTheme ? "#d5c8f8" : "#4a3b6c")
             : control.highlighted || control.hovered ? (control.lightTheme ? "#e8e0fc" : "#392f53") : "transparent"
    }
}
