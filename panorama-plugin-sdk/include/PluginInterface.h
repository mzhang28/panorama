#pragma once

#include "HostContext.h"
#include <QString>
#include <QObject>

// Pure C++ plugin interface. We deliberately avoid Q_OBJECT here so the
// interface is header-only and does not require a moc-generated translation
// unit in the plugin-sdk. Implementations should inherit from QObject and
// implement this interface.
class PluginInterface {
public:
  virtual ~PluginInterface() = default;
  virtual QString name() const = 0;
  virtual QString version() const = 0;

  // Handle an incoming URL. The HostContext may be used to request services
  // from the host. "data" is an optional opaque payload (may be nullptr).
  virtual void handleUrl(HostContext *ctx, QString &url, QObject *data = nullptr) const = 0;
};

#define PluginInterface_iid "io.panorama.PluginInterface"
Q_DECLARE_INTERFACE(PluginInterface, PluginInterface_iid)
