#pragma once

#include <QWidget>
#include <QtNetwork/QNetworkAccessManager>

#include "../../../../plugin-sdk/HostContext.h"

class QTimer;
class QLabel;
class MarkdownEdit;

class Journal : public QWidget {
  Q_OBJECT

public:
  explicit Journal(HostContext *ctx, const QString &nodeId,
                  QWidget *parent = nullptr);
signals:
  // Emitted when a panorama:// link is activated inside this journal view.
  void panoramaLinkActivated(const QString &href);

private:
  QString m_nodeId;
  HostContext *ctx;
  // QNetworkAccessManager *m_mgr{nullptr};
  class MarkdownEdit *m_editor{nullptr};
  bool m_loading{false};
  QLabel *m_statusLabel{nullptr};
};
