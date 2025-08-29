#pragma once

#include <QObject>
#include <QMap>
#include <QString>

class QNetworkAccessManager;
class QTimer;

class JournalStore : public QObject {
  Q_OBJECT
public:
  static JournalStore *instance();
  void setNetworkManager(QNetworkAccessManager *mgr);

  // Ensure a node's content is loaded (will emit contentChanged when ready)
  void ensureLoaded(const QString &nodeId);

  // Update the local content and schedule a save. Emits contentChanged immediately.
  void setContent(const QString &nodeId, const QString &content);

  // Read current cached content (may be empty)
  QString content(const QString &nodeId) const;
  // Read current cached title (may be empty)
  QString title(const QString &nodeId) const;

signals:
  void contentChanged(const QString &nodeId, const QString &content);
  void titleChanged(const QString &nodeId, const QString &title);
  void statusChanged(const QString &nodeId, const QString &status);

private slots:
  void onSaveTimerTimeout();

private:
  explicit JournalStore(QObject *parent = nullptr);
  static JournalStore *s_instance;

  QNetworkAccessManager *m_mgr{nullptr};
  QMap<QString, QString> m_contents;
  QMap<QString, QString> m_titles;
  QMap<QString, QTimer *> m_timers;
};
