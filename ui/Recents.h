#pragma once

#include <QScrollArea>

class Recents : public QScrollArea {
  Q_OBJECT
public:
  explicit Recents(QWidget *parent = nullptr);

signals:
  // Emitted when the user wants to open a recent node for editing
  void openNode(const QString &nodeId);
};
