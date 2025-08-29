#pragma once

#include <QObject>
#include <QtNetwork/QNetworkAccessManager>

#include "PluginInterface.h"

class JournalsPlugin : public PluginInterface {
  Q_OBJECT
  Q_PLUGIN_METADATA(IID PluginInterface_iid)
  Q_INTERFACES(PluginInterface)

public:
  explicit JournalsPlugin(QObject *parent = nullptr)
      : PluginInterface(parent) {}
  ~JournalsPlugin() override = default;

  bool handlesUrl(const QString &url) override;
  QWidget *createWidgetForUrl(HostContext *ctx, const QString &url,
                              QWidget *parent = nullptr) override;
};
