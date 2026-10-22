import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

RowLayout {
    id: modes
    property var appWindow: ApplicationWindow.window
    property bool tun: false
    property bool proxy: false
    property bool tunActive: false
    property bool proxyActive: false
    property bool showActual: true
    property bool interactive: true
    signal changed(bool tun, bool proxy)
    spacing: 14
    Repeater {
        model: ["TUN", "System Proxy"]
        delegate: Card {
            required property int index
            required property string modelData
            objectName: index===0 ? "tunModeCard" : "proxyModeCard"
            Layout.fillWidth: true; Layout.fillHeight: true; Layout.preferredWidth: 1
            implicitHeight: modeContents.implicitHeight + 28
            ColumnLayout {
                id: modeContents
                anchors.left: parent.left; anchors.right: parent.right; anchors.top: parent.top; anchors.margins: 14; spacing: 9
                RowLayout {
                    Layout.fillWidth: true
                    Label { text: modelData; color: modes.appWindow.textColor; font.bold: true; Layout.fillWidth: true }
                    Toggle {
                        objectName: index === 0 ? "tunModeSwitch" : "proxyModeSwitch"
                        checked: index === 0 ? modes.tun : modes.proxy
                        enabled: modes.interactive
                        onToggled: modes.changed(index === 0 ? checked : modes.tun, index === 1 ? checked : modes.proxy)
                    }
                }
                Label {
                    text: index === 0
                        ? modes.appWindow.tr("Перехватывает IP-трафик, включая UDP, и применяет правила приложений. Требуется служба NetPulse.", "Captures IP traffic including UDP and applies application rules. Requires the NetPulse service.")
                        : modes.appWindow.tr("Прокси Windows для приложений, которые его поддерживают. Можно использовать одновременно с TUN.", "Windows proxy for applications that support it. Can be used together with TUN.")
                    Layout.fillWidth: true; wrapMode: Text.WordWrap; font.pixelSize: 12; color: modes.appWindow.mutedColor
                }
                Label {
                    visible: modes.showActual
                    text: modes.appWindow.tr("Сейчас: ", "Currently: ") + ((index === 0 ? modes.tunActive : modes.proxyActive) ? modes.appWindow.tr("включён", "active") : modes.appWindow.tr("выключен", "inactive"))
                    color: (index === 0 ? modes.tunActive : modes.proxyActive) ? (modes.appWindow.light ? "#17694f" : "#70dbb1") : modes.appWindow.mutedColor
                    font.pixelSize: 11
                }
            }
        }
    }
}
