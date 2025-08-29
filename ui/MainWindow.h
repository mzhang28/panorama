#pragma once

#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>
#include <QWebSocket>
#include <QtNetwork/QNetworkAccessManager>
#include <string_view>
#include <QList>
#include <QPluginLoader>
class PluginInterface;

#include "DockManager.h"
#include "ads_globals.h"
#include <QDockWidget>

class MainToolBar;

// Include the plugin SDK so we can expose a HostContext to plugins
#include "plugin-sdk/PluginInterface.h"
#include "plugin-sdk/HostContext.h"

class MainWindow : public QMainWindow {
  Q_OBJECT

public:
  explicit MainWindow(QWidget *parent = nullptr);
  ~MainWindow();

  /** Open the given URL as a new window with the given dock widget */
  enum class OpenDirection {
    Left,
    Right,
    Top,
    Bottom,
    NewTab,
    Floating,
    Center
  };

public slots:
  void openUrl(std::string_view url,
               ads::DockWidgetArea area = ads::CenterDockWidgetArea);

  // Open with widget context and direction
  void openUrlWithContext(std::string_view url, const QString &originWidgetId,
                          OpenDirection direction = OpenDirection::NewTab);

  void handleWsMessage(const QString &msg);

protected:
  void closeEvent(QCloseEvent *event) override;
  void dragEnterEvent(QDragEnterEvent *event) override;
  void dropEvent(QDropEvent *event) override;

private:
  ads::CDockManager *m_DockManager;

  QNetworkAccessManager *backendConn;
  // Context object passed to plugins allowing them to request UI actions
  HostContext *m_mainContext{nullptr};
  QWebSocket *wsClient{nullptr};
  MainToolBar *m_toolbar{nullptr};
  QMenu *m_menuView;
  QList<QPluginLoader*> m_pluginLoaders;
  QList<PluginInterface*> m_plugins;
};
