#include "Backend.h"
#include <QApplication>
#include <QClipboard>
#include <QDateTime>
#include <QDesktopServices>
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMessageBox>
#include <QSet>
#include <QUrl>
#include <QFileDialog>
#include <QStandardPaths>
#include <QPushButton>
#ifdef Q_OS_WIN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <shellapi.h>
#endif

Backend::Backend(QString directory, bool isolated, QObject* parent)
    : QObject(parent), m_directory(std::move(directory)), m_isolated(isolated) {
    connect(&m_process, &QProcess::readyReadStandardOutput, this, &Backend::readOutput);
    // Private controller stderr is deliberately not written into the UI log.
    connect(&m_process, &QProcess::readyReadStandardError, this, [this] { m_process.readAllStandardError(); });
    connect(&m_process, &QProcess::started, this, [this] {
        m_lastStartMs = QDateTime::currentMSecsSinceEpoch();
        refresh(); m_poll.start();
    });
    connect(&m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart) {
            // FailedToStart does not emit finished(); treat it as an exit so restart logic runs.
            handleProcessFinished();
            if (!m_quitting) setError(QStringLiteral("Не удалось запустить netpulsed.exe. Проверьте комплектность приложения."));
        }
    });
    connect(&m_process, &QProcess::finished, this, [this](int, QProcess::ExitStatus) { handleProcessFinished(); });
    // A crashed controller is restarted automatically (bounded), instead of leaving
    // the UI permanently disconnected. The controller restores the System Proxy from
    // its recovery journal on startup.
    m_restart.setSingleShot(true);
    connect(&m_restart, &QTimer::timeout, this, [this] {
        if (!m_quitting && m_process.state() == QProcess::NotRunning) start();
    });
    m_poll.setInterval(2000);
    connect(&m_poll, &QTimer::timeout, this, [this] {
        if (m_ready && !m_quitting && !m_pending.values().contains(QStringLiteral("snapshot")))
            request(QStringLiteral("snapshot"));
    });
}

void Backend::handleProcessFinished() {
    m_poll.stop();
    // Drop any partial frame from the dead process so it cannot corrupt the next one.
    m_buffer.fill('\0'); m_buffer.clear();
    m_refreshQueued = false;
    const auto pendingRequests = m_pending;
    m_pending.clear(); emit pendingChanged();
    if (m_fileDialog) m_fileDialog->reject();
    for (auto i = pendingRequests.cbegin(); i != pendingRequests.cend(); ++i)
        emit completed(QString::number(i.key()), i.value(), {},
            {{QStringLiteral("code"), QStringLiteral("HostStopped")}, {QStringLiteral("message"), QStringLiteral("Сетевой контроллер остановлен")}});
    if (m_quitting) { emit quitReady(); return; }
    if (m_ready) { m_ready = false; emit readyChanged(); }
    // A controller that ran for a while before exiting earns a fresh retry budget.
    const auto startedAt = m_lastStartMs;
    m_lastStartMs = 0; // Failed starts must not reuse a previous healthy run's budget.
    if (startedAt && QDateTime::currentMSecsSinceEpoch() - startedAt > 120000) m_restartAttempts = 0;
    if (m_restartAttempts < 3) {
        ++m_restartAttempts; m_restarting = true;
        setError(QStringLiteral("Сетевой контроллер завершился; выполняется перезапуск (попытка %1 из 3).").arg(m_restartAttempts));
        m_restart.start(1000 * m_restartAttempts);
    } else {
        setError(QStringLiteral("Сетевой контроллер завершился. Перезапустите NetPulse; журнал восстановления сохранён."));
    }
}

