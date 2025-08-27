#pragma once

#include <QWidget>
#include <QtNetwork/QNetworkAccessManager>

class QTimer;

class Journal : public QWidget {
  Q_OBJECT

public:
  explicit Journal(const QString &nodeId, QNetworkAccessManager *mgr, QWidget *parent = nullptr);

private:
  QString m_nodeId;
  QNetworkAccessManager *m_mgr;
  QTimer *m_saveTimer{nullptr};
  class QMarkdownTextEdit *m_editor{nullptr};
};
