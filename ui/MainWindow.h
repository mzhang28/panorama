#pragma once

#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>
#include <QWebSocket>
#include <QtNetwork/QNetworkAccessManager>
#include <string_view>

#include "DockManager.h"
#include "ads_globals.h"
#include <QDockWidget>

class MainToolBar;

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
  QWebSocket *wsClient{nullptr};
  MainToolBar *m_toolbar{nullptr};
  QMenu *m_menuView;
};
