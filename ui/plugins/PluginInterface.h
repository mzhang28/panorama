#pragma once

#include <QObject>
#include <QStringList>
#include <QWidget>

class PluginInterface : public QObject {
    Q_OBJECT
public:
    explicit PluginInterface(QObject* parent = nullptr) : QObject(parent) {}
    ~PluginInterface() override = default;

    // Return a list of widget type names this plugin provides
    virtual QStringList availableWidgetTypes() = 0;

    // Create a widget instance for the given type. Parent may be null.
    virtual QWidget* createWidget(const QString& type, QWidget* parent = nullptr) = 0;
};

#define PluginInterface_iid "io.panorama.PluginInterface"
Q_DECLARE_INTERFACE(PluginInterface, PluginInterface_iid)

