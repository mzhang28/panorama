#pragma once

#include <QAction>
#include <QString>
#include <QToolBar>

// Forward-declare MainWindow to avoid including the header here and
// creating an unnecessary circular dependency.
class MainWindow;

class MainToolBar : public QToolBar {
  Q_OBJECT
public:
  explicit MainToolBar(MainWindow *parent);

signals:
  void buttonClicked(const QString &name);
};
