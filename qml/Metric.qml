import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Card {
    id: metric
    property string label: ""
    property string value: "—"
    property string hint: ""
    property string symbol: ""
    property color tint: "#8775ff"
    implicitHeight: 158
    ColumnLayout {
        anchors.fill: parent; anchors.margins: 20; spacing: 12
        RowLayout {
            Layout.fillWidth: true; spacing: 10
            Rectangle {
                width: 32; height: 32; radius: 10; color: Qt.rgba(metric.tint.r, metric.tint.g, metric.tint.b, 0.14)
                Label { anchors.centerIn: parent; text: metric.symbol; color: metric.tint; font.pixelSize: 22 }
            }
            Label { text: metric.label; color: ApplicationWindow.window.mutedColor; font.pixelSize: 12; Layout.fillWidth: true; elide: Text.ElideRight }
        }
        Label { textFormat: Text.PlainText; text: metric.value; color: ApplicationWindow.window.textColor; font.pixelSize: 27; font.weight: Font.DemiBold }
        Label { textFormat: Text.PlainText; text: metric.hint; color: ApplicationWindow.window.mutedColor; font.pixelSize: 11; Layout.fillWidth: true; elide: Text.ElideRight }
    }
}
