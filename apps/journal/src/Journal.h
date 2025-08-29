#pragma once

#include "HostContext.h"
#include <QWidget>
#include <QtNetwork/QNetworkAccessManager>

class QTimer;
class QLabel;
class MarkdownEdit;

class Journal : public QWidget {
  Q_OBJECT

public:
  explicit Journal(const QString &nodeId, HostContext *ctx,
                   QWidget *parent = nullptr);

private:
  QString m_nodeId;
  QNetworkAccessManager *m_mgr{nullptr};
  HostContext *m_hostCtx{nullptr};
  class MarkdownEdit *m_editor{nullptr};
  bool m_loading{false};
  QLabel *m_statusLabel{nullptr};
};
