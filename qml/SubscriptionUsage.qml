import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: control
    property var appWindow: ApplicationWindow.window
    property var usage: null
    readonly property real used: usage ? Number(usage.upload||0)+Number(usage.download||0) : 0
    readonly property real total: usage ? Number(usage.total||0) : 0
    readonly property real ratio: total>0 ? Math.max(0,Math.min(1,used/total)) : 0
    readonly property bool expired: !!usage&&!!usage.expire&&Number(usage.expire)*1000<Date.now()
    readonly property string trafficText: !usage ? appWindow.tr("Данные о трафике не переданы","Traffic information not provided") : appWindow.bytes(used)+" / "+(total>0?appWindow.bytes(total):"∞")
    readonly property string expiryText: !usage ? appWindow.tr("Срок действия не передан","Expiry information not provided") : usage.expire ? (expired?appWindow.tr("Истёк: ","Expired: "):appWindow.tr("Действует до: ","Valid until: "))+new Date(usage.expire*1000).toLocaleString() : appWindow.tr("Без срока действия","No expiry")
    spacing: 6
    Rectangle {
        Layout.fillWidth: true; implicitHeight: 19; radius: 5
        color: control.appWindow.light?"#f4f1fc":"#1c1b2c"
        border.color: control.appWindow.light?"#b4a5df":"#79659e"
        Rectangle { x: 1; y: 1; height: parent.height-2; width: (parent.width-2)*control.ratio; radius: 4; color: control.ratio>=1?"#a24d66":control.appWindow.light?"#d1c4f1":"#51416e" }
        Label { anchors.centerIn: parent; text: control.trafficText; color: control.appWindow.textColor; font.pixelSize: 10 }
        Accessible.name: control.appWindow.tr("Использовано трафика: ","Traffic used: ")+control.trafficText
    }
    Label { Layout.fillWidth: true; text: control.expiryText; textFormat: Text.PlainText; wrapMode: Text.Wrap; color: control.expired?(control.appWindow.light?"#a62442":"#ff8e9f"):control.appWindow.mutedColor; font.pixelSize: 10 }
}
