#include <QStyleHints>
#include <QFileIconProvider>
#include <QQuickImageProvider>
#include <QFileInfo>
#include <QCache>
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
#include <QCursor>
#include <QMessageBox>
#include <QQmlApplicationEngine>
#include <QQmlContext>
#include <QQuickStyle>
#include <QQuickWindow>
#include <QSystemTrayIcon>
#include <QSet>
#include <QTimer>
#include <QProcess>
#include <memory>
#include <vector>
#include <cstdio>
#ifdef Q_OS_WIN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#endif

class ApplicationIcons final : public QQuickImageProvider {
public:
 ApplicationIcons():QQuickImageProvider(QQuickImageProvider::Pixmap){}
 QPixmap requestPixmap(const QString& id,QSize* size,const QSize& requested) override {
  const QString path=QUrl::fromPercentEncoding(id.toUtf8());
  const QSize target=requested.isValid()?requested:QSize(32,32);
  if(size)*size=target;
  // Imported process paths must not make the icon provider contact SMB shares
  // or traverse directory junctions before the user takes any action.
  if(path.size()<4 || !((path[0]>=QChar(u'A')&&path[0]<=QChar(u'Z'))||(path[0]>=QChar(u'a')&&path[0]<=QChar(u'z'))) || path[1]!=QChar(u':') || (path[2]!=QChar(u'\\')&&path[2]!=QChar(u'/')) || path.mid(2).contains(QChar(u':')))return {};
#ifdef Q_OS_WIN
  const auto parts=QDir::fromNativeSeparators(path).split(QChar(u'/'),Qt::SkipEmptyParts);
  QString checked=parts.first()+QStringLiteral("/");
  for(qsizetype index=1;index<parts.size();++index){checked+=parts[index];const DWORD attributes=GetFileAttributesW(reinterpret_cast<const wchar_t*>(checked.utf16()));if(attributes==INVALID_FILE_ATTRIBUTES || (attributes&FILE_ATTRIBUTE_REPARSE_POINT))return {};checked+=QChar(u'/');}
#endif
  if(auto* cached=cache.object(path))return cached->scaled(target,Qt::KeepAspectRatio,Qt::SmoothTransformation);
  QFileInfo file(path); if(!file.isAbsolute() || file.suffix().compare(QStringLiteral("exe"),Qt::CaseInsensitive)!=0)return {};
  QPixmap pixmap=provider.icon(file).pixmap(32,32); if(pixmap.isNull())pixmap=provider.icon(QFileIconProvider::File).pixmap(32,32); cache.insert(path,new QPixmap(pixmap)); return pixmap;
 }
private: QFileIconProvider provider; QCache<QString,QPixmap> cache{1000};
};
int main(int argc, char** argv) {
    QApplication app(argc, argv);
    QCoreApplication::setApplicationName(QStringLiteral("NetPulse"));
    QCoreApplication::setOrganizationName(QStringLiteral("NetPulse"));
    QCoreApplication::setApplicationVersion(QStringLiteral("1.0.7"));
    QApplication::setFont(QFont(QStringLiteral("Segoe UI"), 10));
    QQuickStyle::setStyle(QStringLiteral("Basic"));
    QCommandLineParser parser;
    parser.addHelpOption(); parser.addVersionOption();
    parser.addOption({QStringLiteral("data-dir"), QStringLiteral("Independent data folder"), QStringLiteral("path")});
    parser.addOption({QStringLiteral("background"), QStringLiteral("Start in the tray")});
    parser.addOption({QStringLiteral("installer-check"), QStringLiteral("Check that this user's Qt client is closed")});
    parser.addOption({QStringLiteral("startup-check"), QStringLiteral("Check profile preparation and locking without starting the network")});
    parser.addOption({QStringLiteral("smoke-test"), QStringLiteral("Render all QML pages in isolated mode and exit"), QStringLiteral("output-directory")});
    parser.addOption({QStringLiteral("isolated"), QStringLiteral("Use isolated data for UI verification")});
    parser.process(app);
    const bool smoke = parser.isSet(QStringLiteral("smoke-test"));
    const bool startupCheck=parser.isSet(QStringLiteral("startup-check"));
    const auto startupFailure=[startupCheck](int code,const QString& message,bool information=false) {
        if(startupCheck) std::fprintf(stderr,"%s\n",message.toUtf8().constData());
        else if(information) QMessageBox::information(nullptr,QStringLiteral("NetPulse"),message);
        else QMessageBox::critical(nullptr,QStringLiteral("NetPulse"),message);
        return code;
    };
    QString dataDirectory = parser.value(QStringLiteral("data-dir"));
    if (smoke) dataDirectory = QDir(parser.value(QStringLiteral("smoke-test"))).absoluteFilePath(QStringLiteral("data"));

    std::vector<std::unique_ptr<QLockFile>> legacyLocks;
    if (dataDirectory.isEmpty()) {
        QStringList sources{qEnvironmentVariable("LOCALAPPDATA")+QStringLiteral("/NetPulseQt")};
        if (!parser.isSet(QStringLiteral("installer-check"))) {
            QProcess paths;paths.setProgram(QCoreApplication::applicationDirPath()+QStringLiteral("/netpulsed.exe"));paths.setArguments({QStringLiteral("--profile-paths")});
#ifdef Q_OS_WIN
            paths.setCreateProcessArgumentsModifier([](QProcess::CreateProcessArguments* a){a->flags|=0x08000000;});
#endif
            paths.start();if(!paths.waitForStarted(10000)||!paths.waitForFinished(10000)||paths.exitCode()!=0)return startupFailure(12,QStringLiteral("Не удалось определить прежнюю папку данных NetPulse."));
            sources.clear();for(const auto& source:QJsonDocument::fromJson(paths.readAllStandardOutput()).object().value(QStringLiteral("sources")).toArray())sources.append(source.toString());
        }
        for (const auto& legacy:sources) if (QFileInfo::exists(legacy+QStringLiteral("/netpulse.db"))) {
            auto sourceLock=std::make_unique<QLockFile>(legacy+QStringLiteral("/client.lock"));
            if (!sourceLock->tryLock(100)) {
                if (!parser.isSet(QStringLiteral("installer-check"))) return startupFailure(10,sourceLock->error()==QLockFile::LockFailedError?QStringLiteral("Закройте прежний NetPulse через значок в трее перед переносом данных."):QStringLiteral("Не удалось открыть папку прежнего профиля NetPulse. Проверьте права доступа к папке:\n")+legacy,true);
                return 10;
            }
            legacyLocks.push_back(std::move(sourceLock));
        }
        // Checking a closed installation must not migrate data while elevated.
        if (parser.isSet(QStringLiteral("installer-check"))) {
            QDir profiles(QCoreApplication::applicationDirPath()+QStringLiteral("/data"));
            for (const auto& name:profiles.entryList({QStringLiteral("S-1-*")},QDir::Dirs|QDir::NoDotAndDotDot)) {
                QLockFile profileLock(profiles.filePath(name+QStringLiteral("/client.lock")));
                if (!profileLock.tryLock(100)) return 10;
            }
            return 0;
        }
        QProcess resolver;
        resolver.setProgram(QCoreApplication::applicationDirPath()+QStringLiteral("/netpulsed.exe"));
        resolver.setArguments({QStringLiteral("--resolve-data")});
#ifdef Q_OS_WIN
        resolver.setCreateProcessArgumentsModifier([](QProcess::CreateProcessArguments* a){a->flags|=0x08000000;});
#endif
        resolver.start();
        if (!resolver.waitForStarted(10000) || !resolver.waitForFinished(180000) || resolver.exitCode()!=0) {
            const auto detail=QJsonDocument::fromJson(resolver.readAllStandardOutput()).object().value(QStringLiteral("data")).toObject().value(QStringLiteral("message")).toString();
            return startupFailure(12,QStringLiteral("Не удалось подготовить папку данных NetPulse. Исходный профиль сохранён.\n")+detail+QString::fromUtf8(resolver.readAllStandardError()));
        }
        dataDirectory=QJsonDocument::fromJson(resolver.readAllStandardOutput()).object().value(QStringLiteral("directory")).toString();
        if (dataDirectory.isEmpty() || !QDir::isAbsolutePath(dataDirectory)) return 12;
    }
    dataDirectory = QDir(dataDirectory).absolutePath();
    if(!QDir().mkpath(dataDirectory))return startupFailure(12,QStringLiteral("Не удалось создать папку данных NetPulse:\n")+dataDirectory);
    const QString lockPath=dataDirectory+QStringLiteral("/client.lock");
    const QFileInfo lockInfo(lockPath);
    if(lockInfo.isSymLink()||(lockInfo.exists()&&!lockInfo.isFile()))return startupFailure(12,QStringLiteral("Файл блокировки NetPulse имеет неверный тип. Проверьте путь:\n")+lockPath);
    QLockFile lock(lockPath);
    if (parser.isSet(QStringLiteral("installer-check"))) return lock.tryLock(100) ? 0 : 10;
    if (!lock.tryLock(100)) {
        if(lock.error()==QLockFile::LockFailedError)return startupFailure(1,QStringLiteral("Эта копия NetPulse уже работает. Откройте её через значок в трее."),true);
        return startupFailure(12,QStringLiteral("Не удалось создать файл блокировки NetPulse. Проверьте права доступа к папке данных:\n")+dataDirectory);
    }
    if(startupCheck)return 0;
    Backend backend(dataDirectory, smoke || parser.isSet(QStringLiteral("isolated")));
    QQmlApplicationEngine engine;
    engine.addImageProvider(QStringLiteral("apps"),new ApplicationIcons);
    int qmlIssues = 0;
    QStringList qmlMessages;
    QObject::connect(&engine, &QQmlEngine::warnings, &app, [&qmlIssues, &qmlMessages](const QList<QQmlError>& warnings) {
        for (const auto& warning : warnings) { ++qmlIssues; qmlMessages.append(warning.toString()); }
    });
    engine.rootContext()->setContextProperty(QStringLiteral("backend"), &backend);
    engine.rootContext()->setContextProperty(QStringLiteral("smokeMode"), smoke);
    engine.rootContext()->setContextProperty(QStringLiteral("startHidden"), parser.isSet(QStringLiteral("background")));
    engine.loadFromModule(QStringLiteral("NetPulse"), QStringLiteral("Main"));
    if (engine.rootObjects().isEmpty()) {
        if(smoke){QDir output(parser.value(QStringLiteral("smoke-test")));output.mkpath(QStringLiteral("."));QFile file(output.filePath(QStringLiteral("result.json")));if(file.open(QIODevice::WriteOnly))file.write(QJsonDocument(QJsonObject{{QStringLiteral("result"),QStringLiteral("failed")},{QStringLiteral("qml_messages"),QJsonArray::fromStringList(qmlMessages)}}).toJson());}
        return 2;
    }
    auto* window = qobject_cast<QQuickWindow*>(engine.rootObjects().first());
    app.setQuitOnLastWindowClosed(false);
    QSystemTrayIcon tray(QIcon(QStringLiteral(":/qt/qml/NetPulse/resources/pulse.svg")));
    tray.setToolTip(QStringLiteral("NetPulse"));
    QMenu menu;
    const auto showWindow=[window]{window->show();window->raise();window->requestActivate();};
    const auto invoke=[window,showWindow](const char* method){showWindow();QMetaObject::invokeMethod(window,method);};
    auto* openAction=menu.addAction(QStringLiteral("Открыть NetPulse"),window,showWindow);
    auto* connectAction=menu.addAction(QStringLiteral("Подключиться…"),window,[invoke]{invoke("connectSelected");});
    auto* disconnectAction=menu.addAction(QStringLiteral("Отключиться"),&backend,[&backend]{backend.request(QStringLiteral("disconnect"));});
    auto* reconnectAction=menu.addAction(QStringLiteral("Переподключиться…"),window,[invoke]{invoke("reconnectSelected");});
    auto* modes=menu.addMenu(QStringLiteral("Режим подключения"));
    auto* tunAction=modes->addAction(QStringLiteral("TUN"),window,[invoke]{invoke("connectTun");});tunAction->setCheckable(true);
    auto* proxyAction=modes->addAction(QStringLiteral("System Proxy"),window,[invoke]{invoke("connectProxy");});proxyAction->setCheckable(true);
    menu.addSeparator();
    auto* serversAction=menu.addAction(QStringLiteral("Серверы…"),window,[invoke]{invoke("showServers");});
    auto* ipAction=menu.addAction(QStringLiteral("Проверить IP…"),window,[invoke]{invoke("showPublicIps");});
    auto* networkAction=menu.addAction(QStringLiteral("Сеть…"),window,[invoke]{invoke("showNetwork");});
    auto* settingsAction=menu.addAction(QStringLiteral("Настройки…"),window,[invoke]{invoke("showSettings");});
    menu.addSeparator();auto* quitAction=menu.addAction(QStringLiteral("Выход"),&backend,&Backend::quit);
    QString trayAppearance;
    const auto updateTray=[&] {
        const auto data=backend.data();const auto prefs=data.value(QStringLiteral("preferences")).toMap();const auto snapshot=data.value(QStringLiteral("snapshot")).toMap();
        const bool en=prefs.value(QStringLiteral("language")).toString()==QStringLiteral("en");
        const auto theme=prefs.value(QStringLiteral("theme")).toString();
        const bool light=theme==QStringLiteral("light") || (theme==QStringLiteral("system")&&app.styleHints()->colorScheme()==Qt::ColorScheme::Light);
        const QString appearance=QString::number(en)+QString::number(light);
        if(trayAppearance!=appearance){trayAppearance=appearance;
            menu.setStyleSheet(light?QStringLiteral("QMenu{background:#ffffff;color:#222536;border:1px solid #c7ccda;padding:5px}QMenu::item{padding:8px 28px 8px 22px}QMenu::item:selected{background:#e6e0ff;color:#332268}QMenu::item:disabled{color:#8991a2}QMenu::separator{height:1px;background:#dfe2ed;margin:5px 8px}") : QStringLiteral("QMenu{background:#191c28;color:#edf0fa;border:1px solid #41485f;padding:5px}QMenu::item{padding:8px 28px 8px 22px}QMenu::item:selected{background:#393052;color:#ffffff}QMenu::item:disabled{color:#79839c}QMenu::separator{height:1px;background:#363c50;margin:5px 8px}"));
            openAction->setText(en?QStringLiteral("Open NetPulse"):QStringLiteral("Открыть NetPulse"));
            connectAction->setText(en?QStringLiteral("Connect…"):QStringLiteral("Подключиться…"));
            disconnectAction->setText(en?QStringLiteral("Disconnect"):QStringLiteral("Отключиться"));
            reconnectAction->setText(en?QStringLiteral("Reconnect…"):QStringLiteral("Переподключиться…"));
            modes->setTitle(en?QStringLiteral("Connection mode"):QStringLiteral("Режим подключения"));
            serversAction->setText(en?QStringLiteral("Servers…"):QStringLiteral("Серверы…"));
            ipAction->setText(en?QStringLiteral("Check IP…"):QStringLiteral("Проверить IP…"));
            networkAction->setText(en?QStringLiteral("Network…"):QStringLiteral("Сеть…"));
            settingsAction->setText(en?QStringLiteral("Settings…"):QStringLiteral("Настройки…"));
            quitAction->setText(en?QStringLiteral("Quit"):QStringLiteral("Выход"));
        }
        const bool connected=!snapshot.value(QStringLiteral("engine_pid")).isNull();
        const bool working=backend.pending().contains(QStringLiteral("connect"))||backend.pending().contains(QStringLiteral("disconnect"))||backend.pending().contains(QStringLiteral("quick_network"));
        connectAction->setEnabled(backend.ready()&&!connected&&!working);disconnectAction->setEnabled(connected&&!working);reconnectAction->setEnabled(connected&&!working);modes->setEnabled(backend.ready()&&!working);
        tunAction->setChecked(prefs.value(QStringLiteral("tun")).toBool());proxyAction->setChecked(prefs.value(QStringLiteral("system_proxy")).toBool());
        tray.setToolTip(QStringLiteral("NetPulse · ")+(connected?(en?QStringLiteral("Connected"):QStringLiteral("Подключено")):(en?QStringLiteral("Disconnected"):QStringLiteral("Отключено"))));
    };
    QObject::connect(&backend,&Backend::dataChanged,&app,updateTray);
    QObject::connect(&backend,&Backend::pendingChanged,&app,updateTray);
    QObject::connect(app.styleHints(),&QStyleHints::colorSchemeChanged,&app,[&](Qt::ColorScheme){updateTray();});
    updateTray();
    // Keep the themed widget menu separate from the Windows native tray menu.
    // Re-polishing a native menu with submenus can introduce a platform menu cycle in Qt 6.8.
    QObject::connect(&tray, &QSystemTrayIcon::activated, window, [window, &menu](QSystemTrayIcon::ActivationReason reason) {
        if (reason == QSystemTrayIcon::Context) { menu.popup(QCursor::pos()); return; }
        if (reason == QSystemTrayIcon::Trigger || reason == QSystemTrayIcon::DoubleClick) { window->show(); window->raise(); window->requestActivate(); }
    });
    if (!smoke) tray.show();
    QObject::connect(&backend, &Backend::quitReady, &app, [&] { app.exit(smoke && qmlIssues ? 5 : 0); });
    QSet<int> capturedPages;
    QSet<QString> capturedFrames;
    bool activityViewportsFit = true;
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
            if (!capturedPages.contains(page) && backend.ready() && !backend.data().value(QStringLiteral("runtime_preparing")).toBool()) {
                capturedPages.insert(page);
                window->grabWindow().save(output + QStringLiteral("/page-%1.png").arg(page));
            }
            const auto frame=window->property("smokeFrame").toString();
            if(backend.ready()&&!backend.data().value(QStringLiteral("runtime_preparing")).toBool()&&!capturedFrames.contains(frame)){capturedFrames.insert(frame);if(page==6)activityViewportsFit=activityViewportsFit&&window->property("activityFitsViewport").toBool();window->grabWindow().save(output+QStringLiteral("/frame-%1.png").arg(frame));window->setProperty("smokeCaptured",frame);}
            if (!window->property("smokeFinished").toBool()) return;
            check->stop();
            window->grabWindow().save(output + QStringLiteral("/window.png"));
            const QJsonObject result{{QStringLiteral("result"), qmlIssues ? QStringLiteral("failed") : QStringLiteral("passed")}, {QStringLiteral("qt"), QString::fromLatin1(qVersion())},
                {QStringLiteral("version"), QCoreApplication::applicationVersion()}, {QStringLiteral("pages"), window->property("smokePages").toInt()},
                {QStringLiteral("frames"),capturedFrames.size()}, {QStringLiteral("activity_viewports_fit"),activityViewportsFit}, {QStringLiteral("core_log_rows"),window->property("smokeCoreRows").toInt()}, {QStringLiteral("qml_issues"), qmlIssues}, {QStringLiteral("qml_messages"), QJsonArray::fromStringList(qmlMessages)},
                {QStringLiteral("network_changed"), false}, {QStringLiteral("webview_used"), false}};
            QFile file(output + QStringLiteral("/result.json")); if (file.open(QIODevice::WriteOnly)) file.write(QJsonDocument(result).toJson());
            backend.quit();
        }); check->start();
    }
    if(parser.isSet(QStringLiteral("isolated")))window->setTitle(QStringLiteral("NetPulse"));
    QTimer::singleShot(0, &backend, &Backend::start);
    return app.exec();
}
