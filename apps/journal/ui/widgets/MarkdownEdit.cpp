#include "MarkdownEdit.h"

void MarkdownEdit::openUrl(const QString &url) {
  if (url.startsWith("panorama://")) {
    emit panoramaLinkActivated(url);
    return;
  }
  QDesktopServices::openUrl(url);
}

