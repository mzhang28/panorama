#include <QApplication>
#include <QCloseEvent>
#include <QCryptographicHash>
#include <QDir>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFile>
#include <QFileInfo>
#include <QFontDatabase>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMessageBox>
#include <QMimeData>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QSettings>
#include <QUrl>
#include <QUuid>
#include <QVBoxLayout>

#include "AutoHideDockContainer.h"
#include "DockAreaWidget.h"
#include "DockManager.h"
#include "DockWidget.h"
#include "MainWindow.h"
#include "Recents.h"
#include "Toolbar.h"
#include "ads_globals.h"
#include "stores/JournalStore.h"
#include "views/Journal.h"
#include "widgets/FileView.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  // Allow drag & drop of files onto the main window
  setAcceptDrops(true);

  this->backendConn = new QNetworkAccessManager();
  // Provide the network manager to the centralized JournalStore
  JournalStore::instance()->setNetworkManager(this->backendConn);

  // Load window state
  QSettings settings("mzhang", "panorama");
  restoreGeometry(settings.value("geometry").toByteArray());
  restoreState(settings.value("windowState").toByteArray());

  m_toolbar = new MainToolBar(this);
  addToolBar(m_toolbar);
  connect(m_toolbar, &MainToolBar::buttonClicked, this,
          [](const QString &name) {
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
  ads::CDockWidget *recentsDock =
      m_DockManager->createDockWidget("RecentsDock");
  recentsDock->setMinimumWidth(240);
  recentsDock->setMaximumWidth(300);
  recentsDock->setWidget(recentsWidget);
  m_DockManager->addDockWidget(ads::LeftDockWidgetArea, recentsDock);
  if (auto left = recentsDock->dockAreaWidget()) {
    left->setMinimumWidth(240);
    left->setMaximumWidth(300);
  }

  // recentsDock->toggleView(false);
  // auto container =
  //     m_DockManager->addAutoHideDockWidget(ads::SideBarLeft, recentsDock);
  // container->collapseView(false);
  // container->toggleView(true);
  // Open recent node when double-clicked in the Recents list
  connect(recentsWidget, &Recents::openNode, this,
          [this](const QString &nodeId) {
            QString url = QString("/journal/%1").arg(nodeId);
            this->openUrl(url.toStdString(), ads::CenterDockWidgetArea);
          });
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
  ads::CDockWidget *newWidget;

  if (url.starts_with("/node/")) {
    std::string_view id = url.substr(6);
    std::cout << "lol! " << id << std::endl;
  }

  if (url.starts_with("/journal/")) {
    std::string_view id = url.substr(9);
    newWidget = m_DockManager->createDockWidget("journal");
    QString nid = QString::fromStdString(std::string(id));
    Journal *journal = new Journal(nid, this->backendConn, this);
    newWidget->setWidget(journal);
  } else if (url == "/importFile") {
    newWidget = m_DockManager->createDockWidget("import");
    QWidget *w = new QWidget();
    QVBoxLayout *l = new QVBoxLayout(w);
    QLabel *label = new QLabel(tr("Importing file..."), w);
    l->addWidget(label);
    newWidget->setWidget(w);
  } else if (url.starts_with("/file/")) {
    std::string_view id = url.substr(6);
    newWidget = m_DockManager->createDockWidget("file");
    QString nid = QString::fromStdString(std::string(id));
    FileView *view = new FileView(nid, this->backendConn, this);
    newWidget->setWidget(view);
  }

  m_DockManager->addDockWidget(area, newWidget);
}

void MainWindow::dragEnterEvent(QDragEnterEvent *event) {
  if (event->mimeData()->hasUrls()) {
    event->acceptProposedAction();
  } else {
    event->ignore();
  }
}

void MainWindow::dropEvent(QDropEvent *event) {
  if (!event->mimeData()->hasUrls()) {
    event->ignore();
    return;
  }

  // Open an import panel while we process the drop
  openUrl("/importFile", ads::CenterDockWidgetArea);

  // Handle the first file only for now
  QList<QUrl> urls = event->mimeData()->urls();
  if (urls.isEmpty())
    return;
  QUrl url = urls.first();
  if (!url.isLocalFile())
    return;
  QString path = url.toLocalFile();

  QFile f(path);
  if (!f.open(QIODevice::ReadOnly))
    return;
  QByteArray data = f.readAll();
  f.close();

// Compute SHA256
#include <QCryptographicHash>
  QByteArray hash =
      QCryptographicHash::hash(data, QCryptographicHash::Sha256).toHex();
  QString hex = QString::fromUtf8(hash);

  // Copy into blobs/<first2>/<hash>
  QString blobsRoot = QDir::currentPath() + "/blobs";
  QString prefix = hex.left(2);
  QDir dir(blobsRoot);
  if (!dir.exists())
    dir.mkpath(".");
  QString sub = blobsRoot + "/" + prefix;
  QDir subdir(sub);
  if (!subdir.exists())
    subdir.mkpath(".");
  QString targetPath = sub + "/" + hex;
  if (!QFile::exists(targetPath)) {
    QFile::copy(path, targetPath);
  }

  // Prepare GraphQL mutation to create node and set fields
  QString nodeId = QUuid::createUuid().toString(QUuid::WithoutBraces);
  qint64 size = data.size();

  QJsonObject vars;
  vars.insert("nodeId", nodeId);
  vars.insert("sha", hex);
  vars.insert("name", QFileInfo(path).fileName());
  vars.insert("size", QString::number(size));

  QString gql =
      "mutation($nodeId: String!, $sha: String!, $name: String!, $size: "
      "String!) {"
      " setField(nodeId: $nodeId, app: \"file\", field: \"sha256\", value: "
      "$sha)"
      " setField(nodeId: $nodeId, app: \"file\", field: \"name\", value: $name)"
      " setField(nodeId: $nodeId, app: \"file\", field: \"size\", value: $size)"
      " }";

  QJsonObject payload;
  payload.insert("query", gql);
  payload.insert("variables", vars);

  QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
  req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  QNetworkReply *reply =
      backendConn->post(req, QJsonDocument(payload).toJson());
  connect(reply, &QNetworkReply::finished, this, [this, reply, nodeId]() {
    if (reply->error() == QNetworkReply::NoError) {
      // Open the file panel for the newly created node
      QString url = QString("/file/%1").arg(nodeId);
      this->openUrl(url.toStdString());
    }
    reply->deleteLater();
  });
}
