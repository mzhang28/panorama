#include <QCloseEvent>
#include <QFontDatabase>
#include <QLabel>
#include <QSettings>

#include "DockWidget.h"
#include "MainWindow.h"
#include "Toolbar.h"
#include "ads_globals.h"
#include "views/Calendar.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  this->backendConn = new QNetworkAccessManager();

  QSettings settings("mzhang", "panorama");
  restoreGeometry(settings.value("geometry").toByteArray());
  restoreState(settings.value("windowState").toByteArray());

  MainToolBar *toolbar = new MainToolBar(this);
  addToolBar(toolbar);
  connect(toolbar, &MainToolBar::buttonClicked, this, [](const QString &name) {
    QMessageBox::information(nullptr, "Toolbar", name + " clicked");
  });

  // Set font
  int id = QFontDatabase::addApplicationFont(
      ":/fonts/Inter-VariableFont_opsz,wght.ttf");
  QString family = QFontDatabase::applicationFontFamilies(id).at(0);
  QFont font(family, 12);
  QApplication::setFont(font);

  // Create the dock manager
  m_DockManager = new ads::CDockManager(this);
  m_DockManager->restoreState(settings.value("dockManagerState").toByteArray());
  ads::CDockManager::setConfigFlag(
      ads::CDockManager::HideSingleCentralWidgetTitleBar, true);

  // QLabel *label = new QLabel("Welcome to panorama!");
  // label->setAlignment(Qt::AlignCenter);

  // ads::CDockWidget *empty = new ads::CDockWidget(m_DockManager, "Empty");
  // empty->setWidget(label);
  // empty->setFeature(ads::CDockWidget::NoTab, true);
  // m_DockManager->setCentralWidget(empty);

  // m_DockManager->setEmptyDockWidgetText("No dock widgets available");

  // // Create a dock widget with the label
  {
    ads::CDockWidget *dockWidget = m_DockManager->createDockWidget("Calendar");
    GCalWeekView *calendar = new GCalWeekView(this);
    dockWidget->setWidget(calendar);
    m_DockManager->addDockWidget(ads::TopDockWidgetArea, dockWidget);
  }

  {
    ads::CDockWidget *dockWidget = m_DockManager->createDockWidget("Calendar2");
    GCalWeekView *calendar = new GCalWeekView(this);
    dockWidget->setWidget(calendar);
    m_DockManager->addDockWidget(ads::CenterDockWidgetArea, dockWidget);
  }

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

void MainWindow::openUrl(std::string_view url, ads::DockWidgetArea area) {
  std::cout << "openge " << url << std::endl;
}
