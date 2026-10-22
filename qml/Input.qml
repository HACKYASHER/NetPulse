import QtQuick
import QtQuick.Controls

TextField {
    id: control
    implicitHeight: 42
    color: palette.text
    placeholderTextColor: palette.placeholderText
    font.pixelSize: 13
    selectByMouse: true
    leftPadding: 13; rightPadding: 13
    background: Rectangle {
        color: control.palette.base
        border.color: control.activeFocus ? "#8775ff" : control.palette.mid
        radius: 9
    }
}
