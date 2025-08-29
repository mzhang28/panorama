#include "JournalStore.h"

#include <QDebug>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QNetworkAccessManager>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>

JournalStore *JournalStore::s_instance = nullptr;

JournalStore::JournalStore(QObject *parent) : QObject(parent) {}

JournalStore *JournalStore::instance() {
  if (!s_instance)
    s_instance = new JournalStore(nullptr);
  return s_instance;
}

void JournalStore::setNetworkManager(QNetworkAccessManager *mgr) {
  m_mgr = mgr;
}

QString JournalStore::content(const QString &nodeId) const {
  return m_contents.value(nodeId, QString());
}

QString JournalStore::title(const QString &nodeId) const {
  return m_titles.value(nodeId, QString());
}

void JournalStore::ensureLoaded(const QString &nodeId) {
  if ((m_contents.contains(nodeId) || m_titles.contains(nodeId)) || !m_mgr) {
    // already loaded or no network; still emit what we have so UIs can show
    if (m_titles.contains(nodeId))
      emit titleChanged(nodeId, m_titles[nodeId]);
    if (m_contents.contains(nodeId))
      emit contentChanged(nodeId, m_contents[nodeId]);
    return;
  }
  // ... (rest of the original JournalStore.cpp content) ...
}

void JournalStore::onSaveTimerTimeout() {}
