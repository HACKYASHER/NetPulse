#include "Backend.h"
#include <QApplication>
#include <QCommandLineParser>
#include <QDir>
#include <QFile>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QLockFile>
#include <QMenu>
#include <QMessageBox>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSystemTrayIcon>
#include <QSet>
#include <QTimer>

int main(int argc, char** argv) {
    QApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("NetPulseQt"));
    QCoreApplication::setOrganizationName(QStringLiteral("NetPulse"));
    QCoreApplication::setApplicationVersion(QStringLiteral("1.0.0-beta"));
    QApplication::setFont(QFont(QStringLiteral("Segoe UI"), 10));
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QCommandLineParser parser;
    parser.addHelpOption(); parser.addVersionOption();
    parser.addOption({QStringLiteral("data-dir"), QStringLiteral("Independent data folder"), QStringLiteral("path")});
    parser.addOption({QStringLiteral("background"), QStringLiteral("Start in the tray")});
    parser.addOption({QStringLiteral("installer-check"), QStringLiteral("Check that this user's Qt client is closed")});
    parser.addOption({QStringLiteral("smoke-test"), QStringLiteral("Render all QML pages in isolated mode and exit"), QStringLiteral("output-directory")});
    parser.process(app);
    const bool smoke = parser.isSet(QStringLiteral("smoke-test"));
    QString dataDirectory = parser.value(QStringLiteral("data-dir"));
    if (smoke) dataDirectory = QDir(parser.value(QStringLiteral("smoke-test"))).absoluteFilePath(QStringLiteral("data"));
    if (dataDirectory.isEmpty()) dataDirectory = qEnvironmentVariable("LOCALAPPDATA") + QStringLiteral("/NetPulseQt");
    dataDirectory = QDir(dataDirectory).absolutePath(); QDir().mkpath(dataDirectory);
    QLockFile lock(dataDirectory + QStringLiteral("/client.lock"));
    if (parser.isSet(QStringLiteral("installer-check"))) return lock.tryLock(100) ? 0 : 10;
    if (!lock.tryLock(100)) { QMessageBox::information(nullptr, QStringLiteral("NetPulse"), QStringLiteral("Эта копия NetPulse уже работает. Откройте её через значок в трее.")); return 1; }
    Backend backend(dataDirectory, smoke);
    QQmlApplicationEngine engine;
    int qmlIssues = 0;
    QStringList qmlMessages;
    QObject::connect(&engine, &QQmlEngine::warnings, &app, [&qmlIssues, &qmlMessages](const QList<QQmlError>& warnings) {
        for (const auto& warning : warnings) { ++qmlIssues; qmlMessages.append(warning.toString()); }
    });
    engine.rootContext()->setContextProperty(QStringLiteral("backend"), &backend);
    engine.rootContext()->setContextProperty(QStringLiteral("smokeMode"), smoke);
    engine.rootContext()->setContextProperty(QStringLiteral("startHidden"), parser.isSet(QStringLiteral("background")));
    engine.loadFromModule(QStringLiteral("NetPulse"), QStringLiteral("Main"));
    if (engine.rootObjects().isEmpty()) return 2;
    auto* window = qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    app.setQuitOnLastWindowClosed(false);
    QSystemTrayIcon tray(QIcon(QStringLiteral(":/qt/qml/NetPulse/resources/pulse.svg")));
    tray.setToolTip(QStringLiteral("NetPulse"));
    QMenu menu;
    menu.addAction(QStringLiteral("Открыть NetPulse"), window, [window] { window->show(); window->raise(); window->requestActivate(); });
    menu.addAction(QStringLiteral("Отключить"), &backend, [&backend] { backend.request(QStringLiteral("disconnect")); });
    menu.addSeparator(); menu.addAction(QStringLiteral("Выход"), &backend, &Backend::quit);
    tray.setContextMenu(&menu);
    QObject::connect(&tray, &QSystemTrayIcon::activated, window, [window](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) { window->show(); window->raise(); window->requestActivate(); }
    });
    if (!smoke) tray.show();
    QObject::connect(&backend, &Backend::quitReady, &app, [&] { app.exit(smoke && qmlIssues ? 5 : 0); });
    QSet<int> capturedPages;
    if (smoke) {
        const QString output = QDir(parser.value(QStringLiteral("smoke-test"))).absolutePath(); QDir().mkpath(output);
        auto* watchdog = new QTimer(&app); watchdog->setSingleShot(true);
        QObject::connect(watchdog, &QTimer::timeout, &app, [&app] { app.exit(4); }); watchdog->start(60000);
        QObject::connect(&backend, &Backend::eventReceived, &app, [&backend](const QString& name, const QVariant&) {
            if (name == QStringLiteral("runtime-prepared")) backend.refresh();
        });
        // QML sets smokeFinished after each page has had a rendering turn.
        auto* check = new QTimer(&app); check->setInterval(100);
        QObject::connect(check, &QTimer::timeout, &app, [&, check, output] {
            const int page = window->property("page").toInt();
            if (!capturedPages.contains(page) && backend.ready()) {
                capturedPages.insert(page);
                window->grabWindow().save(output + QStringLiteral("/page-%1.png").arg(page));
            }
            if (!window->property("smokeFinished").toBool()) return;
            check->stop();
            window->grabWindow().save(output + QStringLiteral("/window.png"));
            const QJsonObject result{{QStringLiteral("result"), qmlIssues ? QStringLiteral("failed") : QStringLiteral("passed")}, {QStringLiteral("qt"), QString::fromLatin1(qVersion())},
                {QStringLiteral("version"), QCoreApplication::applicationVersion()}, {QStringLiteral("pages"), window->property("smokePages").toInt()},
                {QStringLiteral("qml_issues"), qmlIssues}, {QStringLiteral("qml_messages"), QJsonArray::fromStringList(qmlMessages)},
                {QStringLiteral("network_changed"), false}, {QStringLiteral("webview_used"), false}};
            QFile file(output + QStringLiteral("/result.json")); if (file.open(QIODevice::WriteOnly)) file.write(QJsonDocument(result).toJson());
            backend.quit();
        }); check->start();
    }
    QTimer::singleShot(0, &backend, &Backend::start);
    return app.exec();
}
