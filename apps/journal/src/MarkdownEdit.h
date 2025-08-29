#pragma once

#include "HostContext.h"
#include "qmarkdowntextedit.h"
#include <QDesktopServices>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFileInfo>
#include <QList>
#include <QMimeData>
#include <QString>
#include <QTextCursor>
#include <QUrl>
#include <QUuid>

#include "Journal.h"

class MarkdownEdit : public QMarkdownTextEdit {
  Q_OBJECT
public:
  explicit MarkdownEdit(Journal *parent = nullptr, HostContext *ctx = nullptr)
      : QMarkdownTextEdit(parent), ctx(ctx) {
    setAcceptDrops(true);
  }

signals:
  void panoramaLinkActivated(const QString &href);
  void fileDropped(const QString &uuid, const QString &path);

public slots:
  void replacePlaceholder(const QString &placeholder,
                          const QString &replacement) {
    QString content = toPlainText();
    if (!content.contains(placeholder))
      return;
    content.replace(placeholder, replacement);
    // simple replace for now
    setPlainText(content);
  }

protected:
  void dragEnterEvent(QDragEnterEvent *event) override {
    if (event->mimeData()->hasUrls())
      event->acceptProposedAction();
    else
      event->ignore();
  }

  void dropEvent(QDropEvent *event) override {
    if (!event->mimeData()->hasUrls()) {
      event->ignore();
      return;
    }
    QList<QUrl> urls = event->mimeData()->urls();
    if (urls.isEmpty())
      return;
    QUrl url = urls.first();
    if (!url.isLocalFile())
      return;
    QString path = url.toLocalFile();

    QString uuid = QUuid::createUuid().toString(QUuid::WithoutBraces);
    QString filename = QFileInfo(path).fileName();
    QString placeholder =
        QString("[Uploading %1](uploading://%2)").arg(filename).arg(uuid);
    QTextCursor cursor = textCursor();
    cursor.insertText(placeholder);
    emit fileDropped(placeholder, path);
  }

  // Prefer overriding QMarkdownTextEdit::openUrl which the upstream editor
  // calls when a link is activated. Provide both QString and QUrl entry
  // points to be robust against varying signatures.
  void openUrl(const QString &url) override;

private:
  HostContext *ctx;
};
