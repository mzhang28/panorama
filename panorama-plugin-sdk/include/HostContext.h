#pragma once

#include <QObject>

class HostContext : public QObject {
  Q_OBJECT

public:
  explicit HostContext(QObject *parent = nullptr) : QObject(parent) {}
  ~HostContext() override {}
};
