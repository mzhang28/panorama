#pragma once

#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>
#include <QPluginLoader>
#include <QtNetwork/QNetworkAccessManager>
#include <string_view>

#include "DockManager.h"
#include "ads_globals.h"

class MainToolBar;

class MainWindow : public QMainWindow {
  Q_OBJECT

public:
  explicit MainWindow(QWidget *parent = nullptr);
  ~MainWindow();

public slots:
  /** Open the given URL as a new window with the given dock widget */
  void openUrl(std::string_view url,
               ads::DockWidgetArea area = ads::CenterDockWidgetArea);

protected:
  void closeEvent(QCloseEvent *event) override;
  void dragEnterEvent(QDragEnterEvent *event) override;
  void dropEvent(QDropEvent *event) override;

private:
  ads::CDockManager *m_DockManager;

  QNetworkAccessManager *backendConn;
  MainToolBar *m_toolbar{nullptr};
  QMenu *m_menuView;
  QVector<QPluginLoader *> m_pluginLoaders;
};
