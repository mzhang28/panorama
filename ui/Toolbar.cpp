#include "Toolbar.h"
#include "MainWindow.h"
#include "ads_globals.h"
#include <QWidget>

MainToolBar::MainToolBar(MainWindow *parent)
    : QToolBar("Main Toolbar", parent) {
  QAction *dailyNoteBtn = addAction("Daily Note");
  connect(dailyNoteBtn, &QAction::triggered, parent, [parent]() {
    parent->openUrl("/note", ads::DockWidgetArea::LeftDockWidgetArea);
  });
}
