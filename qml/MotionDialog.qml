import QtQuick
import QtQuick.Controls
import "Motion.js" as Motion

Dialog {
    id: dialog
    property var appWindow: ApplicationWindow.window
    enter: Transition {
        enabled: Motion.enabled(dialog.appWindow)
        NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 160; easing.type: Easing.OutCubic }
        NumberAnimation { property: "scale"; from: 0.97; to: 1; duration: 160; easing.type: Easing.OutCubic }
    }
    exit: Transition {
        enabled: Motion.enabled(dialog.appWindow)
        NumberAnimation { property: "opacity"; from: 1; to: 0; duration: 110 }
    }
}
