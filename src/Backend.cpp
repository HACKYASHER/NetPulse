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
            if (name == QStringLiteral("runtime-prepared") || name == QStringLiteral("subscriptions-updated") || name == QStringLiteral("automatic-selection")) refresh();
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
void Backend::confirmDelete(const QString& method, const QVariantMap& params, const QString& label) {
    static const QSet<QString> allowed{QStringLiteral("delete_profile"), QStringLiteral("delete_subscription"), QStringLiteral("clear_logs"), QStringLiteral("clear_file_logs"), QStringLiteral("delete_local_data")};
    if (!allowed.contains(method)) return;
    if (QMessageBox::question(nullptr, QStringLiteral("NetPulse"), label, QMessageBox::Yes | QMessageBox::No, QMessageBox::No) == QMessageBox::Yes) request(method, params);
}
