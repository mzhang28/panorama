#pragma once

#include <QHash>
#include <QMutex>
#include <QString>

#include "DockWidget.h"

class WidgetRegistry {
public:
  static WidgetRegistry *instance();
  void registerWidget(const QString &id, ads::CDockWidget *w);
  void unregisterWidget(const QString &id);
  ads::CDockWidget *findWidget(const QString &id);

private:
  WidgetRegistry() = default;
  QMutex mtx;
  QHash<QString, ads::CDockWidget *> map;
};
