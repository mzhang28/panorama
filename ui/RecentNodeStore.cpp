#include "RecentNodeStore.h"

RecentNodeStore &RecentNodeStore::instance() {
  static RecentNodeStore _instance;
  return _instance;
}

RecentNodeStore::RecentNodeStore() {}
