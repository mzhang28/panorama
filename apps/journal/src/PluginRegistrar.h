#pragma once

#include <QString>
#include <QDebug>
#include <QWidget>
#include "../../ui/plugins/PluginManager.h"
#include "Journal.h"

class PluginRegistrar {
public:
    static inline void registerPlugin() {
        // Local handler that creates the Journal widget for /journal/*
        static auto journalUrlHandler = [](const QString &url) -> QWidget* {
            if (url.startsWith("/journal/")) {
                QString id = url.mid(QString("/journal/").length());
                return new Journal(id, nullptr, nullptr); // TODO: pass network manager and parent
            }
            return nullptr;
        };
        qDebug() << "Journal plugin registering handler for /journal/";
        ui::PluginManager::instance().registerUrlHandler("^/journal/", journalUrlHandler);
    }
};
