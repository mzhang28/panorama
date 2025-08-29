#include "UiHostContext.h"

void UiHostContext::openUrl(const QString &url) {
  qDebug() << "UiHostContext openUrl" << url;
  this->m_mainWindow->openUrl(url.toStdString());
}
