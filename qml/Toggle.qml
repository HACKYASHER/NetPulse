import QtQuick
import QtQuick.Controls
import "Motion.js" as Motion
Switch {
 id: c
 property var appWindow: ApplicationWindow.window
 readonly property bool animate: Motion.enabled(appWindow)
 implicitWidth: 92; implicitHeight: 34
 padding: 0
 indicator: Rectangle {
  width: 52; height: 28; radius: 14; anchors.verticalCenter: parent.verticalCenter
  color: c.checked ? "#725ce6" : c.palette.mid
  border.color: c.checked ? "#9b89ff" : c.palette.placeholderText
  Behavior on color { enabled: c.animate; ColorAnimation { duration: 140 } }
  Rectangle { x: c.checked ? 27 : 3; y: 3; width: 22; height: 22; radius: 11; color: "white"; Behavior on x { enabled: c.animate; NumberAnimation { duration: 160; easing.type: Easing.OutCubic } } }
 }
 contentItem: Text { text: c.checked ? "ON" : "OFF"; color: c.checked ? "#8973ed" : c.palette.placeholderText; font.pixelSize: 11; font.bold: true; leftPadding: 59; verticalAlignment: Text.AlignVCenter }
 Accessible.name: text.length ? text : (checked ? "On" : "Off")
}
