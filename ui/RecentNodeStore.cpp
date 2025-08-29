#include "RecentNodeStore.h"

#include <QDebug>

RecentNodeStore &RecentNodeStore::instance() {
  static RecentNodeStore _instance;
  return _instance;
}

RecentNodeStore::RecentNodeStore() {
  // No app-specific subscriptions here. App plugins may connect their
  // internal stores to RecentNodeStore to update recents when relevant.
}

void RecentNodeStore::addOrUpdate(const QString &nodeId, const QString &title, const QString &snippet) {
  // remove existing entry if present
  for (auto it = m_entries.begin(); it != m_entries.end(); ++it) {
    if (it->nodeId == nodeId) {
      m_entries.erase(it);
      break;
    }
  }
  RecentNodeEntry e;
  e.nodeId = nodeId;
  e.title = title;
  e.snippet = snippet;
  e.lastModified = QDateTime::currentDateTime();
  m_entries.push_front(e);
  while ((int)m_entries.size() > kMaxEntries)
    m_entries.pop_back();
  emit updated();
}
