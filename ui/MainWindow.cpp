#include <QCloseEvent>
#include <QLabel>
#include <QSettings>

#include "DockWidget.h"
#include "MainWindow.h"
#include "Toolbar.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  QSettings settings("mzhang", "panorama");
  restoreGeometry(settings.value("geometry").toByteArray());
  restoreState(settings.value("windowState").toByteArray());

  MainToolBar *toolbar = new MainToolBar(this);
  addToolBar(toolbar);
  connect(toolbar, &MainToolBar::buttonClicked, this, [](const QString &name) {
    QMessageBox::information(nullptr, "Toolbar", name + " clicked");
  });

  // Create the dock manager
  m_DockManager = new ads::CDockManager(this);
  m_DockManager->restoreState(settings.value("dockManagerState").toByteArray());

  QLabel *label = new QLabel("Welcome to panorama!");
  label->setAlignment(Qt::AlignCenter);

  ads::CDockWidget *empty = new ads::CDockWidget("Empty");
  empty->setWidget(label);
  empty->setFeature(ads::CDockWidget::NoTab, true);
  m_DockManager->setCentralWidget(empty);

  // m_DockManager->setEmptyDockWidgetText("No dock widgets available");

  // // Create a dock widget with the label
  // ads::CDockWidget *dockWidget = m_DockManager->createDockWidget("Label 1");
  // dockWidget->setWidget(label);

  // // Add the dock widget to the top dock widget area
  // m_DockManager->addDockWidget(ads::TopDockWidgetArea, dockWidget);

  // // Create the View menu and add toggle action
  // m_menuView = menuBar()->addMenu("View");
  // m_menuView->addAction(dockWidget->toggleViewAction());
}

MainWindow::~MainWindow() {
  // No ui pointer to delete
  delete m_DockManager;
}

void MainWindow::closeEvent(QCloseEvent *event) {
  QSettings settings("mzhang", "panorama");
  settings.setValue("geometry", saveGeometry());
  settings.setValue("windowState", saveState());
  settings.setValue("dockManagerState", m_DockManager->saveState());
  QMainWindow::closeEvent(event);
}
