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
        color: control.primary ? "#ffffff" : control.danger ? "#f08393" : control.palette.text
        horizontalAlignment: Text.AlignHCenter; verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        radius: 10
        color: control.primary ? (control.down ? "#6855dc" : control.hovered ? "#9888ff" : "#8775ff")
             : control.hovered ? control.palette.mid : control.palette.button
        border.color: control.primary ? "transparent" : control.palette.mid
        border.width: 1
        Behavior on color { ColorAnimation { duration: 100 } }
    }
    ToolTip.visible: hovered && ToolTip.text.length > 0
    ToolTip.delay: 600
}
