import QtQuick
import QtQuick.Controls

Card {
    id: metric
    property string label: ""
    property string value: "—"
    property string hint: ""
    implicitHeight: 126
    Column {
        anchors.fill: parent; anchors.margins: 20; spacing: 8
        Label { textFormat: Text.PlainText; text: metric.label; color: ApplicationWindow.window.mutedColor; font.pixelSize: 12 }
        Label { textFormat: Text.PlainText; text: metric.value; color: ApplicationWindow.window.textColor; font.pixelSize: 27; font.weight: Font.DemiBold }
        Label { textFormat: Text.PlainText; text: metric.hint; color: ApplicationWindow.window.mutedColor; font.pixelSize: 11 }
    }
}
