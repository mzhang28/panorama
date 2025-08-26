#pragma once

#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>
#include <QtNetwork/QNetworkAccessManager>
#include <string_view>

#include "DockManager.h"
#include "ads_globals.h"

class MainWindow : public QMainWindow {
  Q_OBJECT

public:
  explicit MainWindow(QWidget *parent = nullptr);
  ~MainWindow();

public slots:
  /** Open the given URL as a new window with the given dock widget */
  void openUrl(std::string_view url,
               ads::DockWidgetArea area = ads::TopDockWidgetArea);

protected:
  void closeEvent(QCloseEvent *event) override;

private:
  ads::CDockManager *m_DockManager;

  QNetworkAccessManager *backendConn;

  QMenu *m_menuView;
};
