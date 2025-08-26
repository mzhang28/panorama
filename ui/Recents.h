#pragma once

#include <QScrollArea>

class Recents : public QScrollArea {
  Q_OBJECT
public:
  explicit Recents(QWidget *parent = nullptr);
};
