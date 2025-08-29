#pragma once

#include <QString>
#include <QStringList>
#include <QRegularExpression>
#include <functional>
#include <vector>
#include <string>

class QWidget;

class PluginManager {
public:
  using UrlHandler = std::function<QWidget *(const QString &url)>;

  static PluginManager &instance();

  // Register a URL handler associated with a regex pattern (can be a simple
  // prefix). The pattern will be compiled into a QRegularExpression.
  void registerUrlHandler(const std::string &pattern, UrlHandler handler);

  // Register a URL pattern without a handler (used for precompiling regexes
  // discovered from the backend). This does not attach a widget-creating
  // handler; it's useful for discovery and precompilation.
  void registerUrlPattern(const std::string &pattern);

  QWidget *handleUrl(const QString &url);

private:
  struct Entry {
    QRegularExpression regex;
    UrlHandler handler;
    bool hasHandler = false;
  };

  std::vector<Entry> entries;
};
