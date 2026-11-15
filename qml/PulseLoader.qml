import QtQuick
import QtQuick.Controls
import "Motion.js" as Motion

Item {
    id: pulse
    property var appWindow: ApplicationWindow.window
    property bool running: true
    readonly property bool animated: running && visible && Motion.enabled(appWindow)
    property real phase: 0
    implicitWidth: 64
    implicitHeight: 28
    Accessible.role: Accessible.ProgressBar
    Accessible.name: appWindow && appWindow.tr ? appWindow.tr("Загрузка", "Loading") : "Loading"
    NumberAnimation on phase { from: 0; to: 2*Math.PI; duration: 1200; loops: Animation.Infinite; running: pulse.animated }
    Row {
        anchors.centerIn: parent; spacing: 5
        Repeater {
            model: 5
            delegate: Rectangle {
                id: bar
                required property int index
                readonly property real wave: (1 + Math.sin(pulse.phase - index * 0.7)) / 2
                width: 6; height: Math.max(8,Math.min(26,pulse.height)); radius: 3
                color: pulse.appWindow ? pulse.appWindow.accentColor : "#8973ed"
                transform: Scale {
                    origin.x: bar.width / 2; origin.y: bar.height / 2
                    yScale: pulse.animated ? 0.35 + 0.65 * bar.wave : 0.45 + (2 - Math.abs(bar.index - 2)) * 0.2
                }
                opacity: pulse.animated ? 0.5 + 0.5 * wave : 0.8
            }
        }
    }
}