void Backend::reportError(const QString& method, const QVariantMap& error) {
    // Background polling must not raise the global error banner: those requests are
    // retried automatically and their failures are already written to the log.
    static const QSet<QString> quiet{QStringLiteral("snapshot"), QStringLiteral("diagnostic_status"), QStringLiteral("current_diagnostic"),
        QStringLiteral("connection_analysis"), QStringLiteral("playground_processes"), QStringLiteral("application_icon"),
        QStringLiteral("check_client_update"), QStringLiteral("log_days"), QStringLiteral("log_page")};
    const auto code = error.value(QStringLiteral("code")).toString();
    if (quiet.contains(method)) return;
    // Preference saves blocked by a transient runtime operation are retried by the UI.
    if (method == QStringLiteral("save_preferences") && code == QStringLiteral("RuntimePreparing")) return;
    // A request cancelled because the controller stopped is reported once by handleProcessFinished().
    if (code == QStringLiteral("HostStopped")) return;
    setError(error.value(QStringLiteral("message")).toString());
}

Backend::~Backend() {
    m_quitting = true; // Destruction must never schedule a controller restart.
    m_restart.stop();
    if (m_fileDialog) { m_fileDialog->disconnect(this); delete m_fileDialog.data(); }
    if (m_process.state() != QProcess::NotRunning) {
        m_process.closeWriteChannel(); // EOF asks the controller to restore and disconnect.
        if (!m_process.waitForFinished(20000)) {
            m_process.kill();
            m_process.waitForFinished(3000); // The independent DPAPI recovery helper remains alive.
        }
    }
}
void Backend::start() {
    m_process.setProgram(QCoreApplication::applicationDirPath() + QStringLiteral("/netpulsed.exe"));
    QStringList args{QStringLiteral("--stdio"), m_directory};
    if (m_isolated) args << QStringLiteral("--isolated");
    m_process.setArguments(args);
    m_process.setWorkingDirectory(QCoreApplication::applicationDirPath());
#ifdef Q_OS_WIN
    m_process.setCreateProcessArgumentsModifier([](QProcess::CreateProcessArguments* a) {
        a->flags |= 0x08000000; // CREATE_NO_WINDOW; stdio remains private to QProcess.
    });
#endif
    m_process.start();
}
QString Backend::request(const QString& method, const QVariantMap& params) {
    if (m_process.state() != QProcess::Running) {
        setError(QStringLiteral("Сетевой контроллер недоступен")); return {};
    }
    if (m_pending.size() >= 24) {
        setError(QStringLiteral("Дождитесь завершения текущих операций")); return {};
    }
    if (!confirmServerChange(method, params)) return {};
    static const QSet<QString> fileMethods{QStringLiteral("import_file"), QStringLiteral("import_routing"), QStringLiteral("export_routing"), QStringLiteral("export_data"), QStringLiteral("restore_data"), QStringLiteral("choose_exe")};
    const bool pickFile=fileMethods.contains(method)&&!params.contains(QStringLiteral("path"));
    if (pickFile&&m_fileDialog) {setError(QStringLiteral("Завершите выбор файла в открытом окне"));return {};}
    const quint64 id = m_nextId++;
    m_pending.insert(id, method); emit pendingChanged();
    if (pickFile) beginFileDialog(id,method,params);
    else sendPendingRequest(id,method,params);
    return QString::number(id);
}
void Backend::sendPendingRequest(quint64 id,const QString& method,const QVariantMap& params) {
    if (!m_pending.contains(id)) return;
    if (m_process.state()!=QProcess::Running||(m_quitting&&method!=QStringLiteral("quit_application"))) {
        const QVariantMap error{{QStringLiteral("code"),QStringLiteral("HostStopped")},{QStringLiteral("message"),QStringLiteral("Сетевой контроллер недоступен")}};
        QTimer::singleShot(0,this,[this,id,error]{finishPendingRequest(id,{},error);});return;
    }
    const QJsonObject request{{QStringLiteral("id"), qint64(id)}, {QStringLiteral("method"), method},
        {QStringLiteral("params"), QJsonObject::fromVariantMap(params)}};
    QByteArray bytes = QJsonDocument(request).toJson(QJsonDocument::Compact);
    if (bytes.size() > 12 * 1024 * 1024 - 1) {
        bytes.fill('\0');
        QTimer::singleShot(0,this,[this,id]{finishPendingRequest(id,{},{{QStringLiteral("code"),QStringLiteral("FrameLimit")},{QStringLiteral("message"),QStringLiteral("Слишком большой запрос")}});});return;
    }
    bytes.append('\n');
    const auto size=bytes.size();
    const auto written=m_process.write(bytes);
    bytes.fill('\0');
    if(written!=size) {
        m_process.closeWriteChannel();
        QTimer::singleShot(0,this,[this,id]{finishPendingRequest(id,{},{{QStringLiteral("code"),QStringLiteral("HostWriteFailed")},{QStringLiteral("message"),QStringLiteral("Не удалось передать команду сетевому контроллеру")}});});
    }
}
void Backend::finishPendingRequest(quint64 id,const QVariant& result,const QVariantMap& error) {
    if(!m_pending.contains(id))return;
    const auto method=m_pending.take(id);emit pendingChanged();
    if(!error.isEmpty())reportError(method,error);
    if(method==QStringLiteral("quit_application")&&!error.isEmpty()){m_quitting=false;m_poll.start();}
    emit completed(QString::number(id),method,result,error);
}
void Backend::setMainWindow(QWindow* window) {m_mainWindow=window;}
void Backend::parentDialog(QWidget* dialog) {
    dialog->setWindowModality(Qt::WindowModal);
    if(m_mainWindow) {
        if(m_mainWindow->visibility()==QWindow::Minimized)m_mainWindow->showNormal();
        else m_mainWindow->show();
        m_mainWindow->raise();m_mainWindow->requestActivate();
        dialog->winId();
        if(dialog->windowHandle())dialog->windowHandle()->setTransientParent(m_mainWindow);
    }
}
QFileDialog* Backend::createFileDialog(const QString& method,const QVariantMap& params) {
    auto* dialog=new QFileDialog;
    dialog->setWindowTitle(QStringLiteral("NetPulse"));
    dialog->setSupportedSchemes({QStringLiteral("file")});
    dialog->setOption(QFileDialog::DontUseCustomDirectoryIcons);
    dialog->setDirectory(QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation));
    const bool save=method==QStringLiteral("export_data")||method==QStringLiteral("export_routing");
    dialog->setAcceptMode(save?QFileDialog::AcceptSave:QFileDialog::AcceptOpen);
    dialog->setFileMode(save?QFileDialog::AnyFile:QFileDialog::ExistingFile);
    if(method==QStringLiteral("choose_exe"))dialog->setNameFilter(QStringLiteral("Windows applications (*.exe)"));
    else if(method==QStringLiteral("import_file")&&params.value(QStringLiteral("qr")).toBool())dialog->setNameFilter(QStringLiteral("QR images (*.png *.jpg *.jpeg *.webp)"));
    else if(method==QStringLiteral("export_routing")||method==QStringLiteral("import_routing")) {
        dialog->setNameFilter(QStringLiteral("NetPulse routing (*.json)"));dialog->setDefaultSuffix(QStringLiteral("json"));
        if(save)dialog->selectFile(QStringLiteral("netpulse-routing.json"));
    } else if(method==QStringLiteral("export_data")) {
        const auto kind=params.value(QStringLiteral("kind")).toString();
        const bool backup=kind==QStringLiteral("backup");
        dialog->setDefaultSuffix(backup?QStringLiteral("npb"):QStringLiteral("json"));
        dialog->selectFile(backup?QStringLiteral("netpulse-backup.npb"):kind==QStringLiteral("logs")?QStringLiteral("netpulse-logs.json"):QStringLiteral("netpulse-settings.json"));
    } else dialog->setNameFilter(QStringLiteral("All files (*)"));
    parentDialog(dialog);return dialog;
}
void Backend::beginFileDialog(quint64 id,const QString& method,const QVariantMap& params) {
    auto* dialog=createFileDialog(method,params);m_fileDialog=dialog;
    connect(dialog,&QDialog::finished,this,[this,dialog,id,method,params](int result) {
        m_fileDialog=nullptr;
        if(m_pending.contains(id)) {
            if(result==QDialog::Accepted&&!dialog->selectedFiles().isEmpty()) {
                auto selected=params;selected.insert(QStringLiteral("path"),dialog->selectedFiles().first());
                sendPendingRequest(id,method,selected);
            } else finishPendingRequest(id,{});
        }
        dialog->deleteLater();
    });
    dialog->QDialog::open();
}
bool Backend::confirmServerChange(const QString& method,const QVariantMap& params) {
    if(method!=QStringLiteral("select_profile")&&method!=QStringLiteral("connect"))return true;
    const auto snapshot=m_data.value(QStringLiteral("snapshot")).toMap();
    const auto id=params.value(QStringLiteral("id")).toString();
    if(snapshot.value(QStringLiteral("engine_pid")).isNull()||id.isEmpty()||snapshot.value(QStringLiteral("active")).toMap().value(QStringLiteral("id")).toString()==id)return true;
    const bool en=m_data.value(QStringLiteral("preferences")).toMap().value(QStringLiteral("language")).toString()==QStringLiteral("en");
    QMessageBox box;box.setIcon(QMessageBox::Question);box.setWindowTitle(QStringLiteral("NetPulse"));box.setTextFormat(Qt::PlainText);
    box.setText(en?QStringLiteral("Switch server? Active application connections may be interrupted."):QStringLiteral("Переключить сервер? Активные соединения приложений могут быть прерваны."));
    auto* accept=box.addButton(en?QStringLiteral("Switch"):QStringLiteral("Переключить"),QMessageBox::AcceptRole);
    auto* cancel=box.addButton(en?QStringLiteral("Cancel"):QStringLiteral("Отмена"),QMessageBox::RejectRole);
    box.setDefaultButton(cancel);box.setEscapeButton(cancel);parentDialog(&box);box.exec();return box.clickedButton()==accept;
}
void Backend::refresh() {
    if (!m_pending.values().contains(QStringLiteral("bootstrap"))) request(QStringLiteral("bootstrap"));
}
void Backend::refreshAfterChange() {
    if (m_pending.values().contains(QStringLiteral("bootstrap"))) m_refreshQueued = true;
    else refresh();
}
void Backend::quit() {
    if (m_quitting) return;
    if (m_process.state() == QProcess::NotRunning) { m_quitting = true; m_restart.stop(); emit quitReady(); return; }
    m_quitting = true; m_poll.stop();
    if (request(QStringLiteral("quit_application")).isEmpty()) { m_quitting = false; m_poll.start(); }
}
void Backend::readOutput() {
    consumeOutput(m_process.readAllStandardOutput());
}
void Backend::consumeOutput(const QByteArray& bytes) {
    m_buffer += bytes;
    const auto rejectFrame = [this] {
        m_buffer.fill('\0'); m_buffer.clear();
        setError(QStringLiteral("Ответ контроллера превышает лимит"));
        if (m_process.state() == QProcess::Running) m_process.closeWriteChannel();
    };
    constexpr qsizetype maxFrame = 16 * 1024 * 1024;
    while (true) {
        const auto end = m_buffer.indexOf('\n');
        if (end < 0) { if (m_buffer.size() > maxFrame) rejectFrame(); break; }
        if (end > maxFrame) { rejectFrame(); return; }
        QByteArray line = m_buffer.left(end); m_buffer.remove(0, end + 1);
        QJsonParseError parseError;
        const QJsonObject response = QJsonDocument::fromJson(line, &parseError).object();
        line.fill('\0');
        if (parseError.error != QJsonParseError::NoError) { setError(QStringLiteral("Некорректный ответ контроллера")); continue; }
        if (response.contains(QStringLiteral("event"))) {
            const auto name = response.value(QStringLiteral("event")).toString();
            const auto payload = response.value(QStringLiteral("data")).toVariant();
            emit eventReceived(name, payload);
            if (name == QStringLiteral("connection-state")) {
                auto state = m_data.value(QStringLiteral("snapshot")).toMap();
                state.insert(QStringLiteral("state"), payload);
                m_data.insert(QStringLiteral("snapshot"), state); emit dataChanged();
            }
            if (name == QStringLiteral("runtime-prepared") || name == QStringLiteral("subscriptions-updated") || name == QStringLiteral("automatic-selection") || name == QStringLiteral("mtu-tuning-finished")) refreshAfterChange();
            if (name == QStringLiteral("host-error")) setError(payload.toMap().value(QStringLiteral("message")).toString());
            continue;
        }
        const quint64 id = quint64(response.value(QStringLiteral("id")).toInteger());
        if (!m_pending.contains(id)) continue;
        const auto method = m_pending.take(id); emit pendingChanged();
        const auto result = response.value(QStringLiteral("result")).toVariant();
        const auto error = response.value(QStringLiteral("error")).toObject().toVariantMap();
        if (!error.isEmpty()) {
            reportError(method, error);
            if (method == QStringLiteral("quit_application")) { m_quitting = false; m_poll.start(); }
        } else if (method == QStringLiteral("bootstrap")) {
            m_data = result.toMap(); emit dataChanged();
            if (!m_ready) { m_ready = true; emit readyChanged(); }
            if (m_restarting) { m_restarting = false; clearError(); }
        } else if (method == QStringLiteral("snapshot") || method == QStringLiteral("connect") || method == QStringLiteral("disconnect")) {
            m_data.insert(QStringLiteral("snapshot"), result); emit dataChanged();
        }
        emit completed(QString::number(id), method, result, error);
        if (method == QStringLiteral("quit_application") && error.isEmpty()) { m_process.closeWriteChannel(); continue; }
        static const QSet<QString> changes{QStringLiteral("connect"), QStringLiteral("disconnect"), QStringLiteral("import_text"), QStringLiteral("import_clipboard"), QStringLiteral("import_file"),
            QStringLiteral("delete_profile"), QStringLiteral("add_subscription"), QStringLiteral("save_subscription"), QStringLiteral("delete_subscription"),
            QStringLiteral("update_subscription"), QStringLiteral("save_preferences"), QStringLiteral("quick_network"), QStringLiteral("quick_ui"),
            QStringLiteral("select_profile"), QStringLiteral("save_profile_json"), QStringLiteral("save_profile_transport"), QStringLiteral("manage_service"),
            QStringLiteral("restore_data"), QStringLiteral("delete_local_data"), QStringLiteral("activate_core_bundle"), QStringLiteral("update_core"), QStringLiteral("clear_logs"), QStringLiteral("clear_file_logs")};
        if (error.isEmpty() && changes.contains(method)) refreshAfterChange();
        if (method == QStringLiteral("bootstrap") && m_refreshQueued) {
            m_refreshQueued = false;
            refresh();
        }
    }
}
void Backend::setError(const QString& message) { m_error = message; emit errorChanged(); }
void Backend::clearError() { m_error.clear(); emit errorChanged(); }
void Backend::copyText(const QString& text) { QApplication::clipboard()->setText(text); }
QString Backend::clipboardText() const { return QApplication::clipboard()->text(); }
void Backend::openDirectory(const QString& directory) {
#ifdef Q_OS_WIN
    SHELLEXECUTEINFOW info{};info.cbSize=sizeof(info);info.fMask=SEE_MASK_NOASYNC|SEE_MASK_FLAG_NO_UI;
    info.hwnd=m_mainWindow?reinterpret_cast<HWND>(m_mainWindow->winId()):nullptr;
    info.lpVerb=L"open";info.lpFile=reinterpret_cast<const wchar_t*>(directory.utf16());info.nShow=SW_SHOWNORMAL;
    if(!ShellExecuteExW(&info))setError(QStringLiteral("Не удалось открыть папку NetPulse. Проверьте доступ к папке."));
#else
    if(!QDesktopServices::openUrl(QUrl::fromLocalFile(directory)))setError(QStringLiteral("Не удалось открыть папку NetPulse"));
#endif
}
void Backend::openDataDirectory() {openDirectory(m_directory);}
void Backend::openLogDirectory() {const auto path=QDir(m_directory).filePath(QStringLiteral("logs"));if(QDir().mkpath(path))openDirectory(path);else setError(QStringLiteral("Не удалось открыть папку журналов"));}
void Backend::openInstallationDirectory() {openDirectory(QCoreApplication::applicationDirPath());}
void Backend::openProxySettings() { QDesktopServices::openUrl(QUrl(QStringLiteral("ms-settings:network-proxy"))); }
void Backend::confirmDelete(const QString& method, const QVariantMap& params, const QString& label) {
    static const QSet<QString> allowed{QStringLiteral("delete_profile"), QStringLiteral("delete_subscription"), QStringLiteral("clear_logs"), QStringLiteral("clear_file_logs"), QStringLiteral("delete_local_data")};
    if (!allowed.contains(method)) return;
    QMessageBox box(QMessageBox::Question,QStringLiteral("NetPulse"),label,QMessageBox::Yes|QMessageBox::No);
    box.setTextFormat(Qt::PlainText);box.setDefaultButton(QMessageBox::No);parentDialog(&box);
    if(box.exec()==QMessageBox::Yes)request(method,params);
}

