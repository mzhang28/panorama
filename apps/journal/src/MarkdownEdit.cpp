#include "MarkdownEdit.h"

void MarkdownEdit::openUrl(const QString &url) {
  if (url.startsWith("panorama://")) {
    this->ctx->openUrl(url);
    return;
  }
  QDesktopServices::openUrl(url);
}
