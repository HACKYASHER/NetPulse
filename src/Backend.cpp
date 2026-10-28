#include "Backend.h"
#include <QApplication>
#include <QClipboard>
#include <QDesktopServices>
#include <QDir>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMessageBox>
#include <QSet>
#include <QUrl>

Backend::Backend(QString directory, bool isolated, QObject* parent)
    : QObject(parent), m_directory(std::move(directory)), m_isolated(isolated) {
    connect(&m_process, &QProcess::readyReadStandardOutput, this, &Backend::readOutput);
    // Private controller stderr is deliberately not written into the UI log.
    connect(&m_process, &QProcess::readyReadStandardError, this, [this] { m_process.readAllStandardError(); });
    connect(&m_process, &QProcess::started, this, [this] { refresh(); m_poll.start(); });
    connect(&m_process, &QProcess::errorOccurred, this, [this](QProcess::ProcessError error) {
        if (error == QProcess::FailedToStart)
            setError(QStringLiteral("Не удалось запустить netpulsed.exe. Проверьте комплектность приложения."));
    });
    connect(&m_process, &QProcess::finished, this, [this](int, QProcess::ExitStatus) {
        m_poll.stop();
        const auto pendingRequests = m_pending;
        m_pending.clear(); emit pendingChanged();
        for (auto i = pendingRequests.cbegin(); i != pendingRequests.cend(); ++i)
            emit completed(QString::number(i.key()), i.value(), {},
                {{QStringLiteral("code"), QStringLiteral("HostStopped")}, {QStringLiteral("message"), QStringLiteral("Сетевой контроллер остановлен")}});
        if (m_quitting) emit quitReady();
        else { m_ready = false; emit readyChanged(); setError(QStringLiteral("Сетевой контроллер завершился. Перезапустите NetPulse; журнал восстановления сохранён.")); }
    });
    m_poll.setInterval(2000);
    connect(&m_poll, &QTimer::timeout, this, [this] {
        if (m_ready && !m_quitting && !m_pending.values().contains(QStringLiteral("snapshot")))
            request(QStringLiteral("snapshot"));
    });
}

Backend::~Backend() {
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
    const quint64 id = m_nextId++;
    const QJsonObject request{{QStringLiteral("id"), qint64(id)}, {QStringLiteral("method"), method},
        {QStringLiteral("params"), QJsonObject::fromVariantMap(params)}};
    QByteArray bytes = QJsonDocument(request).toJson(QJsonDocument::Compact);
    if (bytes.size() > 12 * 1024 * 1024 - 1) { setError(QStringLiteral("Слишком большой запрос")); return {}; }
    bytes.append('\n');
    m_pending.insert(id, method); emit pendingChanged();
    m_process.write(bytes);
    bytes.fill('\0');
    return QString::number(id);
}
void Backend::refresh() {
    if (!m_pending.values().contains(QStringLiteral("bootstrap"))) request(QStringLiteral("bootstrap"));
}
void Backend::quit() {
    if (m_quitting) return;
    if (m_process.state() == QProcess::NotRunning) { emit quitReady(); return; }
    m_quitting = true; m_poll.stop();
    if (request(QStringLiteral("quit_application")).isEmpty()) { m_quitting = false; m_poll.start(); }
}
void Backend::readOutput() {
    m_buffer += m_process.readAllStandardOutput();
    if (m_buffer.size() > 16 * 1024 * 1024) { setError(QStringLiteral("Ответ контроллера превышает лимит")); m_process.closeWriteChannel(); return; }
    while (true) {
        const auto end = m_buffer.indexOf('\n'); if (end < 0) break;
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
            if (name == QStringLiteral("runtime-prepared") || name == QStringLiteral("subscriptions-updated") || name == QStringLiteral("automatic-selection") || name == QStringLiteral("mtu-tuning-finished")) refresh();
            if (name == QStringLiteral("host-error")) setError(payload.toMap().value(QStringLiteral("message")).toString());
            continue;
        }
        const quint64 id = quint64(response.value(QStringLiteral("id")).toInteger());
        if (!m_pending.contains(id)) continue;
        const auto method = m_pending.take(id); emit pendingChanged();
        const auto result = response.value(QStringLiteral("result")).toVariant();
        const auto error = response.value(QStringLiteral("error")).toObject().toVariantMap();
        if (!error.isEmpty()) {
            setError(error.value(QStringLiteral("message")).toString());
            if (method == QStringLiteral("quit_application")) { m_quitting = false; m_poll.start(); }
        } else if (method == QStringLiteral("bootstrap")) {
            m_data = result.toMap(); emit dataChanged();
            if (!m_ready) { m_ready = true; emit readyChanged(); }
        } else if (method == QStringLiteral("snapshot") || method == QStringLiteral("connect") || method == QStringLiteral("disconnect")) {
            m_data.insert(QStringLiteral("snapshot"), result); emit dataChanged();
        }
        emit completed(QString::number(id), method, result, error);
        if (method == QStringLiteral("quit_application") && error.isEmpty()) { m_process.closeWriteChannel(); continue; }
        static const QSet<QString> changes{QStringLiteral("connect"), QStringLiteral("disconnect"), QStringLiteral("import_text"), QStringLiteral("import_clipboard"), QStringLiteral("import_file"),
            QStringLiteral("delete_profile"), QStringLiteral("add_subscription"), QStringLiteral("save_subscription"), QStringLiteral("delete_subscription"),
            QStringLiteral("update_subscription"), QStringLiteral("save_preferences"), QStringLiteral("quick_network"), QStringLiteral("quick_ui"),
            QStringLiteral("select_profile"), QStringLiteral("save_profile_json"), QStringLiteral("save_profile_transport"), QStringLiteral("manage_service"),
            QStringLiteral("restore_data"), QStringLiteral("activate_core_bundle"), QStringLiteral("update_core"), QStringLiteral("clear_logs"), QStringLiteral("clear_file_logs")};
        if (error.isEmpty() && changes.contains(method)) refresh();
    }
}
void Backend::setError(const QString& message) { m_error = message; emit errorChanged(); }
void Backend::clearError() { m_error.clear(); emit errorChanged(); }
void Backend::copyText(const QString& text) { QApplication::clipboard()->setText(text); }
QString Backend::clipboardText() const { return QApplication::clipboard()->text(); }
void Backend::openDataDirectory() { QDesktopServices::openUrl(QUrl::fromLocalFile(m_directory)); }
void Backend::openInstallationDirectory() { QDesktopServices::openUrl(QUrl::fromLocalFile(QCoreApplication::applicationDirPath())); }
void Backend::openProxySettings() { QDesktopServices::openUrl(QUrl(QStringLiteral("ms-settings:network-proxy"))); }
void Backend::confirmDelete(const QString& method, const QVariantMap& params, const QString& label) {
    static const QSet<QString> allowed{QStringLiteral("delete_profile"), QStringLiteral("delete_subscription"), QStringLiteral("clear_logs"), QStringLiteral("clear_file_logs"), QStringLiteral("delete_local_data")};
    if (!allowed.contains(method)) return;
    if (QMessageBox::question(nullptr, QStringLiteral("NetPulse"), label, QMessageBox::Yes | QMessageBox::No, QMessageBox::No) == QMessageBox::Yes) request(method, params);
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
