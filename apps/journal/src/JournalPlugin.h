#pragma once

#include "PluginInterface.h"

class JournalPlugin : public PluginInterface, public QObject {
  Q_OBJECT
  Q_PLUGIN_METADATA(IID PluginInterface_iid)
  Q_INTERFACES(PluginInterface)

public:
  JournalPlugin();
};
