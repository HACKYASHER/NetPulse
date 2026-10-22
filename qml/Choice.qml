pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
ComboBox {
 id: c
 property var appWindow: ApplicationWindow.window
 readonly property bool lightTheme: !!appWindow && appWindow.light
 property var values: []
 readonly property string selectedValue: values.length && currentIndex>=0 ? String(values[currentIndex]) : currentText
 implicitHeight: 42; implicitWidth: 210
 font.pixelSize: 13
 opacity: enabled ? 1 : 0.5
 contentItem: Text { text: c.displayText; font: c.font; color: c.palette.text; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight; leftPadding: 12; rightPadding: 32 }
 indicator: Text { text: "⌄"; color: c.palette.text; x: c.width-26; y: (c.height-height)/2; font.pixelSize: 19 }
 hoverEnabled: true
 background: Rectangle { color: c.hovered ? (c.lightTheme ? "#eeebfa" : "#1e2231") : c.palette.base; border.color: c.activeFocus || c.hovered ? (c.lightTheme ? "#8069cf" : "#9a86eb") : c.palette.mid; radius: 8 }
 delegate: ItemDelegate {
  required property int index
  required property var modelData
  id: option
  width: c.popup.availableWidth; height: 40; hoverEnabled: true
  contentItem: Text { text: String(option.modelData); color: option.highlighted || option.hovered ? (c.lightTheme ? "#38266c" : "#f0ebff") : c.palette.text; font: c.font; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight }
  background: Rectangle { color: option.highlighted || option.hovered ? (c.lightTheme ? "#e5ddfb" : "#3c315d") : c.palette.base; border.color: option.activeFocus ? (c.lightTheme ? "#8069cf" : "#9a86eb") : "transparent"; radius: 5 }
  highlighted: c.highlightedIndex===index
 }
 popup: Popup {
  y: c.height+4; width: c.width; padding: 5
  implicitHeight: Math.min(contentItem.implicitHeight+10,300)
  contentItem: ListView { clip: true; implicitHeight: contentHeight; model: c.popup.visible ? c.delegateModel : null; currentIndex: c.highlightedIndex; ScrollIndicator.vertical: ScrollIndicator {} }
  background: Rectangle { color: c.palette.base; border.color: c.palette.mid; radius: 9 }
 }
}
