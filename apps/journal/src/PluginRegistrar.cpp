#include "PluginRegistrar.h"
#include "../../ui/plugins/PluginManager.h"
#include "Journal.h"

#include <QString>

static QWidget* journalUrlHandler(const QString &url) {
    if (url.startsWith("/journal/")) {
        QString id = url.mid(QString("/journal/").length());
        return new Journal(id, nullptr, nullptr); // TODO: pass network manager and parent
    }
    return nullptr;
}

void PluginRegistrar::registerPlugin() {
    ui::PluginManager::instance().registerUrlHandler("/journal/", journalUrlHandler);
}
