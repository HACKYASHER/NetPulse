import QtQuick
Item {
 id: a
 property string path: ""
 implicitWidth: 26; implicitHeight: 26
 Rectangle { anchors.fill: parent; radius: 6; color: "#424b65"; visible: icon.status!==Image.Ready; Text { anchors.centerIn: parent; text: "▣"; color: "white" } }
 Image { id: icon; anchors.fill: parent; source: /^(?:[a-zA-Z]:[\\\/]|\\\\)/.test(a.path) ? "image://apps/"+encodeURIComponent(a.path) : ""; sourceSize: Qt.size(32,32); fillMode: Image.PreserveAspectFit }
}
