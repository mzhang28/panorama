#include <QCloseEvent>
#include <QFontDatabase>
#include <QLabel>
#include <QSettings>

#include "DockManager.h"
#include "DockOverlay.h"
#include "DockWidget.h"
#include "MainWindow.h"
#include "Recents.h"
#include "Toolbar.h"
#include "ads_globals.h"
#include "views/Journal.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  this->backendConn = new QNetworkAccessManager();


  // Load window state
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
  font.setStyleStrategy(QFont::PreferAntialias);
  QApplication::setFont(font);

  // Create the dock manager
  ads::CDockManager::setConfigFlag(
      ads::CDockManager::HideSingleCentralWidgetTitleBar, true);
  ads::CDockManager::setAutoHideConfigFlag(
      ads::CDockManager::AutoHideFeatureEnabled, true);
  ads::CDockManager::setAutoHideConfigFlag(
      ads::CDockManager::DockAreaHasAutoHideButton, true);

  m_DockManager = new ads::CDockManager(this);
  m_DockManager->restoreState(settings.value("dockManagerState").toByteArray());
  // ads::CDockManager::setConfigFlag(ads::CDockManager::FocusHighlighting,
  // true); // THIS CAUSES A SEGFAULT??

  if (m_DockManager->dockWidgetsMap().size() == 0) {
    auto today =
        std::chrono::floor<std::chrono::days>(std::chrono::system_clock::now());
    auto url = std::format("/journal/{:%F}", today);
    openUrl(url, ads::CenterDockWidgetArea);
  }

  // Left sidebar
  Recents *recentsWidget = new Recents(this);
  ads::CDockWidget *recentsDock = m_DockManager->createDockWidget("recents");
  recentsDock->setWidget(recentsWidget);
  // recentsDock->toggleView(false);
  m_DockManager->addAutoHideDockWidget(ads::SideBarLeft, recentsDock);
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

  // TODO: Replace this with some proper routing

  if (url.starts_with("/node/")) {
    std::string_view id = url.substr(6);
    std::cout << "lol! " << id << std::endl;
  }

  if (url.starts_with("/journal/")) {
    std::string_view id = url.substr(9);
    ads::CDockWidget *dockWidget = m_DockManager->createDockWidget("journal");
    QString nid = QString::fromStdString(std::string(id));
    Journal *journal = new Journal(nid, this->backendConn, this);
    dockWidget->setWidget(journal);
    m_DockManager->addDockWidget(area, dockWidget);
  }
}
