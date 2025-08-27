#include "FileView.h"

#include <QLabel>
#include <QVBoxLayout>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonArray>
#include <QNetworkRequest>
#include <QNetworkReply>

FileView::FileView(const QString &nodeId, QNetworkAccessManager *mgr, QWidget *parent)
    : QWidget(parent), m_nodeId(nodeId), m_mgr(mgr) {
  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(4);

  QLabel *title = new QLabel(QString("File %1").arg(nodeId), this);
  layout->addWidget(title);

  m_info = new QLabel(tr("Loading..."), this);
  layout->addWidget(m_info);

  if (!m_mgr) return;

  QString gql = QString("{ nodes(id: \"%1\") { id type created_at updated_at file { name sha256 size } } }")
                    .arg(nodeId);

  QJsonObject payload;
  payload.insert("query", gql);

  QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
  req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  QNetworkReply *reply = m_mgr->post(req, QJsonDocument(payload).toJson());
  connect(reply, &QNetworkReply::finished, this, [this, reply]() {
    if (reply->error() != QNetworkReply::NoError) {
      m_info->setText(tr("Error fetching file info"));
      reply->deleteLater();
      return;
    }
    QJsonDocument doc = QJsonDocument::fromJson(reply->readAll());
    reply->deleteLater();
    if (!doc.isObject()) return;
    QJsonObject obj = doc.object();
    QJsonObject data = obj.value("data").toObject();
    QJsonArray nodes = data.value("nodes").toArray();
    if (nodes.isEmpty()) {
      m_info->setText(tr("No node returned"));
      return;
    }
    QJsonObject node = nodes.first().toObject();
    QJsonObject file = node.value("file").toObject();
    QString name = file.value("name").toString();
    QString sha = file.value("sha256").toString();
    QString size = file.value("size").toString();
    QString created = node.value("created_at").toString();
    QString updated = node.value("updated_at").toString();

    QString text = QString("Name: %1\nSHA256: %2\nSize: %3\nCreated: %4\nUpdated: %5")
                       .arg(name)
                       .arg(sha)
                       .arg(size)
                       .arg(created)
                       .arg(updated);
    m_info->setText(text);
  });
}

