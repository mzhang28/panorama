#pragma once

#include <QObject>
#include <QStringList>
#include <QWidget>

#include "HostContext.h"

// Minimal plugin SDK used by both the UI host and native UI plugins.
// This header-only interface exposes a small set of virtual methods and
// relies on Qt signals/slots on the HostContext for host<->plugin
// communication. Keep this file header-only to avoid heavy link-time
// dependencies between UI and apps.

class PluginInterface : public QObject {
public:
  explicit PluginInterface(QObject *parent = nullptr) : QObject(parent) {}

  // Optional: indicate whether the plugin can handle a given URL.
  virtual bool handlesUrl(const QString &url) { Q_UNUSED(url); return false; }

  // Optional: create a widget for the given URL. The plugin receives a
  // HostContext pointer so it can emit signals that the host will handle.
  virtual QWidget *createWidgetForUrl(HostContext *ctx, const QString &url,
                                      QWidget *parent = nullptr) {
    Q_UNUSED(ctx); Q_UNUSED(url); Q_UNUSED(parent); return nullptr;
  }

  // Optional: return a list of widget types this plugin provides.
  virtual QStringList availableWidgetTypes() { return {}; }
};

#define PluginInterface_iid "io.panorama.PluginInterface"
Q_DECLARE_INTERFACE(PluginInterface, PluginInterface_iid)

