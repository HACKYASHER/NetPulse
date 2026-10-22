pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Item {
    id: control
    property var appWindow: ApplicationWindow.window
    implicitHeight: Math.min(520, Math.max(1, applications.count) * 58)

    ListView {
        id: applications
        objectName: "routingApplications"
        anchors.fill: parent
        anchors.rightMargin: bar.visible ? bar.width + 5 : 0
        clip: true
        model: control.appWindow.routingApps()
        spacing: 2
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar {
            id: bar
            parent: control
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            policy: ScrollBar.AsNeeded
        }
        // Own the wheel even at the list boundary so the enclosing page stays put.
        WheelHandler {
            target: null
            onWheel: function(event) {
                var delta = event.pixelDelta.y || event.angleDelta.y / 120 * 52
                applications.contentY = Math.max(0, Math.min(Math.max(0, applications.contentHeight - applications.height), applications.contentY - delta))
                event.accepted = true
            }
        }
        delegate: Rectangle {
            id: row
            required property var modelData
            width: applications.width
            height: 56
            radius: 8
            color: control.appWindow.panelColor
            RowLayout {
                anchors.fill: parent
                anchors.margins: 10
                spacing: 12
                CheckBox {
                    checked: (control.appWindow.prefs.applications || []).indexOf(control.appWindow.exe(row.modelData.path)) >= 0
                    onToggled: {
                        var name = control.appWindow.exe(row.modelData.path), list = (control.appWindow.prefs.applications || []).filter(function(item) { return item !== name })
                        if (checked) list.push(name)
                        control.appWindow.save({applications: list})
                    }
                }
                AppIcon { path: row.modelData.path; Layout.preferredWidth: 26; Layout.preferredHeight: 26 }
                Label { textFormat: Text.PlainText; text: row.modelData.name; color: control.appWindow.textColor; Layout.fillWidth: true; elide: Text.ElideRight }
                Label { textFormat: Text.PlainText; text: control.appWindow.exe(row.modelData.path); color: control.appWindow.mutedColor; font.family: "Consolas"; font.pixelSize: 12; Layout.maximumWidth: Math.min(300, applications.width * 0.4); elide: Text.ElideMiddle }
            }
        }
    }
}
