#pragma once

#include <QObject>

class RecentNodeEntry {};

class RecentNodeStore : public QObject {
  Q_OBJECT

  using const_iterator = typename std::list<RecentNodeEntry>::const_iterator;

public:
  static RecentNodeStore &instance();
  const_iterator begin() const;
  const_iterator end() const;

signals:
  void updated();

private:
  RecentNodeStore();
};