QString Backend::readableText(const QString& value) const {
 static constexpr ushort table[128]={0x402,0x403,0x201a,0x453,0x201e,0x2026,0x2020,0x2021,0x20ac,0x2030,0x409,0x2039,0x40a,0x40c,0x40b,0x40f,0x452,0x2018,0x2019,0x201c,0x201d,0x2022,0x2013,0x2014,0xffff,0x2122,0x459,0x203a,0x45a,0x45c,0x45b,0x45f,0xa0,0x40e,0x45e,0x408,0xa4,0x490,0xa6,0xa7,0x401,0xa9,0x404,0xab,0xac,0xad,0xae,0x407,0xb0,0xb1,0x406,0x456,0x491,0xb5,0xb6,0xb7,0x451,0x2116,0x454,0xbb,0x458,0x405,0x455,0x457,0x410,0x411,0x412,0x413,0x414,0x415,0x416,0x417,0x418,0x419,0x41a,0x41b,0x41c,0x41d,0x41e,0x41f,0x420,0x421,0x422,0x423,0x424,0x425,0x426,0x427,0x428,0x429,0x42a,0x42b,0x42c,0x42d,0x42e,0x42f,0x430,0x431,0x432,0x433,0x434,0x435,0x436,0x437,0x438,0x439,0x43a,0x43b,0x43c,0x43d,0x43e,0x43f,0x440,0x441,0x442,0x443,0x444,0x445,0x446,0x447,0x448,0x449,0x44a,0x44b,0x44c,0x44d,0x44e,0x44f};
 QString text=value;
 for(int pass=0;pass<2;pass++) {
  if(text.count(QChar(0x420))+text.count(QChar(0x421))<2)break;
  QByteArray bytes;bool ok=true;
  for(QChar ch:text) {if(ch.unicode()<128){bytes.append(char(ch.unicode()));continue;}int found=-1;for(int i=0;i<128;i++)if(table[i]==ch.unicode()){found=i;break;}if(found<0){ok=false;break;}bytes.append(char(found+128));}
  if(!ok)break;
  const QString decoded=QString::fromUtf8(bytes);
  if(decoded.contains(QChar(0xfffd))||decoded.size()>=text.size())break;
  text=decoded;
 }
 return text;
}
