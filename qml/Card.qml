import QtQuick
import QtQuick.Controls

Rectangle {
    property var appWindow: ApplicationWindow.window
    color: appWindow ? appWindow.panelColor : "#191c27"
    border.color: appWindow ? appWindow.borderColor : "#2b2e3b"
    radius: 16
}
