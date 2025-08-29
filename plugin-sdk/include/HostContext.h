#pragma once

#include <QObject>
#include <QString>

// HostContext is the minimal QObject-based bridge passed from the host to
// plugins. Plugins emit signals by calling the slots on this object or by
// connecting to its signals. Keeping this header simple and header-only
// avoids sharing UI implementation details.

class HostContext : public QObject {
  Q_OBJECT
public:
  explicit HostContext(QObject *parent = nullptr) : QObject(parent) {}

signals:
  void openUrlRequested(const QString &url, int area);
  void addOrUpdateRecentRequested(const QString &nodeId, const QString &title,
                                  const QString &snippet);

public slots:
  virtual void openUrl(const QString &url, int area) { emit openUrlRequested(url, area); }
  virtual void addOrUpdateRecent(const QString &nodeId, const QString &title,
                                 const QString &snippet) { emit addOrUpdateRecentRequested(nodeId, title, snippet); }
};

