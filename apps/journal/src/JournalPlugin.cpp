#include "JournalPlugin.h"
#include "Journal.h"

JournalPlugin::JournalPlugin() {}

QString JournalPlugin::name() const { return QStringLiteral("journal"); }

QString JournalPlugin::version() const { return QStringLiteral("0.1.0"); }

QWidget *JournalPlugin::handleUrl(HostContext *ctx, const QString &url, QObject *data) {
  Q_UNUSED(data);
  if (url.startsWith("/journal/")) {
    QString id = url.mid(QString("/journal/").length());
    // If a HostContext is provided, hand its network manager to the
    // Journal widget so it can perform GraphQL load/save operations.
  // Pass the HostContext directly to the Journal so the store can use
  // HostContext::graphqlRequest.
  return new Journal(id, ctx, nullptr);
  }
  return nullptr;
}
