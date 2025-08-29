#include "PluginManager.h"

#include <QString>

PluginManager &PluginManager::instance() {
  static PluginManager instance;
  return instance;
}

void PluginManager::registerUrlHandler(const std::string &prefix,
                                       UrlHandler handler) {
  urlHandlers[prefix] = handler;
}

QWidget *PluginManager::handleUrl(const QString &url) {
  for (const auto &entry : urlHandlers) {
    const std::string &prefix = entry.first;
    const UrlHandler &handler = entry.second;
    if (url.startsWith(QString::fromStdString(prefix))) {
      return handler(url);
    }
  }
  return nullptr;
}
