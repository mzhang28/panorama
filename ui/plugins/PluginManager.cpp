#include "PluginManager.h"

#include <QString>
#include <QDebug>

PluginManager &PluginManager::instance() {
  static PluginManager instance;
  return instance;
}

void PluginManager::registerUrlHandler(const std::string &pattern,
                                       UrlHandler handler) {
  QRegularExpression rx(QString::fromStdString(pattern));
  if (!rx.isValid()) {
    qWarning() << "Invalid regex pattern registered:" << QString::fromStdString(pattern);
    return;
  }
  Entry e;
  e.regex = rx;
  e.handler = handler;
  e.hasHandler = true;
  qDebug() << "Registered URL handler pattern:" << QString::fromStdString(pattern);
  entries.push_back(std::move(e));
}

void PluginManager::registerUrlPattern(const std::string &pattern) {
  QRegularExpression rx(QString::fromStdString(pattern));
  if (!rx.isValid()) {
    qWarning() << "Invalid regex pattern registered:" << QString::fromStdString(pattern);
    return;
  }
  Entry e;
  e.regex = rx;
  e.hasHandler = false;
  qDebug() << "Registered URL pattern (no handler):" << QString::fromStdString(pattern);
  entries.push_back(std::move(e));
}

QWidget *PluginManager::handleUrl(const QString &url) {
  // Iterate registered entries and attempt to match their regexes. If an
  // entry has a handler and matches the URL, dispatch to it.
  for (const auto &entry : entries) {
    if (entry.regex.match(url).hasMatch()) {
      if (entry.hasHandler && entry.handler) {
        return entry.handler(url);
      }
      // If we matched a pattern with no handler, continue searching other
      // entries (don't short-circuit) because the pattern list may be used
      // for discovery only.
    }
  }
  return nullptr;
}
