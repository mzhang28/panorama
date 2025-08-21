#include <QCloseEvent>
#include <QLabel>
#include <QSettings>

#include "MainWindow.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  QSettings settings("mzhang", "panorama");
  restoreGeometry(settings.value("geometry").toByteArray());
  restoreState(settings.value("windowState").toByteArray());

  // Create the dock manager
  m_DockManager = new ads::CDockManager(this);

  // Example content label
  QLabel *label =
      new QLabel("Lorem ipsum dolor sit amet, consectetuer adipiscing elit.");
  label->setWordWrap(true);
  label->setAlignment(Qt::AlignTop | Qt::AlignLeft);

  // Create a dock widget with the label
  ads::CDockWidget *dockWidget = m_DockManager->createDockWidget("Label 1");
  dockWidget->setWidget(label);

  // Add the dock widget to the top dock widget area
  m_DockManager->addDockWidget(ads::TopDockWidgetArea, dockWidget);

  // Create the View menu and add toggle action
  m_menuView = menuBar()->addMenu("View");
  m_menuView->addAction(dockWidget->toggleViewAction());
}

MainWindow::~MainWindow() {
  // No ui pointer to delete
  delete m_DockManager;
}

void MainWindow::closeEvent(QCloseEvent *event) {
  QSettings settings("mzhang", "panorama");
  settings.setValue("geometry", saveGeometry());
  settings.setValue("windowState", saveState());
  QMainWindow::closeEvent(event);
}
