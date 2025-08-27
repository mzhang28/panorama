#include <QLabel>
#include <QHBoxLayout>
#include <QTextEdit>
#include <QToolBar>
#include <QVBoxLayout>

#include "Journal.h"
#include "qmarkdowntextedit.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTimer>
#include <QUrl>

Journal::Journal(const QString &nodeId, QNetworkAccessManager *mgr,
                 QWidget *parent)
    : QWidget(parent), m_nodeId(nodeId), m_mgr(mgr) {
  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(0);

  QString title = QString("Journal Entry %1").arg(nodeId);
  QLabel *label = new QLabel(title, this);
  // Header: title on the left, save status on the right
  QHBoxLayout *header = new QHBoxLayout();
  header->setContentsMargins(0, 0, 0, 0);
  header->addWidget(label);
  header->addStretch();
  m_statusLabel = new QLabel(tr("Saved"), this);
  m_statusLabel->setStyleSheet("QLabel { color: #0a0; padding-left:8px; padding-right:8px; }");
  header->addWidget(m_statusLabel);
  layout->addLayout(header);

  m_editor = new QMarkdownTextEdit();
  layout->addWidget(m_editor);

  m_saveTimer = new QTimer(this);
  m_saveTimer->setSingleShot(true);
  connect(m_saveTimer, &QTimer::timeout, this, [this]() {
    // On timeout, send mutation to backend
    if (!m_mgr)
      return;
    QString text = m_editor->toPlainText();
    // Indicate save in-flight
    if (m_statusLabel) {
      m_statusLabel->setText(tr("Saving..."));
      m_statusLabel->setStyleSheet("QLabel { color: #e65a00; padding-left:8px; padding-right:8px; }");
    }
    // Build GraphQL mutation with variables.
    QString mutation = QString(
        "mutation($nodeId: String!, $value: String!) { setField(nodeId: "
        "$nodeId, app: \"journal\", field: \"title\", value: $value) }");

    QJsonObject vars;
    vars.insert("nodeId", QJsonValue(m_nodeId));
    vars.insert("value", QJsonValue(text));

    QJsonObject body;
    body.insert("query", mutation);
    body.insert("variables", vars);
    QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
    req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");

    QJsonDocument doc(body);
    QByteArray data = doc.toJson();
    auto *reply = m_mgr->post(req, data);
    connect(reply, &QNetworkReply::finished, reply, [reply, this]() {
      // Read response and ignore for now
      QByteArray resp = reply->readAll();
      reply->deleteLater();
      // Notify that the content was successfully saved.
      if (m_statusLabel) {
        m_statusLabel->setText(tr("Saved"));
        m_statusLabel->setStyleSheet("QLabel { color: #0a0; padding-left:8px; padding-right:8px; }");
      }
    });
  });

  connect(m_editor, &QMarkdownTextEdit::textChanged, this, [this]() {
    // Ignore events while programmatically loading content
    if (m_loading) return;
    // Notify unsaved state and debounce saves
    if (m_statusLabel) {
      m_statusLabel->setText(tr("Unsaved"));
      m_statusLabel->setStyleSheet("QLabel { color: #c00; padding-left:8px; padding-right:8px; font-weight: bold; }");
    }
    m_saveTimer->start(1000);
  });

  // On load, fetch the latest content for this node
  if (m_mgr) {
    QString query = QString(
        "query($id: String!) { nodes(id: $id) { id journal { title } } }");
    QJsonObject vars2;
    vars2.insert("id", QJsonValue(m_nodeId));
    QJsonObject body2;
    body2.insert("query", query);
    body2.insert("variables", vars2);
    QNetworkRequest req2(QUrl("http://127.0.0.1:4141/graphql"));
    req2.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
    QJsonDocument doc2(body2);
    QByteArray data2 = doc2.toJson();
    auto *reply2 = m_mgr->post(req2, data2);
    connect(reply2, &QNetworkReply::finished, reply2, [reply2, this]() {
      QByteArray resp = reply2->readAll();
      qDebug() << resp;
      reply2->deleteLater();
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
                if (journal.contains("title")) {
                  QString title = journal.value("title").toString();
                  m_loading = true;
                  m_editor->setPlainText(title);
                  m_loading = false;
                  // Loaded content is saved on disk; clear unsaved indicator
                  if (m_statusLabel) {
                    m_statusLabel->setText(tr("Saved"));
                    m_statusLabel->setStyleSheet("QLabel { color: #0a0; padding-left:8px; padding-right:8px; }");
                  }
                }
              }
            }
          }
        }
      }
    });
  }
}
