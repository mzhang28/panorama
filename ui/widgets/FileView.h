#pragma once

#include <QWidget>
#include <QtNetwork/QNetworkAccessManager>

class QLabel;

class FileView : public QWidget {
  Q_OBJECT
public:
  explicit FileView(const QString &nodeId, QNetworkAccessManager *mgr,
                    QWidget *parent = nullptr);

private:
  QString m_nodeId;
  QNetworkAccessManager *m_mgr{nullptr};
  QLabel *m_info{nullptr};
};
