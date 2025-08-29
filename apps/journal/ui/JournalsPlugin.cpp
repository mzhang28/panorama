#include <QLabel>
#include <QtNetwork/QNetworkAccessManager>

#include "JournalsPlugin.h"
#include "stores/JournalStore.h"
#include "views/Journal.h"

bool JournalsPlugin::handlesUrl(const QString &url) {
  return url.startsWith("/journal/");
}

QWidget *JournalsPlugin::createWidgetForUrl(HostContext *ctx,
                                            const QString &url,
                                            QWidget *parent) {
  // extract id portion
  if (!url.startsWith("/journal/"))
    return nullptr;
  QString id = url.mid(QString("/journal/").size());
  // create the real journal view
  Journal *j = new Journal(ctx, id, parent);
  if (ctx) {
    // When the journal emits a panorama link activation, ask the host to
    // open the URL. Use center area as a reasonable default (host maps
    // int to its own enum).
    connect(j, &Journal::panoramaLinkActivated, ctx,
            [ctx](const QString &href) { ctx->openUrl(href, /*area=*/0); });
  }
  return j;
}
