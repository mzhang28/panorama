#include "JournalPlugin.h"
#include "Journal.h"

JournalPlugin::JournalPlugin() {}

QString JournalPlugin::name() const { return QStringLiteral("journal"); }

QString JournalPlugin::version() const { return QStringLiteral("0.1.0"); }

QWidget *JournalPlugin::handleUrl(HostContext *ctx, const QString &url, QObject *data) {
  Q_UNUSED(ctx);
  Q_UNUSED(data);
  if (url.startsWith("/journal/")) {
    QString id = url.mid(QString("/journal/").length());
    return new Journal(id, nullptr, nullptr);
  }
  return nullptr;
}
