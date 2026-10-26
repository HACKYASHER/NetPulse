#pragma once
#include <QObject>
#include <QProcess>
#include <QVariantMap>
#include <QTimer>
#include <QMap>

class Backend final : public QObject {
    Q_OBJECT
    Q_PROPERTY(QVariantMap data READ data NOTIFY dataChanged)
    Q_PROPERTY(bool ready READ ready NOTIFY readyChanged)
    Q_PROPERTY(QStringList pending READ pending NOTIFY pendingChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorChanged)
    Q_PROPERTY(QString dataDirectory READ dataDirectory CONSTANT)
public:
    explicit Backend(QString directory, bool isolated, QObject* parent = nullptr);
    ~Backend() override;
    QVariantMap data() const { return m_data; }
    bool ready() const { return m_ready; }
    QStringList pending() const { return m_pending.values(); }
    QString errorMessage() const { return m_error; }
    QString dataDirectory() const { return m_directory; }
    Q_INVOKABLE QString request(const QString& method, const QVariantMap& params = {});
    Q_INVOKABLE void refresh();
    Q_INVOKABLE void quit();
    Q_INVOKABLE void clearError();
    Q_INVOKABLE QString readableText(const QString& value) const;
    Q_INVOKABLE void copyText(const QString& text);
    Q_INVOKABLE QString clipboardText() const;
    Q_INVOKABLE void openDataDirectory();
    Q_INVOKABLE void openProxySettings();
    Q_INVOKABLE void confirmDelete(const QString& method, const QVariantMap& params, const QString& label);
    void start();
signals:
    void dataChanged();
    void readyChanged();
    void pendingChanged();
    void errorChanged();
    void completed(const QString& id, const QString& method, const QVariant& result, const QVariantMap& error);
    void eventReceived(const QString& name, const QVariant& value);
    void quitReady();
private:
    void readOutput();
    void setError(const QString& message);
    QProcess m_process;
    QTimer m_poll;
    QByteArray m_buffer;
    QVariantMap m_data;
    QMap<quint64, QString> m_pending;
    QString m_directory, m_error;
    quint64 m_nextId = 1;
    bool m_ready = false, m_isolated = false, m_quitting = false;
};
