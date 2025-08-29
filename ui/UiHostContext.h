#pragma once

#include "HostContext.h"
#include "MainWindow.h"

class UiHostContext : public HostContext {
public:
  UiHostContext(MainWindow *mainWindow, QNetworkAccessManager *mgr)
      : HostContext(mgr), m_mainWindow(mainWindow) {}

  void openUrl(const QString &url) override;

private:
  MainWindow *m_mainWindow{nullptr};
};
