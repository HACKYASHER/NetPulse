import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Button {
    id: control
    property var appWindow: ApplicationWindow.window
    property var profiles: []
    property var selectedProfile: null
    property bool busy: false
    readonly property bool light: !!appWindow && appWindow.light
    signal chosen(string id)
    signal manageRequested()
    implicitWidth: 280
    implicitHeight: 44
    hoverEnabled: true
    enabled: !busy
    padding: 12
    function tr(ru,en) { return appWindow.tr(ru,en) }
    function closePicker() { picker.close() }
    function matches() {
        var query=search.text.trim().toLowerCase()
        return profiles.filter(function(p){return (p.name+" "+p.host+" "+p.protocol).toLowerCase().indexOf(query)>=0})
    }
    Accessible.name: tr("Выбрать сервер", "Choose server")+": "+(selectedProfile?appWindow.cleanName(selectedProfile.name):tr("Не выбран", "None selected"))
    contentItem: Item {
        implicitHeight: 20
        Canvas {
            width: 19; height: 20
            anchors.left: parent.left; anchors.verticalCenter: parent.verticalCenter
            property color stroke: control.appWindow.textColor
            onStrokeChanged: requestPaint()
            onPaint: {
                var c=getContext("2d");c.reset();c.strokeStyle=stroke;c.lineWidth=1.3
                c.strokeRect(1,2,17,6);c.strokeRect(1,12,17,6)
                c.fillStyle=stroke;c.fillRect(4,4,2,2);c.fillRect(4,14,2,2)
            }
        }
        RowLayout {
            objectName: "serverPickerNameGroup"
            anchors.centerIn: parent
            width: Math.min(implicitWidth,Math.max(0,parent.width-64)); height: 20; spacing: 6
            Flag { visible: !!control.selectedProfile; name: control.selectedProfile?control.selectedProfile.name:""; Layout.preferredWidth: 22; Layout.preferredHeight: 15; Layout.alignment: Qt.AlignVCenter }
            Label {
                objectName: "serverPickerName"
                textFormat: Text.PlainText
                text: control.selectedProfile?control.appWindow.cleanName(control.selectedProfile.name):control.tr("Выбрать сервер","Choose server")
                color: control.appWindow.textColor; font.bold: true; font.pixelSize: 12
                elide: Text.ElideRight; verticalAlignment: Text.AlignVCenter
                Layout.fillWidth: true; Layout.preferredHeight: 20; Layout.alignment: Qt.AlignVCenter
            }
        }
        Label { anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; width: 12; height: 20; text: "›"; color: control.appWindow.textColor; font.pixelSize: 24; verticalAlignment: Text.AlignVCenter; horizontalAlignment: Text.AlignHCenter }
    }
    background: Rectangle {
        radius: 8
        color: control.down ? (control.light?"#d7cef5":"#39334e") : control.hovered ? (control.light?"#e4ddfa":"#302b43") : (control.light?"#f0f1f8":"#1e2030")
        border.color: control.hovered||control.activeFocus ? "#927fff" : (control.light?"#c9c2e1":"#51456d")
    }
    onClicked: { search.clear();picker.open();search.forceActiveFocus() }
    Dialog {
        id: picker
        objectName: "quickServerDialog"
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: Math.min(650,parent.width-48)
        height: Math.min(570,parent.height-80)
        modal: true
        title: control.tr("Выбор сервера","Choose server")
        palette.window: control.appWindow.panelColor
        palette.windowText: control.appWindow.textColor
        palette.text: control.appWindow.textColor
        palette.base: control.appWindow.bgColor
        background: Rectangle { color: control.appWindow.panelColor; border.color: control.appWindow.borderColor; radius: 12 }
        contentItem: ColumnLayout {
            spacing: 12
            Input { id: search; objectName: "quickServerSearch"; Layout.fillWidth: true; placeholderText: control.tr("Поиск по названию, адресу или протоколу…","Search name, address or protocol…"); onAccepted: if(list.count===1&&!control.busy)control.chosen(list.model[0].id) }
            ListView {
                id: list; objectName: "quickServerList"
                Layout.fillWidth: true; Layout.fillHeight: true
                clip: true; model: control.matches(); spacing: 3
                ScrollBar.vertical: ScrollBar {}
                delegate: ItemDelegate {
                    id: option
                    required property var modelData
                    width: list.width; height: 60
                    enabled: !control.busy
                    hoverEnabled: true
                    highlighted: !!control.selectedProfile&&control.selectedProfile.id===modelData.id
                    contentItem: RowLayout {
                        spacing: 12
                        Flag { name: option.modelData.name; Layout.preferredWidth: 24; Layout.preferredHeight: 17 }
                        ColumnLayout { Layout.fillWidth: true; spacing: 4
                            Label { textFormat: Text.PlainText; text: control.appWindow.cleanName(option.modelData.name); color: control.appWindow.textColor; font.bold: true; Layout.fillWidth: true; elide: Text.ElideRight }
                            Label { textFormat: Text.PlainText; text: option.modelData.protocol.toUpperCase()+" · "+option.modelData.host+":"+option.modelData.port; color: control.appWindow.mutedColor; font.pixelSize: 11; Layout.fillWidth: true; elide: Text.ElideMiddle }
                        }
                        Label { text: option.highlighted?"✓":option.modelData.latency_ms===null||option.modelData.latency_ms===undefined?"—":Math.round(option.modelData.latency_ms)+" ms"; color: control.appWindow.accentColor }
                    }
                    background: Rectangle { radius: 8; color: option.highlighted?(control.light?"#e8dffb":"#2b2542"):option.hovered?(control.light?"#eee9fa":"#303046"):"transparent" }
                    onClicked: control.chosen(modelData.id)
                }
                Label { anchors.centerIn: parent; visible: list.count===0; text: control.tr("Серверы не найдены","No matching servers"); color: control.appWindow.mutedColor }
            }
            RowLayout { Layout.fillWidth: true
                ActionButton { appWindow: control.appWindow; text: control.tr("Открыть серверы","Manage servers"); onClicked: { picker.close();control.manageRequested() } }
                Item { Layout.fillWidth: true }
                ActionButton { appWindow: control.appWindow; text: control.tr("Закрыть","Close"); onClicked: picker.close() }
            }
        }
    }
}
