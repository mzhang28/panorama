#include "JournalStore.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>

#include <QDebug>

#include <QNetworkAccessManager>

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
      emit titleChanged(nodeId, m_titles.value(nodeId));
    emit contentChanged(nodeId, m_contents.value(nodeId));
    emit statusChanged(nodeId, (m_contents.contains(nodeId) || m_titles.contains(nodeId))
                                   ? QString("saved")
                                   : QString("unsaved"));
    return;
  }

  // Query backend for node: request both title and content fields
  QString query = QString(
      "query($id: String!) { nodes(id: $id) { id journal { title content } } }");
  QJsonObject vars;
  vars.insert("id", QJsonValue(nodeId));
  QJsonObject body;
  body.insert("query", query);
  body.insert("variables", vars);
  QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
  req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  QJsonDocument doc(body);
  QByteArray data = doc.toJson();
  auto *reply = m_mgr->post(req, data);
  connect(reply, &QNetworkReply::finished, reply, [this, reply, nodeId]() {
    QByteArray resp = reply->readAll();
    reply->deleteLater();
    QJsonParseError err;
    QJsonDocument rdoc = QJsonDocument::fromJson(resp, &err);
    if (err.error == QJsonParseError::NoError && rdoc.isObject()) {
      QJsonObject obj = rdoc.object();
      if (obj.contains("data")) {
        QJsonObject data = obj.value("data").toObject();
        if (data.contains("nodes")) {
          QJsonArray nodes = data.value("nodes").toArray();
          if (!nodes.isEmpty()) {
            QJsonObject first = nodes.at(0).toObject();
            if (first.contains("journal")) {
              QJsonObject journal = first.value("journal").toObject();
              QString gotTitle;
              QString gotContent;
              if (journal.contains("title"))
                gotTitle = journal.value("title").toString();
              if (journal.contains("content"))
                gotContent = journal.value("content").toString();
              if (!gotTitle.isEmpty()) {
                m_titles.insert(nodeId, gotTitle);
                emit titleChanged(nodeId, gotTitle);
              }
              if (!gotContent.isEmpty()) {
                m_contents.insert(nodeId, gotContent);
                emit contentChanged(nodeId, gotContent);
              }
              if (!gotTitle.isEmpty() || !gotContent.isEmpty()) {
                emit statusChanged(nodeId, QString("saved"));
                return;
              }
            }
          }
        }
      }
    }
    // On failure, leave as empty but notify
    emit contentChanged(nodeId, m_contents.value(nodeId));
    emit statusChanged(nodeId, QString("unsaved"));
  });
}

void JournalStore::setContent(const QString &nodeId, const QString &content) {
  m_contents.insert(nodeId, content);
  emit contentChanged(nodeId, content);
  // restart or create timer for this node to debounce saves
  QTimer *t = nullptr;
  if (m_timers.contains(nodeId)) {
    t = m_timers.value(nodeId);
  } else {
    t = new QTimer(this);
    t->setSingleShot(true);
    // store node id in the timer's property so timeout handler can know
    t->setProperty("nodeId", nodeId);
    connect(t, &QTimer::timeout, this, &JournalStore::onSaveTimerTimeout);
    m_timers.insert(nodeId, t);
  }
  emit statusChanged(nodeId, QString("unsaved"));
  t->start(1000);
}

void JournalStore::onSaveTimerTimeout() {
  QTimer *t = qobject_cast<QTimer *>(sender());
  if (!t)
    return;
  QString nodeId = t->property("nodeId").toString();
  if (nodeId.isEmpty())
    return;
  // perform save
  if (!m_mgr) {
    emit statusChanged(nodeId, QString("error"));
    return;
  }

  emit statusChanged(nodeId, QString("saving"));

  // Persist content to the journal app's "content" field.
  QString mutation =
      QString("mutation($nodeId: String!, $value: String!) { setField(nodeId: "
              "$nodeId, app: \"journal\", field: \"content\", value: $value) }");

  QJsonObject vars;
  vars.insert("nodeId", QJsonValue(nodeId));
  vars.insert("value", QJsonValue(m_contents.value(nodeId)));
  QJsonObject body;
  body.insert("query", mutation);
  body.insert("variables", vars);
  QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
  req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  QJsonDocument doc(body);
  QByteArray data = doc.toJson();
  auto *reply = m_mgr->post(req, data);
  connect(reply, &QNetworkReply::finished, reply, [this, reply, nodeId]() {
    QByteArray resp = reply->readAll();
    reply->deleteLater();
    QJsonParseError err;
    QJsonDocument rdoc = QJsonDocument::fromJson(resp, &err);
    Q_UNUSED(rdoc);
    // For now, assume success if reply arrived. In future, inspect errors.
    emit statusChanged(nodeId, QString("saved"));
  });
}

