#include "RecentNodeStore.h"

#include "stores/JournalStore.h"
#include <QDebug>

RecentNodeStore &RecentNodeStore::instance() {
  static RecentNodeStore _instance;
  return _instance;
}

RecentNodeStore::RecentNodeStore() {
  // Listen to JournalStore updates and add entries when an item is edited.
  JournalStore *js = JournalStore::instance();
  if (js) {
    // When content changes, update recent list. If the server has a title,
    // prefer that; otherwise use the nodeId as the title for daily notes.
    connect(js, &JournalStore::contentChanged, this, [this, js](const QString &nodeId, const QString &content) {
      QString title = js->title(nodeId);
      if (title.isEmpty())
        title = nodeId;
      QString snippet = content;
      if (snippet.size() > 200)
        snippet = snippet.left(200) + QStringLiteral("...");
      addOrUpdate(nodeId, title, snippet);
    });

    // Also update if a title arrives later from the server
    connect(js, &JournalStore::titleChanged, this, [this, js](const QString &nodeId, const QString &title) {
      // find existing entry and update its title, or add a new one with empty snippet
      QString snippet = js->content(nodeId);
      if (snippet.size() > 200)
        snippet = snippet.left(200) + QStringLiteral("...");
      addOrUpdate(nodeId, title.isEmpty() ? nodeId : title, snippet);
    });
  }
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
