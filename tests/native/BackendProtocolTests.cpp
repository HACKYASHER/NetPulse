#include "Backend.h"
#include <QtTest/QTest>
#include <QtTest/QSignalSpy>
#include <QJsonDocument>
#include <QJsonObject>
#include <QWindow>
#include <QFileDialog>
#include <QMessageBox>
#include <QTimer>
#include <QApplication>
#include <QDateTime>

class BackendProtocolTests final : public QObject {
    Q_OBJECT
private slots:
    void failedRestartsCannotReuseThePreviousHealthyRun() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        backend.m_lastStartMs = QDateTime::currentMSecsSinceEpoch() - 121000;
        backend.m_buffer = "partial old reply";
        for (int attempt = 1; attempt <= 3; ++attempt) {
            backend.handleProcessFinished();
            QCOMPARE(backend.m_restartAttempts, attempt);
            QCOMPARE(backend.m_lastStartMs, qint64(0));
            QVERIFY(backend.m_restart.isActive());
            QVERIFY(backend.m_buffer.isEmpty());
            backend.m_restart.stop();
        }
        backend.handleProcessFinished();
        QCOMPARE(backend.m_restartAttempts, 3);
        QVERIFY(!backend.m_restart.isActive());
        backend.m_quitting = true;
        backend.handleProcessFinished();
        QVERIFY(!backend.m_restart.isActive());
    }
    void fileDialogBelongsToTheClientWindowAndCancelsWithoutARequest() {
        Backend backend(QStringLiteral("unused-test-profile"),true);
        QWindow window;backend.setMainWindow(&window);
        auto* dialog=backend.createFileDialog(QStringLiteral("export_routing"),{});
        QCOMPARE(dialog->windowModality(),Qt::WindowModal);
        QCOMPARE(dialog->windowHandle()->transientParent(),&window);
        QCOMPARE(dialog->acceptMode(),QFileDialog::AcceptSave);
        QCOMPARE(dialog->defaultSuffix(),QStringLiteral("json"));
        delete dialog;
        QVERIFY(backend.m_pending.isEmpty());
    }
    void cancellingServerSwitchKeepsTheActiveServer() {
        Backend backend(QStringLiteral("unused-test-profile"),true);
        QVariantMap active{{QStringLiteral("id"),QStringLiteral("old")}};
        QVariantMap snapshot{{QStringLiteral("engine_pid"),123},{QStringLiteral("active"),active}};
        backend.m_data.insert(QStringLiteral("snapshot"),snapshot);
        const auto original=backend.m_data;
        QTimer::singleShot(0,[]{for(auto* widget:QApplication::topLevelWidgets())if(auto* box=qobject_cast<QMessageBox*>(widget))box->reject();});
        QVERIFY(!backend.confirmServerChange(QStringLiteral("select_profile"),{{QStringLiteral("id"),QStringLiteral("new")}}));
        QCOMPARE(backend.m_data,original);QVERIFY(backend.m_pending.isEmpty());
        QVERIFY(backend.confirmServerChange(QStringLiteral("select_profile"),{{QStringLiteral("id"),QStringLiteral("old")}}));
    }
    void coalescedValidFramesHaveIndividualLimits() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        QSignalSpy events(&backend, &Backend::eventReceived);
        const QByteArray frame = QJsonDocument(QJsonObject{
            {QStringLiteral("event"), QStringLiteral("fixture")},
            {QStringLiteral("data"), QString(9 * 1024 * 1024, QChar(u'a'))}
        }).toJson(QJsonDocument::Compact) + '\n';
        backend.consumeOutput(frame + frame);
        QCOMPARE(events.count(), 2);
        QVERIFY(backend.errorMessage().isEmpty());
        QVERIFY(backend.m_buffer.isEmpty());
    }
    void splitUtf8WaitsForTheCompleteFrame() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        QSignalSpy events(&backend, &Backend::eventReceived);
        const QByteArray frame=QByteArray("{\"event\":\"fixture\",\"data\":\"")+QStringLiteral("Проверка").toUtf8()+"\"}\n";
        const auto split=frame.indexOf(QStringLiteral("П").toUtf8())+1;
        backend.consumeOutput(frame.left(split));
        QCOMPARE(events.count(), 0);
        backend.consumeOutput(frame.mid(split));
        QCOMPARE(events.count(), 1);
        QCOMPARE(events.first().at(1).toString(), QStringLiteral("Проверка"));
    }
    void oversizedUnterminatedFrameIsReleased() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        QSignalSpy events(&backend, &Backend::eventReceived);
        backend.consumeOutput(QByteArray(16 * 1024 * 1024 + 1, 'x'));
        QCOMPARE(events.count(), 0);
        QVERIFY(!backend.errorMessage().isEmpty());
        QVERIFY(backend.m_buffer.isEmpty());
    }
    void malformedFrameDoesNotConsumeTheFollowingResponse() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        QSignalSpy events(&backend, &Backend::eventReceived);
        backend.consumeOutput("{invalid\n{\"event\":\"fixture\",\"data\":7}\n");
        QCOMPARE(events.count(), 1);
        QCOMPARE(events.first().at(1).toInt(), 7);
        QVERIFY(!backend.errorMessage().isEmpty());
    }
    void backgroundPollingErrorsDoNotRaiseTheBanner() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        backend.m_pending.insert(5, QStringLiteral("snapshot"));
        backend.m_pending.insert(6, QStringLiteral("save_preferences"));
        backend.consumeOutput("{\"id\":5,\"error\":{\"code\":\"CoreUnavailable\",\"message\":\"poll failed\"}}\n"
                              "{\"id\":6,\"error\":{\"code\":\"RuntimePreparing\",\"message\":\"busy\"}}\n");
        QVERIFY(backend.errorMessage().isEmpty());
        QVERIFY(backend.m_pending.isEmpty());
        backend.m_pending.insert(7, QStringLiteral("import_text"));
        backend.consumeOutput("{\"id\":7,\"error\":{\"code\":\"InvalidLink\",\"message\":\"bad link\"}}\n");
        QCOMPARE(backend.errorMessage(), QStringLiteral("bad link"));
    }
    void deletionQueuesRefreshBehindAnOlderBootstrap() {
        Backend backend(QStringLiteral("unused-test-profile"), true);
        QSignalSpy errors(&backend, &Backend::errorChanged);
        backend.m_pending.insert(1, QStringLiteral("bootstrap"));
        backend.m_pending.insert(2, QStringLiteral("delete_subscription"));

        backend.consumeOutput("{\"id\":2,\"result\":null}\n");
        QVERIFY(backend.m_refreshQueued);
        QCOMPARE(errors.count(), 0);

        backend.consumeOutput("{\"id\":1,\"result\":{}}\n");
        QVERIFY(!backend.m_refreshQueued);
        QCOMPARE(errors.count(), 1); // The replacement request reached the stopped test controller.
    }
};
QTEST_MAIN(BackendProtocolTests)
#include "BackendProtocolTests.moc"
