import QtQuick
import "CountryFlags.js" as Countries

Item {
    id: flag
    property string name: ""
    property string code: ""
    readonly property string resolvedCode: Countries.codeFor(code) || Countries.countryForName(name)
    implicitWidth: 27
    implicitHeight: 18

    Image {
        objectName: "countryFlagImage"
        anchors.fill: parent
        source: flag.resolvedCode
            ? "../resources/flags/" + flag.resolvedCode.toLowerCase() + ".svg"
            : "data:image/svg+xml;utf8," + encodeURIComponent('<svg xmlns="http://www.w3.org/2000/svg" width="30" height="20"><rect width="30" height="20" rx="3" fill="#536079"/><circle cx="15" cy="10" r="7" fill="none" stroke="white"/><path d="M8 10h14M15 3v14" stroke="white"/></svg>')
        fillMode: Image.PreserveAspectFit
    }
}
