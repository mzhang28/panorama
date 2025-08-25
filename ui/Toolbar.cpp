#include "Toolbar.h"
#include "MainWindow.h"
#include "ads_globals.h"
#include <QWidget>

MainToolBar::MainToolBar(MainWindow *parent)
    : QToolBar("Main Toolbar", parent) {
  // QAction *btn1 = addAction("Button 1");
  // QAction *btn2 = addAction("Button 2");
  // QAction *btn3 = addAction("Button 3");
  QAction *dailyNoteBtn = addAction("Daily Note");
  connect(dailyNoteBtn, &QAction::triggered, parent, [parent]() {
    parent->openUrl("/note", ads::DockWidgetArea::LeftDockWidgetArea);
  });

  // connect(btn1, &QAction::triggered, this,
  //         [this] { emit buttonClicked("Button 1"); });
  // connect(btn2, &QAction::triggered, this,
  //         [this] { emit buttonClicked("Button 2"); });
  // connect(btn3, &QAction::triggered, this,
  //         [this] { emit buttonClicked("Button 3"); });
}
