#pragma once

#include <QAction>
#include <QApplication>
#include <QIcon>
#include <QMainWindow>
#include <QMessageBox>
#include <QToolBar>

class MainToolBar : public QToolBar {
  Q_OBJECT
public:
  explicit MainToolBar(QWidget *parent = nullptr);

signals:
  void buttonClicked(const QString &name);
};
