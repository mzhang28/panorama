#pragma once

#include <QObject>

class PluginInterface : public QObject {
  Q_OBJECT

public:
  virtual QString name() const = 0;
};

#define PluginInterface_iid "io.panorama.PluginInterface"
Q_DECLARE_INTERFACE(PluginInterface, PluginInterface_iid)
