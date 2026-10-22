import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
Label {
 textFormat: Text.PlainText
 Layout.fillWidth: true
 wrapMode: Text.WordWrap
 property var appWindow: ApplicationWindow.window
 color: appWindow ? appWindow.mutedColor : "#a7b1c8"
 font.pixelSize: 12
 lineHeight: 1.18
}
