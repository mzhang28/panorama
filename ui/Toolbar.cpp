#include "Toolbar.h"
#include "MainWindow.h"
#include "ads_globals.h"
#include <QWidget>

MainToolBar::MainToolBar(MainWindow *parent)
    : QToolBar("Main Toolbar", parent) {
  setMovable(false);
  setFloatable(false);

  QAction *dailyNoteBtn = addAction("Daily Note");
  connect(dailyNoteBtn, &QAction::triggered, parent, [parent]() {
    auto today =
        std::chrono::floor<std::chrono::days>(std::chrono::system_clock::now());
    auto url = std::format("/journal/{:%F}", today);
    parent->openUrl(url, ads::DockWidgetArea::LeftDockWidgetArea);
  });
}
