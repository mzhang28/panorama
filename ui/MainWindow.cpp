#include <QApplication>
#include <QCloseEvent>
#include <QCryptographicHash>
#include <QDir>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFile>
#include <QFileInfo>
#include <QFontDatabase>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QMessageBox>
#include <QMimeData>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QSettings>
#include <QTimer>
#include <QUrl>
#include <QUuid>
#include <QVBoxLayout>
#include <QWebSocket>

#include "AutoHideDockContainer.h"
#include "DockAreaWidget.h"
#include "DockManager.h"
#include "DockWidget.h"
#include "MainWindow.h"
#include "Recents.h"
#include "Toolbar.h"
#include "WidgetRegistry.h"
#include "ads_globals.h"
#include "plugins/PluginInterface.h"
#include "stores/JournalStore.h"
#include "views/Journal.h"
#include "widgets/FileView.h"
#include <QPluginLoader>
#include <QUuid>
#include <functional>

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent) {
  // Allow drag & drop of files onto the main window
  setAcceptDrops(true);

  this->backendConn = new QNetworkAccessManager();
  // Provide the network manager to the centralized JournalStore
  JournalStore::instance()->setNetworkManager(this->backendConn);

  // Query backend for available UI plugins and load them
  QNetworkRequest plreq(QUrl("http://127.0.0.1:4141/plugins"));
  QNetworkReply *plreply = this->backendConn->get(plreq);
  connect(plreply, &QNetworkReply::finished, this, [this, plreply]() {
    if (plreply->error() != QNetworkReply::NoError) {
      qWarning() << "failed to fetch plugins:" << plreply->errorString();
      return;
    }
    auto data = plreply->readAll();
    QJsonDocument doc = QJsonDocument::fromJson(data);
    if (!doc.isArray())
      return;
    QJsonArray arr = doc.array();
    for (auto v : arr) {
      if (!v.isObject())
        continue;
      QJsonObject obj = v.toObject();
      QString path = obj.value("path").toString();
      if (path.isEmpty())
        continue;
      QPluginLoader loader(path);
      QObject *plugin = loader.instance();
      if (!plugin) {
        qWarning() << "failed to load plugin:" << path << loader.errorString();
        continue;
      }
      PluginInterface *iface = qobject_cast<PluginInterface *>(plugin);
      if (!iface) {
        qWarning() << "plugin does not implement PluginInterface:" << path;
        continue;
      }
      QStringList types = iface->availableWidgetTypes();
      for (const QString &t : types) {
        // Register factory into JournalStore or MainWindow (POC: print)
        qDebug() << "plugin" << path << "provides widget" << t;
        // In a real implementation we'd store a factory that calls
        // iface->createWidget
      }
    }
  });

  // Query startup tasks and run them
  QNetworkRequest streq(QUrl("http://127.0.0.1:4141/startup"));
  QNetworkReply *streply = this->backendConn->get(streq);
  connect(streply, &QNetworkReply::finished, this, [this, streply]() {
    if (streply->error() != QNetworkReply::NoError) {
      qWarning() << "failed to fetch startup tasks:" << streply->errorString();
      return;
    }
    auto data = streply->readAll();
    QJsonDocument doc = QJsonDocument::fromJson(data);
    if (!doc.isArray())
      return;
    QJsonArray arr = doc.array();
    for (auto v : arr) {
      if (!v.isObject())
        continue;
      QJsonObject obj = v.toObject();
      QString type = obj.value("type").toString();
      if (type == "call_app") {
        QString app = obj.value("app").toString();
        QString func = obj.value("function").toString();
        QJsonObject payload;
        payload.insert("app", app);
        payload.insert("function", func);
        QNetworkRequest callreq(
            QUrl("http://127.0.0.1:4141/call_app_function"));
        callreq.setHeader(QNetworkRequest::ContentTypeHeader,
                          "application/json");
        QNetworkReply *callreply =
            backendConn->post(callreq, QJsonDocument(payload).toJson());
        connect(callreply, &QNetworkReply::finished, this, [this, callreply]() {
          if (callreply->error() != QNetworkReply::NoError) {
            qWarning() << "call_app_function failed:"
                       << callreply->errorString();
            return;
          }
          QJsonDocument resp = QJsonDocument::fromJson(callreply->readAll());
          if (!resp.isObject())
            return;
          QJsonObject robj = resp.object();
          if (robj.contains("result")) {
            QString url = robj.value("result").toString();
            this->openUrl(url.toStdString(), ads::CenterDockWidgetArea);
          }
        });
      }
    }
  });

  // Poll for UI events from the backend regularly
  QTimer *evtTimer = new QTimer(this);
  evtTimer->setInterval(1000);
  connect(evtTimer, &QTimer::timeout, this, [this]() {
    QNetworkRequest ereq(QUrl("http://127.0.0.1:4141/ui/events"));
    QNetworkReply *ereply = this->backendConn->get(ereq);
    connect(ereply, &QNetworkReply::finished, this, [this, ereply]() {
      if (ereply->error() != QNetworkReply::NoError)
        return;
      QJsonDocument doc = QJsonDocument::fromJson(ereply->readAll());
      if (!doc.isArray())
        return;
      QJsonArray arr = doc.array();
      for (auto v : arr) {
        if (!v.isObject())
          continue;
        QJsonObject obj = v.toObject();
        QString src = obj.value("source").toString();
        QString url = obj.value("url").toString();
        // Heuristic: if source contains "journal" open to the right, else
        // center
        ads::DockWidgetArea area = ads::CenterDockWidgetArea;
        if (src.contains("journal"))
          area = ads::RightDockWidgetArea;
        this->openUrl(url.toStdString(), area);
      }
    });
  });
  evtTimer->start();

  // Connect to backend websocket for pushed UI events. This complements
  // the existing polling fallback; when websocket messages arrive we'll
  // open URLs with context immediately.
  this->wsClient =
      new QWebSocket(QString(), QWebSocketProtocol::VersionLatest, this);
  connect(this->wsClient, &QWebSocket::textMessageReceived, this,
          &MainWindow::handleWsMessage);
  this->wsClient->open(QUrl("ws://127.0.0.1:4141/ws/ui"));

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
  QString family;
  if (id >= 0) {
    QStringList fams = QFontDatabase::applicationFontFamilies(id);
    if (!fams.isEmpty()) {
      family = fams.at(0);
    }
  }
  if (family.isEmpty()) {
    family = QApplication::font().family();
  }
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

  // Left sidebar (persistent, not part of the ADS docking system)
  Recents *recentsWidget = new Recents(this);
  QDockWidget *sidebarDock = new QDockWidget(tr("Sidebar"), this);
  sidebarDock->setObjectName("SidebarDock");
  sidebarDock->setAllowedAreas(Qt::LeftDockWidgetArea);
  sidebarDock->setFeatures(QDockWidget::NoDockWidgetFeatures);
  sidebarDock->setWidget(recentsWidget);
  sidebarDock->setMinimumWidth(240);
  sidebarDock->setMaximumWidth(300);
  addDockWidget(Qt::LeftDockWidgetArea, sidebarDock);

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
    // Register widget id
    QString wid = QUuid::createUuid().toString(QUuid::WithoutBraces);
    newWidget->setProperty("widget_id", wid);
    WidgetRegistry::instance()->registerWidget(wid, newWidget);
    connect(newWidget, &QObject::destroyed, this,
            [wid]() { WidgetRegistry::instance()->unregisterWidget(wid); });
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
    // Prefer opening file views on the right side
    if (area == ads::CenterDockWidgetArea) {
      area = ads::RightDockWidgetArea;
    }
  }

  m_DockManager->addDockWidget(area, newWidget);
}

