import QtQuick
import QtQuick.Controls

Rectangle {
    id: control
    property var appWindow: ApplicationWindow.window
    property var rows: []
    property string sourceKey: ""
    property bool follow: true
    property string emptyText: ""
    property int fontSize: 12
    readonly property bool lightTheme: !!appWindow && appWindow.light
    readonly property string plainText: rows.map(function(row) { return row.text }).join("\n")
    readonly property int lineCount: rows.length
    readonly property string selectedText: consoleText.selectedText
    property var renderedRows: []
    property string renderedSource: ""
    property bool initialized: false
    property int renderGeneration: 0
    property int interactionGeneration: 0
    property bool selectionDragging: selectionPointer.active
    property int dragAnchor: 0
    property bool documentPending: false
    signal copyRequested(string text)
    signal readingStarted()
    radius: 10
    color: lightTheme ? "#f9faff" : "#0b0e16"
    border.color: appWindow ? appWindow.borderColor : "#2a2e3e"

    function beginReading() { interactionGeneration++; readingStarted() }
    function html(value) {
        return String(value).replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;")
                .replace(/\t/g, "&nbsp;&nbsp;&nbsp;&nbsp;").replace(/ /g, "&nbsp;").replace(/\n/g, "<br>")
    }
    function severityColor(level) {
        level = String(level || "INFO").toUpperCase()
        if (level === "ERROR" || level === "FATAL") return lightTheme ? "#a62442" : "#ff8e9f"
        if (level === "WARN" || level === "WARNING") return lightTheme ? "#8b5800" : "#e4be76"
        if (level === "DEBUG" || level === "TRACE") return lightTheme ? "#5d667c" : "#909bb4"
        return lightTheme ? "#156143" : "#83dabb"
    }
    function rowHtml(row) {
        return '<p style="margin:0;white-space:pre;color:' + severityColor(row.level) + '">' + html(row.text) + '</p>'
    }
    function rowKey(row) { return String(row.key || "") + "\u0001" + row.level + "\u0001" + row.text }
    function scrollToBottom() {
        viewport.contentY = Math.max(0, viewport.contentHeight - viewport.height)
    }
    function selectAll() { consoleText.forceActiveFocus(); consoleText.selectAll() }
    function copySelection() {
        // QTextDocument uses paragraph separators and NBSP for coloured, unwrapped lines.
        // Put ordinary text on the clipboard, including selections made with Ctrl+C.
        if (selectedText.length) copyRequested(selectedText.replace(/[\u2028\u2029]/g, "\n").replace(/\u00a0/g, " "))
    }
    function dragScroll() {
        var p = viewport.mapFromItem(null, selectionPointer.point.scenePosition.x, selectionPointer.point.scenePosition.y)
        function step(value, extent) {
            if (value < 22) return -Math.min(64, Math.max(6, (22-value)*0.6))
            if (value > extent-22) return Math.min(64, Math.max(6, (value-extent+22)*0.6))
            return 0
        }
        var dx=step(p.x,viewport.width), dy=step(p.y,viewport.height)
        if (!dx && !dy) return
        viewport.contentX=Math.max(0,Math.min(viewport.contentWidth-viewport.width,viewport.contentX+dx))
        viewport.contentY=Math.max(0,Math.min(viewport.contentHeight-viewport.height,viewport.contentY+dy))
        var textPoint=viewport.mapToItem(consoleText,Math.max(0,Math.min(viewport.width-1,p.x)),Math.max(0,Math.min(viewport.height-1,p.y)))
        consoleText.select(dragAnchor,consoleText.positionAt(textPoint.x,textPoint.y))
    }

    function syncDocument(force) {
        if (!initialized) return
        if (selectionDragging && renderedSource === sourceKey) { documentPending=true; return }
        documentPending=false
        var next = rows.slice(), old = renderedRows, sameSource = renderedSource === sourceKey
        var prefix = sameSource && !force && old.length > 0 && old.length <= next.length
        var i, cutRows = 0, overlap = 0
        if (prefix) for (i = 0; i < old.length; ++i) if (rowKey(old[i]) !== rowKey(next[i])) { prefix = false; break }
        if (prefix && old.length === next.length) return

        // Log readers may return a moving tail. Keep the shared suffix instead of replacing the document.
        if (!prefix && sameSource && !force && old.length && next.length) {
            for (i = 1; i < old.length; ++i) {
                var count = old.length - i
                if (count > next.length || rowKey(old[i]) !== rowKey(next[0])) continue
                var matches = true
                for (var j = 1; j < count; ++j) if (rowKey(old[i + j]) !== rowKey(next[j])) { matches = false; break }
                if (matches) { cutRows = i; overlap = count; break }
            }
        }

        var oldX = sameSource ? viewport.contentX : 0, oldY = viewport.contentY
        var oldCursor = consoleText.cursorPosition, start = consoleText.selectionStart, end = consoleText.selectionEnd
        var previousHeight = consoleText.contentHeight, removedCharacters = 0
        var incremental = prefix || overlap > 0
        var generation = ++renderGeneration
        var interaction = interactionGeneration
        if (cutRows) {
            // TextEdit positions and JavaScript strings both count UTF-16 code units.
            for (i = 0; i < cutRows; ++i) removedCharacters += String(old[i].text).replace(/\t/g, "    ").length + 1
            consoleText.remove(0, removedCharacters)
            oldY = Math.max(0, oldY - Math.max(0, previousHeight - consoleText.contentHeight))
        }
        if (incremental) {
            var added = next.slice(prefix ? old.length : overlap)
            if (added.length) consoleText.append(added.map(rowHtml).join(""))
        } else {
            consoleText.text = next.map(rowHtml).join("")
        }
        renderedRows = next
        renderedSource = sourceKey

        // Updating a document must not move its selection or viewport as a side effect.
        if (incremental || sameSource) {
            if (end > start) consoleText.select(Math.max(0, start - removedCharacters), Math.max(0, end - removedCharacters))
            else consoleText.cursorPosition = Math.max(0, Math.min(consoleText.length, oldCursor - removedCharacters))
        }
        if (!sameSource) { consoleText.deselect(); consoleText.cursorPosition=0; viewport.contentX=0 }
        if (follow) scrollToBottom()
        else {
            viewport.contentX = Math.max(0, Math.min(oldX, Math.max(0, viewport.contentWidth - viewport.width)))
            viewport.contentY = sameSource ? Math.max(0, Math.min(oldY, Math.max(0, viewport.contentHeight - viewport.height))) : 0
        }
        Qt.callLater(function() {
            if (generation !== control.renderGeneration || interaction !== control.interactionGeneration || control.selectionDragging) return
            if (control.follow) control.scrollToBottom()
            else {
                viewport.contentX = Math.max(0, Math.min(oldX, Math.max(0, viewport.contentWidth - viewport.width)))
                viewport.contentY = sameSource ? Math.max(0, Math.min(oldY, Math.max(0, viewport.contentHeight - viewport.height))) : 0
            }
        })
    }
    onRowsChanged: syncDocument(false)
    onSourceKeyChanged: syncDocument(false)
    onLightThemeChanged: syncDocument(true)
    onFollowChanged: if (follow) scrollToBottom()
    Component.onCompleted: { initialized = true; syncDocument(true) }

    Flickable {
        id: viewport
        objectName: "logViewport"
        anchors.fill: parent
        anchors.margins: 2
        anchors.rightMargin: verticalBar.width + 5
        anchors.bottomMargin: horizontalBar.height + 5
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        interactive: false // Dragging belongs to text selection; wheel and scrollbars handle scrolling.
        contentWidth: consoleText.width
        contentHeight: consoleText.height

        TextArea {
            id: consoleText
            objectName: "logConsole"
            width: Math.max(viewport.width, implicitWidth)
            height: Math.max(viewport.height, implicitHeight)
            readOnly: true
            selectByMouse: true
            persistentSelection: true
            textFormat: TextEdit.RichText
            wrapMode: TextEdit.NoWrap
            font.family: "Consolas"
            font.pixelSize: control.fontSize
            color: control.appWindow ? control.appWindow.textColor : "#edf0fa"
            selectionColor: control.lightTheme ? "#d5c9f4" : "#59458c"
            selectedTextColor: control.lightTheme ? "#231638" : "#ffffff"
            padding: 12
            background: null
            Keys.onPressed: function(event) {
                control.beginReading()
                if (event.matches(StandardKey.Copy)) { control.copySelection(); event.accepted=true }
            }
            PointHandler {
                id: selectionPointer
                target: null
                acceptedButtons: Qt.LeftButton
                onActiveChanged: {
                    if (active) {
                        control.dragAnchor=consoleText.positionAt(point.position.x,point.position.y)
                        control.beginReading()
                    } else if (control.documentPending) Qt.callLater(function(){control.syncDocument(false)})
                }
            }
            TapHandler { acceptedButtons: Qt.RightButton; onTapped: consoleMenu.popup() }
        }
        Label {
            visible: control.rows.length === 0
            x: 12
            y: 12
            text: control.emptyText
            textFormat: Text.PlainText
            color: control.appWindow ? control.appWindow.mutedColor : "#a7b1c8"
            font.family: "Consolas"
            font.pixelSize: control.fontSize
        }
        ScrollBar.vertical: ScrollBar {
            id: verticalBar
            objectName: "logVerticalScrollBar"
            parent: control
            x: control.width-width-3; y: 3
            width: 10; height: Math.max(0,control.height-horizontalBar.height-6)
            policy: ScrollBar.AlwaysOn
            onPressedChanged: if(pressed)control.beginReading()
        }
        ScrollBar.horizontal: ScrollBar {
            id: horizontalBar
            objectName: "logHorizontalScrollBar"
            parent: control
            x: 3; y: control.height-height-3
            height: 10; width: Math.max(0,control.width-verticalBar.width-6)
            policy: ScrollBar.AlwaysOn
            onPressedChanged: if(pressed)control.beginReading()
        }
        WheelHandler {
            target: null
            onWheel: function(event) {
                control.beginReading()
                var dx = event.pixelDelta.x || event.angleDelta.x / 120 * 52
                var dy = event.pixelDelta.y || event.angleDelta.y / 120 * 52
                if (event.modifiers & Qt.ShiftModifier) { dx += dy; dy = 0 }
                viewport.contentX = Math.max(0, Math.min(Math.max(0, viewport.contentWidth - viewport.width), viewport.contentX - dx))
                viewport.contentY = Math.max(0, Math.min(Math.max(0, viewport.contentHeight - viewport.height), viewport.contentY - dy))
                event.accepted = true
            }
        }
    }
    Timer { interval: 16; repeat: true; running: control.selectionDragging; onTriggered: control.dragScroll() }
    ThemedMenu { appWindow: control.appWindow;
        id: consoleMenu
        ThemedMenuItem { appWindow: control.appWindow; text: control.appWindow.tr("Копировать выделенное", "Copy selection"); enabled: consoleText.selectedText.length > 0; onTriggered: control.copySelection() }
        ThemedMenuItem { appWindow: control.appWindow; text: control.appWindow.tr("Копировать всё", "Copy all"); enabled: control.rows.length > 0; onTriggered: control.copyRequested(control.plainText) }
        ThemedMenuItem { appWindow: control.appWindow; text: control.appWindow.tr("Выделить всё", "Select all"); enabled: control.rows.length > 0; onTriggered: control.selectAll() }
    }
}
