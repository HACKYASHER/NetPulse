import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: row
    property string title: ""
    property string description: ""
    property alias checked: toggle.checked
    signal toggled(bool value)
    spacing: 24
    ColumnLayout {
        Layout.fillWidth: true; spacing: 5
        Label { textFormat: Text.PlainText; text: row.title; font.pixelSize: 14; font.weight: Font.DemiBold; color: ApplicationWindow.window.textColor }
        Label { textFormat: Text.PlainText; text: row.description; Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12; color: ApplicationWindow.window.mutedColor }
    }
    Toggle { id: toggle; onToggled: row.toggled(checked) }
}
