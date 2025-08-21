#pragma once

#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>

#include "DockManager.h"

class MainWindow : public QMainWindow {
  Q_OBJECT

public:
  explicit MainWindow(QWidget *parent = nullptr);
  ~MainWindow();

protected:
  void closeEvent(QCloseEvent *event) override;

private:
  // The main container for docking
  ads::CDockManager *m_DockManager;

  // Menu
  QMenu *m_menuView;
};
