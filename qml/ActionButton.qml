import QtQuick
import QtQuick.Controls

Button {
    id: control
    property bool primary: false
    property bool danger: false
    property var appWindow: ApplicationWindow.window
    implicitHeight: 40
    implicitWidth: Math.max(86, contentItem.implicitWidth + 28)
    leftPadding: 14; rightPadding: 14
    hoverEnabled: true
    font.pixelSize: 13
    font.weight: Font.DemiBold
    opacity: enabled ? 1 : 0.45
    contentItem: Text {
        textFormat: Text.PlainText
        text: control.text; font: control.font
        color: control.primary ? "#ffffff" : control.danger ? (control.appWindow && control.appWindow.light ? "#a52b43" : "#f5a0ae") : (control.appWindow && control.appWindow.light ? "#222536" : "#edf0fa")
        horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        radius: 10
        color: control.primary ? (control.down ? "#4f37b3" : control.hovered ? "#755bd5" : control.appWindow && control.appWindow.light ? "#6047ce" : "#7058d5")
             : control.down ? (control.appWindow && control.appWindow.light ? "#d7cef5" : "#39334e")
             : control.hovered ? (control.appWindow && control.appWindow.light ? "#e4ddfa" : "#302b43") : (control.appWindow && control.appWindow.light ? "#f0f1f8" : "#222635")
        border.color: control.activeFocus || (!control.primary && control.hovered) ? (control.appWindow && control.appWindow.light ? "#8069cf" : "#9a86eb") : control.primary ? "transparent" : (control.appWindow && control.appWindow.light ? "#cdd1df" : "#34394c")
        border.width: 1
    }
    ToolTip.visible: hovered && ToolTip.text.length > 0
    ToolTip.delay: 600
}
