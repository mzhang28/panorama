#include <QHBoxLayout>
#include <QLabel>
#include <QMimeData>
#include <QTextEdit>
#include <QToolBar>
#include <QVBoxLayout>

#include "../stores/JournalStore.h"
#include "../widgets/MarkdownEdit.h"
#include "Journal.h"

#include <QApplication>
#include <QCryptographicHash>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QJsonParseError>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QTextCursor>
#include <QTimer>
#include <QUrl>
#include <QUuid>

Journal::Journal(const QString &nodeId, QNetworkAccessManager *mgr,
                 QWidget *parent)
    : QWidget(parent), m_nodeId(nodeId), m_mgr(mgr) {
  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(0);

  QString title = QString("Journal Entry %1").arg(nodeId);
  QLabel *label = new QLabel(title, this);
  // Header: title on the left, save status on the right
  QHBoxLayout *header = new QHBoxLayout();
  header->setContentsMargins(0, 0, 0, 0);
  header->addWidget(label);
  header->addStretch();
  m_statusLabel = new QLabel(this);
  m_statusLabel->setFixedSize(12, 12);
  m_statusLabel->setStyleSheet(
      "QLabel { background-color: #0a0; border-radius: 6px; }");
  m_statusLabel->setToolTip(tr("Saved"));
  header->addWidget(m_statusLabel);
  layout->addLayout(header);

  m_editor = new MarkdownEdit();
  layout->addWidget(m_editor);

  // Use the centralized JournalStore to load/save content and coordinate
  JournalStore *store = JournalStore::instance();
  // Ensure the store has a network manager if our MainWindow provided one
  if (m_mgr)
    store->setNetworkManager(m_mgr);

  // React to content updates from the store (including our own edits from other
  // windows)
  connect(store, &JournalStore::contentChanged, this,
          [this](const QString &id, const QString &content) {
            if (id != m_nodeId)
              return;
            // If our editor already has the same text, avoid calling
            // setPlainText which would reset the cursor (this prevents the
            // "jump to start" issue).
            if (m_editor->toPlainText() == content)
              return;
            // Programmatic update: avoid triggering save
            m_loading = true;
            m_editor->setPlainText(content);
            m_loading = false;
          });

  // Update the status indicator when the store emits status changes
  connect(store, &JournalStore::statusChanged, this,
          [this](const QString &id, const QString &status) {
            if (id != m_nodeId)
              return;
            if (!m_statusLabel)
              return;
            if (status == "saving") {
              m_statusLabel->setStyleSheet(
                  "QLabel { background-color: #e65a00; border-radius: 6px; }");
              m_statusLabel->setToolTip(tr("Saving..."));
            } else if (status == "saved") {
              m_statusLabel->setStyleSheet(
                  "QLabel { background-color: #0a0; border-radius: 6px; }");
              m_statusLabel->setToolTip(tr("Saved"));
            } else if (status == "unsaved") {
              m_statusLabel->setStyleSheet(
                  "QLabel { background-color: #c00; border-radius: 6px; }");
              m_statusLabel->setToolTip(tr("Unsaved"));
            } else {
              m_statusLabel->setStyleSheet(
                  "QLabel { background-color: #666; border-radius: 6px; }");
              m_statusLabel->setToolTip(tr("Unknown"));
            }
          });

  // Local edits update the store; the store will debounce and broadcast saves
  connect(m_editor, &QMarkdownTextEdit::textChanged, this, [this, store]() {
    if (m_loading)
      return;
    QString text = m_editor->toPlainText();
    store->setContent(m_nodeId, text);
  });

  // Ask the store to ensure we have the latest data for this node
  store->ensureLoaded(m_nodeId);

  // Handle file drops from the editor (MarkdownEdit emits fileDropped after
  // inserting a placeholder)
  connect(
      m_editor, &MarkdownEdit::fileDropped, this,
      [this](const QString &placeholder, const QString &path) {
        // Read file
        QFile f(path);
        if (!f.open(QIODevice::ReadOnly))
          return;
        QByteArray data = f.readAll();
        f.close();

        QByteArray hash =
            QCryptographicHash::hash(data, QCryptographicHash::Sha256).toHex();
        QString hex = QString::fromUtf8(hash);

        QString blobsRoot = QDir::currentPath() + "/blobs";
        QString prefix = hex.left(2);
        QDir dir(blobsRoot);
        if (!dir.exists())
          dir.mkpath(".");
        QString sub = blobsRoot + "/" + prefix;
        QDir subdir(sub);
        if (!subdir.exists())
          subdir.mkpath(".");
        QString targetPath = sub + "/" + hex;
        if (!QFile::exists(targetPath)) {
          QFile::copy(path, targetPath);
        }

        QString nodeId = QUuid::createUuid().toString(QUuid::WithoutBraces);
        QString name = QFileInfo(path).fileName();
        QString size = QString::number(data.size());

        QJsonObject vars;
        vars.insert("nodeId", nodeId);
        vars.insert("sha", hex);
        vars.insert("name", name);
        vars.insert("size", size);

        QString gql = "mutation($nodeId: String!, $sha: String!, $name: "
                      "String!, $size: String!) {"
                      " setField(nodeId: $nodeId, app: \"file\", field: "
                      "\"sha256\", value: $sha)"
                      " setField(nodeId: $nodeId, app: \"file\", field: "
                      "\"name\", value: $name)"
                      " setField(nodeId: $nodeId, app: \"file\", field: "
                      "\"size\", value: $size)"
                      " }";

        QJsonObject payload;
        payload.insert("query", gql);
        payload.insert("variables", vars);

        if (!m_mgr)
          return;
        QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
        req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
        QNetworkReply *reply =
            m_mgr->post(req, QJsonDocument(payload).toJson());
        connect(reply, &QNetworkReply::finished, this,
                [this, reply, placeholder, nodeId, hex, name]() {
                  if (reply->error() == QNetworkReply::NoError) {
                    QString replacement =
                        QString("[%1](panorama:///%2)").arg(name).arg(nodeId);
                    m_editor->replacePlaceholder(placeholder, replacement);
                  }
                  reply->deleteLater();
                });
      });

  // Handle clicks on panorama:// links inside the editor. When such a link is
  // clicked, open the corresponding file view in the main window.
  connect(m_editor, &MarkdownEdit::panoramaLinkActivated, this,
          [this](const QString &href) {
            QUrl u(href);
            QString path = u.path();
            if (path.startsWith('/'))
              path = path.mid(1);
            if (path.isEmpty())
              return;
            QString url = QString("/file/%1").arg(path);

            // Try to find the MainWindow: prefer the top-level window for this
            // widget, fallback to scanning top-level widgets.
            QWidget *top = this->window();
            MainWindow *mw = nullptr;
            if (top)
              mw = qobject_cast<MainWindow *>(top);
            if (!mw) {
              for (QWidget *w : QApplication::topLevelWidgets()) {
                mw = qobject_cast<MainWindow *>(w);
                if (mw)
                  break;
              }
            }
            if (!mw)
              return;
            mw->openUrl(url.toStdString(), ads::CenterDockWidgetArea);
          });
}
