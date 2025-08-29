#pragma once

#include <QJsonDocument>
#include <QJsonObject>
#include <QObject>
#include <QUrl>
#include <QtNetwork/QNetworkAccessManager>
#include <QtNetwork/QNetworkReply>
#include <QtNetwork/QNetworkRequest>

class HostContext : public QObject {
  Q_OBJECT

public:
  explicit HostContext(QObject *parent = nullptr) : QObject(parent) {}
  ~HostContext() override {}

  // Provide the frontend's network manager so plugins can make requests to
  // the backend (e.g. GraphQL queries). The HostContext does not take
  // ownership of the manager.
  void setNetworkAccessManager(QNetworkAccessManager *mgr) { m_mgr = mgr; }
  QNetworkAccessManager *networkAccessManager() const { return m_mgr; }

  // Convenience helper to perform a GraphQL request. Returns the
  // QNetworkReply* so callers can connect to its finished() signal and read
  // results. If no network manager is configured, returns nullptr.
  QNetworkReply *graphqlRequest(const QString &query,
                                const QJsonObject &variables = QJsonObject()) {
    if (!m_mgr)
      return nullptr;
    QJsonObject payload;
    payload.insert("query", query);
    payload.insert("variables", variables);
    QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
    req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
    return m_mgr->post(req, QJsonDocument(payload).toJson());
  }

  virtual void openUrl(const QString &url) {}

private:
  QNetworkAccessManager *m_mgr{nullptr};
};
