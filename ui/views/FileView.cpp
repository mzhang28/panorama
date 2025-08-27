#include "FileView.h"

#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QImage>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QLabel>
#include <QNetworkReply>
#include <QNetworkRequest>
#include <QScrollArea>
#include <QUrl>
#include <QVBoxLayout>
#include <QtPdf/QPdfDocument>
#include <QtPdfWidgets/QPdfView>
#include <algorithm>
#include <cmath>

FileView::FileView(const QString &nodeId, QNetworkAccessManager *mgr,
                   QWidget *parent)
    : QWidget(parent), m_nodeId(nodeId), m_mgr(mgr) {
  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(4);

  QLabel *title = new QLabel(QString("File %1").arg(nodeId), this);
  layout->addWidget(title);

  m_info = new QLabel(tr("Loading..."), this);
  layout->addWidget(m_info);

  if (!m_mgr)
    return;

  QString gql = QString("{ nodes(id: \"%1\") { id type created_at updated_at "
                        "file { name sha256 size } } }")
                    .arg(nodeId);

  QJsonObject payload;
  payload.insert("query", gql);

  QNetworkRequest req(QUrl("http://127.0.0.1:4141/graphql"));
  req.setHeader(QNetworkRequest::ContentTypeHeader, "application/json");
  QNetworkReply *reply = m_mgr->post(req, QJsonDocument(payload).toJson());
  connect(reply, &QNetworkReply::finished, this, [this, reply]() {
    if (reply->error() != QNetworkReply::NoError) {
      m_info->setText(tr("Error fetching file info"));
      reply->deleteLater();
      return;
    }
    QJsonDocument doc = QJsonDocument::fromJson(reply->readAll());
    reply->deleteLater();
    if (!doc.isObject())
      return;
    QJsonObject obj = doc.object();
    QJsonObject data = obj.value("data").toObject();
    QJsonArray nodes = data.value("nodes").toArray();
    if (nodes.isEmpty()) {
      m_info->setText(tr("No node returned"));
      return;
    }
    QJsonObject node = nodes.first().toObject();
    QJsonObject file = node.value("file").toObject();
    QString name = file.value("name").toString();
    QString sha = file.value("sha256").toString();
    QString size = file.value("size").toString();
    QString created = node.value("created_at").toString();
    QString updated = node.value("updated_at").toString();

    QString text =
        QString("Name: %1\nSHA256: %2\nSize: %3\nCreated: %4\nUpdated: %5")
            .arg(name)
            .arg(sha)
            .arg(size)
            .arg(created)
            .arg(updated);
    m_info->setText(text);

    // If this looks like a PDF, attempt to open it in the system PDF viewer.
    bool looksLikePdf = name.toLower().endsWith(".pdf");
    QString blobPath =
        QDir::currentPath() + "/blobs/" + sha.left(2) + "/" + sha;
    if (!looksLikePdf) {
      // Check magic header
      QFile blob(blobPath);
      if (blob.open(QIODevice::ReadOnly)) {
        QByteArray head = blob.read(4);
        blob.close();
        if (head == "%PDF")
          looksLikePdf = true;
      }
    }

    if (looksLikePdf && QFile::exists(blobPath)) {
      // Embed the PDF by rendering all pages into a scrollable view.
      QPdfDocument *doc = new QPdfDocument(this);
      QPdfDocument::Error error = doc->load(blobPath);
      if (error == QPdfDocument::Error::None) {
        int pages = doc->pageCount();

        QScrollArea *scroll = new QScrollArea(this);
        scroll->setWidgetResizable(true);
        QWidget *container = new QWidget();
        QVBoxLayout *vlay = new QVBoxLayout(container);
        vlay->setContentsMargins(8, 8, 8, 8);
        vlay->setSpacing(12);

        const double dpi = 150.0; // rendering DPI
        for (int i = 0; i < pages; ++i) {
          QSizeF pts = doc->pagePointSize(i);
          QSize imgSize(std::max(1, int(std::ceil(pts.width() * dpi / 72.0))),
                        std::max(1, int(std::ceil(pts.height() * dpi / 72.0))));
          // QImage image(imgSize, QImage::Format_ARGB32);
          // image.fill(Qt::white);
          // // Render page into the image (API: render(page, QImage*))
          // doc->render(i, &image);
          QImage image = doc->render(i, imgSize);
          if (image.isNull())
            continue;
          QLabel *p = new QLabel();
          p->setAlignment(Qt::AlignCenter);
          p->setPixmap(QPixmap::fromImage(image));
          vlay->addWidget(p);
        }

        container->setLayout(vlay);
        scroll->setWidget(container);
        this->layout()->addWidget(scroll);
        m_info->setText(text + "\n\nDisplayed embedded PDF (all pages).");
      } else {
        // Fallback to single-page QPdfView if document loading failed.
        QPdfView *pdf = new QPdfView(this);
        QPdfDocument *doc2 = new QPdfDocument(pdf);
        doc2->load(blobPath);
        pdf->setDocument(doc2);
        pdf->setZoomMode(QPdfView::ZoomMode::FitInView);
        this->layout()->addWidget(pdf);
        m_info->setText(text + "\n\nDisplayed embedded PDF (fallback view).");
      }
    }
  });
}
