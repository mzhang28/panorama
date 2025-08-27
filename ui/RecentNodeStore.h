#pragma once

#include <QObject>
#include <list>
#include <QString>
#include <QDateTime>

struct RecentNodeEntry {
  QString nodeId;
  QString title;
  QString snippet;
  QDateTime lastModified;
};

class RecentNodeStore : public QObject {
  Q_OBJECT

public:
  static RecentNodeStore &instance();

  using const_iterator = std::list<RecentNodeEntry>::const_iterator;
  const std::list<RecentNodeEntry> &entries() const { return m_entries; }

  // Add or update a recent entry (moves it to the front)
  void addOrUpdate(const QString &nodeId, const QString &title, const QString &snippet = QString());

signals:
  void updated();

private:
  RecentNodeStore();
  std::list<RecentNodeEntry> m_entries;
  const int kMaxEntries = 200;
};
