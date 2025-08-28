#include "JournalsPlugin.h"
#include <QLabel>

QStringList JournalsPlugin::availableWidgetTypes() {
    return {"JournalViewer"};
}

QWidget* JournalsPlugin::createWidget(const QString& type, QWidget* parent) {
    if (type == "JournalViewer") {
        QLabel* l = new QLabel("Journal Viewer (plugin)", parent);
        return l;
    }
    return nullptr;
}
