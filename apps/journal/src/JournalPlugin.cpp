#include "PluginInterface.h"
#include "PluginRegistrar.h"

class JournalPlugin : public PluginInterface {
public:
    JournalPlugin() {
        PluginRegistrar::registerPlugin();
    }
};
