#pragma once

#include "HostContext.h"
#include <QObject>

class PluginInterface : public QObject {
  Q_OBJECT

public:
  virtual QString name() const = 0;
  virtual QString version() const = 0;

  virtual void handleUrl(HostContext *ctx, QString &url,
                         QObject *data = new QObject()) const = 0;
};

#define PluginInterface_iid "io.panorama.PluginInterface"
Q_DECLARE_INTERFACE(PluginInterface, PluginInterface_iid)
