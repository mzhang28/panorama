#pragma once

#include "PluginInterface.h"

class JournalPlugin : public PluginInterface {
  Q_OBJECT
  Q_PLUGIN_METADATA(IID PluginInterface_iid)
  Q_INTERFACES(PluginInterface)

public:
  JournalPlugin();
  QString name() const override;
  QString version() const override;
  QWidget *handleUrl(HostContext *ctx, const QString &url,
                     QObject *data = nullptr) override;
};
