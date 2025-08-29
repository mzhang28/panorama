#pragma once

#include <QString>
#include <functional>
#include <map>
#include <string>

class QWidget;

class PluginManager {
public:
  using UrlHandler = std::function<QWidget *(const QString &url)>;

  static PluginManager &instance();

  void registerUrlHandler(const std::string &prefix, UrlHandler handler);
  QWidget *handleUrl(const QString &url);

private:
  std::map<std::string, UrlHandler> urlHandlers;
};