void MainWindow::openUrlWithContext(std::string_view url,
                                    const QString &originWidgetId,
                                    OpenDirection direction) {
  // Determine area based on direction and whether origin widget exists
  ads::DockWidgetArea area = ads::CenterDockWidgetArea;
  if (direction == OpenDirection::NewTab)
    area = ads::CenterDockWidgetArea;
  else if (direction == OpenDirection::Left)
    area = ads::LeftDockWidgetArea;
  else if (direction == OpenDirection::Right)
    area = ads::RightDockWidgetArea;
  else if (direction == OpenDirection::Top)
    area = ads::TopDockWidgetArea;
  else if (direction == OpenDirection::Bottom)
    area = ads::BottomDockWidgetArea;
  // For Floating we open center and then set floating flag if supported

  // Create the widget normally
  openUrl(url, area);
  // If direction == Floating try to set floating on the last created widget
  if (direction == OpenDirection::Floating) {
    // Best-effort: find widget by widget_id property on last dock
    // Not implemented: ADS API to mark floating here is non-portable across
    // versions.
  }
}

void MainWindow::handleWsMessage(const QString &msg) {
  QJsonDocument doc = QJsonDocument::fromJson(msg.toUtf8());
  if (!doc.isObject())
    return;
  QJsonObject obj = doc.object();
  QString url = obj.value("url").toString();
  QString wid = obj.value("widget_id").toString();
  QString dir = obj.value("direction").toString();

  MainWindow::OpenDirection od = MainWindow::OpenDirection::NewTab;
  QString ldir = dir.toLower();
  if (ldir == "left")
    od = MainWindow::OpenDirection::Left;
  else if (ldir == "right")
    od = MainWindow::OpenDirection::Right;
  else if (ldir == "top")
    od = MainWindow::OpenDirection::Top;
  else if (ldir == "bottom")
    od = MainWindow::OpenDirection::Bottom;
  else if (ldir == "floating")
    od = MainWindow::OpenDirection::Floating;
  else if (ldir == "center")
    od = MainWindow::OpenDirection::Center;
  else if (ldir == "newtab" || ldir == "new_tab")
    od = MainWindow::OpenDirection::NewTab;

  openUrlWithContext(url.toStdString(), wid, od);
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
