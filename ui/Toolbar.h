#pragma once

#include <QAction>
#include <QApplication>
#include <QIcon>
#include <QMainWindow>
#include <QMessageBox>
#include <QToolBar>

#include "MainWindow.h"

class MainToolBar : public QToolBar {
  Q_OBJECT
public:
  explicit MainToolBar(MainWindow *parent);

signals:
  void buttonClicked(const QString &name);
};
