#pragma once

#include <QObject>
#include <QString>

#include "HostContext.h"

class PluginInterface : public QObject {
public:
  explicit PluginInterface(QObject *parent = nullptr) : QObject(parent) {}
  ~PluginInterface() {}

  virtual QString name() const = 0;
  virtual QString version() const = 0;
  // Create and return a widget for the given URL. Implementations should
  // return a newly-allocated QWidget (parent may be set by the caller).
  virtual QWidget *handleUrl(HostContext *ctx, const QString &url,
                             QObject *data = nullptr) = 0;
};

#define PluginInterface_iid "io.panorama.PluginInterface"
Q_DECLARE_INTERFACE(PluginInterface, PluginInterface_iid)
