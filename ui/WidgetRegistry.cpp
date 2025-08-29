#include "WidgetRegistry.h"
#include "ads_globals.h"
#include "DockManager.h"

WidgetRegistry* WidgetRegistry::instance() {
    static WidgetRegistry inst;
    return &inst;
}

void WidgetRegistry::registerWidget(const QString &id, ads::CDockWidget* w) {
    QMutexLocker lk(&mtx);
    map.insert(id, w);
}

void WidgetRegistry::unregisterWidget(const QString &id) {
    QMutexLocker lk(&mtx);
    map.remove(id);
}

ads::CDockWidget* WidgetRegistry::findWidget(const QString &id) {
    QMutexLocker lk(&mtx);
    return map.value(id, nullptr);
}

