import QtQuick
import QtQuick.Controls
ComboBox {
 id: c
 property var values: []
 readonly property string selectedValue: values.length && currentIndex>=0 ? String(values[currentIndex]) : currentText
 implicitHeight: 42; implicitWidth: 210
 font.pixelSize: 13
 opacity: enabled ? 1 : 0.5
 contentItem: Text { text: c.displayText; font: c.font; color: c.palette.text; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight; leftPadding: 12; rightPadding: 32 }
 indicator: Text { text: "⌄"; color: c.palette.text; x: c.width-26; y: (c.height-height)/2; font.pixelSize: 19 }
 background: Rectangle { color: c.palette.base; border.color: c.activeFocus ? "#8775ff" : c.palette.mid; radius: 8 }
 delegate: ItemDelegate {
  required property int index
  required property var modelData
  width: c.width; height: 40
  contentItem: Text { text: String(modelData); color: c.highlightedIndex===index ? "#ffffff" : c.palette.text; font: c.font; verticalAlignment: Text.AlignVCenter; elide: Text.ElideRight }
  background: Rectangle { color: c.highlightedIndex===index ? "#6753d5" : c.palette.base; radius: 5 }
  highlighted: c.highlightedIndex===index
 }
 popup: Popup {
  y: c.height+4; width: c.width; padding: 5
  implicitHeight: Math.min(contentItem.implicitHeight+10,300)
  contentItem: ListView { clip: true; implicitHeight: contentHeight; model: c.popup.visible ? c.delegateModel : null; currentIndex: c.highlightedIndex; ScrollIndicator.vertical: ScrollIndicator {} }
  background: Rectangle { color: c.palette.base; border.color: c.palette.mid; radius: 9 }
 }
}
