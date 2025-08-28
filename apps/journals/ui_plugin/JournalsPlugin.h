#pragma once

#include <QObject>
#include "../../../../ui/plugins/PluginInterface.h"

class JournalsPlugin : public PluginInterface {
    Q_OBJECT
    Q_PLUGIN_METADATA(IID PluginInterface_iid)
    Q_INTERFACES(PluginInterface)
public:
    explicit JournalsPlugin(QObject* parent = nullptr) : PluginInterface(parent) {}
    ~JournalsPlugin() override = default;

    QStringList availableWidgetTypes() override;
    QWidget* createWidget(const QString& type, QWidget* parent = nullptr) override;
};
