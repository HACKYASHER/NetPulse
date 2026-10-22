pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ColumnLayout {
    id: view
    property var appWindow: ApplicationWindow.window
    property var bridge: backend
    property var collapsedGroups: ({})
    property var serverContext: ({})
    property var subscriptionContext: ({})
    readonly property bool latencyBusy: appWindow.busy("start_latency") || !!appWindow.diagnosticRunning || !!(appWindow.job && appWindow.job.running)
    signal addRequested()
    spacing: 14

    function tr(ru, en) { return appWindow.tr(ru, en) }
    function groupKey(group) { return group.id || "standalone" }
    function toggleGroup(group) {
        var next = Object.assign({}, collapsedGroups), key = groupKey(group)
        next[key] = !next[key]
        collapsedGroups = next
    }
    function subscriptionProfiles(group) {
        return appWindow.profiles.filter(function(profile){return (profile.subscription_id||"")===(group.id||"")})
    }
    function testProfiles(items) {
        if (!items.length) return
        appWindow.testServers(items.map(function(item) { return item.id }))
    }
    function configLocked(profile) {
        return appWindow.subscriptions.some(function(sub) { return sub.id === profile.subscription_id && sub.config_locked })
    }
    function latency(profile) {
        return profile.latency_ms === null || profile.latency_ms === undefined ? "—" : Math.round(profile.latency_ms) + " ms"
    }

    Input {
        objectName: "serverSearch"
        Layout.fillWidth: true
        placeholderText: view.tr("Поиск серверов…", "Search servers…")
        text: view.appWindow.profileSearch
        onTextChanged: view.appWindow.profileSearch = text
    }
    RowLayout {
        Layout.fillWidth: true
        spacing: 8
        Choice { appWindow: view.appWindow;
            objectName: "serverSource"
            model: [view.tr("Все источники", "All sources")].concat(view.appWindow.subscriptions.map(function(sub) { return sub.name }))
            currentIndex: Math.max(0, view.appWindow.subscriptions.findIndex(function(sub) { return sub.id === view.appWindow.sourceFilter }) + 1)
            onActivated: view.appWindow.sourceFilter = currentIndex === 0 ? "all" : view.appWindow.subscriptions[currentIndex - 1].id
        }
        Choice { appWindow: view.appWindow;
            model: [view.tr("По названию", "By name"), view.tr("По пингу", "By latency")]
            currentIndex: view.appWindow.serverSort
            onActivated: view.appWindow.serverSort = currentIndex
        }
        Item { Layout.fillWidth: true }
        ActionButton { appWindow: view.appWindow; text: "+ " + view.tr("Добавить подключение", "Add connection"); primary: true; onClicked: view.addRequested() }
    }
    Label {
        textFormat: Text.PlainText
        text: view.tr("Серверов: ", "Servers: ") + view.appWindow.profiles.length + view.tr(" · ПКМ — действия с сервером", " · Right-click a server for actions")
        color: view.appWindow.mutedColor
        font.pixelSize: 11
    }
    Label {
        visible: view.appWindow.profiles.length === 0 && view.appWindow.subscriptions.length === 0
        text: view.tr("Добавьте подписку или ссылку на сервер", "Add a subscription or server link")
        color: view.appWindow.mutedColor
    }

    Repeater {
        model: view.appWindow.serverGroups()
        delegate: Card {
            id: groupCard
            required property var modelData
            readonly property bool collapsed: !!view.collapsedGroups[view.groupKey(modelData)]
            Layout.fillWidth: true
            implicitHeight: groupContents.implicitHeight + 2
            radius: 10

            ColumnLayout {
                id: groupContents
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 1
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.margins: 12
                    spacing: 10
                    ActionButton { appWindow: view.appWindow;
                        text: groupCard.collapsed ? "▸" : "▾"
                        implicitWidth: 28
                        implicitHeight: 34
                        leftPadding: 0
                        rightPadding: 0
                        ToolTip.text: groupCard.collapsed ? view.tr("Развернуть подписку", "Expand subscription") : view.tr("Свернуть подписку", "Collapse subscription")
                        onClicked: view.toggleGroup(groupCard.modelData)
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        RowLayout {
                            spacing: 8
                            Label {
                                textFormat: Text.PlainText
                                text: groupCard.modelData.name
                                color: view.appWindow.textColor
                                font.bold: true
                                font.pixelSize: 13
                                Layout.fillWidth: true
                                elide: Text.ElideRight
                            }
                            Label {
                                text: groupCard.modelData.items.length
                                color: view.appWindow.mutedColor
                                font.pixelSize: 10
                                padding: 4
                                background: Rectangle { radius: 5; color: view.appWindow.bgColor; border.color: view.appWindow.borderColor }
                            }
                        }
                        Label {
                            visible: !!groupCard.modelData.id
                            textFormat: Text.PlainText
                            text: (groupCard.modelData.updated_at ? new Date(groupCard.modelData.updated_at * 1000).toLocaleString() : view.tr("Ещё не обновлялась", "Not refreshed yet"))
                                  + view.tr(" · каждые ", " · every ") + groupCard.modelData.interval_hours + view.tr(" ч · при запуске", " h · on startup")
                            color: view.appWindow.mutedColor
                            font.pixelSize: 10
                            Layout.fillWidth: true
                            elide: Text.ElideRight
                        }
                    }
                    ActionButton { appWindow: view.appWindow;
                        visible: !!groupCard.modelData.id
                        objectName: "subscriptionRefresh"
                        text: "↻"
                        implicitWidth: 36
                        implicitHeight: 34
                        leftPadding: 4
                        rightPadding: 4
                        enabled: !view.appWindow.busy("update_subscription")
                        ToolTip.text: view.tr("Обновить подписку", "Refresh subscription")
                        onClicked: view.appWindow.call("update_subscription", {id: groupCard.modelData.id})
                    }
                    ActionButton { appWindow: view.appWindow;
                        objectName: "subscriptionPing"
                        text: "◴"
                        implicitWidth: 36
                        implicitHeight: 34
                        leftPadding: 4
                        rightPadding: 4
                        enabled: !view.latencyBusy && groupCard.modelData.items.length > 0
                        ToolTip.text: view.tr("Проверить пинг серверов подписки", "Test subscription server latency")
                        onClicked: view.testProfiles(view.subscriptionProfiles(groupCard.modelData))
                    }
                    ActionButton { appWindow: view.appWindow;
                        visible: !!groupCard.modelData.id
                        text: "…"
                        implicitWidth: 36
                        implicitHeight: 34
                        leftPadding: 4
                        rightPadding: 4
                        ToolTip.text: view.tr("Действия с подпиской", "Subscription actions")
                        onClicked: { view.subscriptionContext = groupCard.modelData; subscriptionMenu.popup() }
                    }
                }
                Rectangle {
                    visible: !!groupCard.modelData.id && !groupCard.collapsed
                    Layout.fillWidth: true
                    implicitHeight: usageLabel.implicitHeight + 16
                    color: view.appWindow.light ? "#f7f8fc" : "#121621"
                    Rectangle { anchors.top: parent.top; width: parent.width; height: 1; color: view.appWindow.borderColor }
                    Label {
                        id: usageLabel
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.margins: 14
                        textFormat: Text.PlainText
                        text: {
                            var usage = groupCard.modelData.usage
                            if (!usage) return view.tr("Трафик без лимита · Без срока", "Unlimited traffic · No expiry")
                            return view.tr("Трафик: ", "Traffic: ") + view.appWindow.bytes((usage.upload || 0) + (usage.download || 0)) + " / " + (usage.total ? view.appWindow.bytes(usage.total) : "∞")
                                + (usage.expire ? " · " + new Date(usage.expire * 1000).toLocaleDateString() : view.tr(" · Без срока", " · No expiry"))
                        }
                        color: view.appWindow.mutedColor
                        font.pixelSize: 11
                        elide: Text.ElideRight
                    }
                }
                Repeater {
                    model: groupCard.collapsed ? [] : groupCard.modelData.items
                    delegate: Rectangle {
                        id: serverRow
                        required property var modelData
                        readonly property bool selected: view.appWindow.prefs.selected_profile === modelData.id
                        Layout.fillWidth: true
                        implicitHeight: 38
                        color: selected ? (view.appWindow.light ? "#e8dffb" : "#2b2542") : rowMouse.containsMouse ? (view.appWindow.light ? "#eee9fa" : "#242b3e") : "transparent"
                        Rectangle { anchors.top: parent.top; height: 1; width: parent.width; color: view.appWindow.borderColor }
                        Rectangle { anchors.left: parent.left; anchors.top: parent.top; anchors.bottom: parent.bottom; width: 3; visible: serverRow.selected; color: view.appWindow.accentColor }
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 14
                            spacing: 9
                            Flag { name: serverRow.modelData.name; Layout.preferredWidth: 22; Layout.preferredHeight: 15 }
                            Label {
                                textFormat: Text.PlainText
                                text: (view.appWindow.pinned(serverRow.modelData.id) ? "⌖  " : "") + view.appWindow.cleanName(serverRow.modelData.name) + (serverRow.selected ? "  ✓" : "")
                                Layout.fillWidth: true
                                Layout.minimumWidth: 120
                                color: view.appWindow.textColor
                                elide: Text.ElideRight
                                font.pixelSize: 12
                                font.bold: true
                            }
                            Label {
                                textFormat: Text.PlainText
                                text: String(serverRow.modelData.protocol).toUpperCase()
                                color: view.appWindow.mutedColor
                                font.pixelSize: 10
                            }
                            Label {
                                textFormat: Text.PlainText
                                text: serverRow.modelData.host + ":" + serverRow.modelData.port
                                color: view.appWindow.mutedColor
                                font.pixelSize: 10
                                Layout.maximumWidth: Math.max(130, Math.min(245, view.width * 0.3))
                                elide: Text.ElideMiddle
                            }
                            Label {
                                text: view.latency(serverRow.modelData)
                                color: serverRow.modelData.latency_ms === null || serverRow.modelData.latency_ms === undefined ? view.appWindow.mutedColor : view.appWindow.light ? "#176443" : "#79e0bf"
                                font.pixelSize: 11
                                Layout.minimumWidth: 44
                                horizontalAlignment: Text.AlignRight
                            }
                        }
                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onClicked: function(mouse) {
                                if (mouse.button === Qt.RightButton) { view.serverContext = serverRow.modelData; serverMenu.popup() }
                                else view.appWindow.call("select_profile", {id: serverRow.modelData.id})
                            }
                            onDoubleClicked: function(mouse) {
                                if (mouse.button === Qt.LeftButton) view.appWindow.call("select_profile", {id: serverRow.modelData.id}, function() { if (!view.appWindow.snapshot.engine_pid) view.appWindow.reconnectSelected() })
                            }
                        }
                    }
                }
            }
        }
    }

    ThemedMenu { appWindow: view.appWindow;
        id: serverMenu
        ThemedMenuItem { appWindow: view.appWindow; text: view.tr("Проверить пинг", "Test latency"); enabled: !view.latencyBusy; onTriggered: view.testProfiles([view.serverContext]) }
        ThemedMenuItem { appWindow: view.appWindow; text: view.appWindow.pinned(view.serverContext.id) ? view.tr("Открепить", "Unpin") : view.tr("Закрепить", "Pin"); onTriggered: view.appWindow.call("quick_ui", {patch: {favorite_id: view.serverContext.id}}) }
        ThemedMenuItem { appWindow: view.appWindow;
            objectName: "serverConfigure"
            text: view.tr("Настройка конфига", "Configure server")
            visible: !view.configLocked(view.serverContext)
            onTriggered: view.appWindow.call("profile_json", {id: view.serverContext.id}, function(json) { view.appWindow.editJson(view.appWindow.cleanName(view.serverContext.name), json, "save_profile_json", {id: view.serverContext.id}) })
        }
        MenuSeparator { contentItem: Rectangle { implicitHeight: 1; color: view.appWindow.borderColor } }
        ThemedMenuItem { appWindow: view.appWindow; text: view.tr("Удалить сервер", "Delete server"); danger: true; onTriggered: view.bridge.confirmDelete("delete_profile", {id: view.serverContext.id}, view.tr("Удалить сервер?", "Delete this server?")) }
    }
    ThemedMenu { appWindow: view.appWindow;
        id: subscriptionMenu
        ThemedMenuItem { appWindow: view.appWindow; text: view.tr("Настройки подписки", "Subscription settings"); onTriggered: view.appWindow.editSubscription(view.subscriptionContext) }
        ThemedMenuItem { appWindow: view.appWindow; text: view.tr("Обновить", "Refresh"); enabled: !view.appWindow.busy("update_subscription"); onTriggered: view.appWindow.call("update_subscription", {id: view.subscriptionContext.id}) }
        ThemedMenuItem { appWindow: view.appWindow;
            objectName: "subscriptionCopyUrl"
            text: view.tr("Копировать ссылку", "Copy URL")
            visible: !view.subscriptionContext.url_protected
            onTriggered: view.appWindow.call("copy_subscription_url", {id: view.subscriptionContext.id})
        }
        MenuSeparator { contentItem: Rectangle { implicitHeight: 1; color: view.appWindow.borderColor } }
        ThemedMenuItem { appWindow: view.appWindow; text: view.tr("Удалить подписку", "Delete subscription"); danger: true; onTriggered: view.bridge.confirmDelete("delete_subscription", {id: view.subscriptionContext.id}, view.tr("Удалить подписку и её серверы?", "Delete this subscription and its servers?")) }
    }
}
