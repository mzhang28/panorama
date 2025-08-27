#include "Recents.h"

#include <QListWidget>
#include <QVBoxLayout>
#include <QToolTip>
#include <QTimer>
#include <QMouseEvent>

#include "RecentNodeStore.h"

class RecentsList : public QListWidget {
  Q_OBJECT
public:
  explicit RecentsList(QWidget *parent = nullptr) : QListWidget(parent) {
    setMouseTracking(true);
    setSelectionMode(QAbstractItemView::NoSelection);
    setFocusPolicy(Qt::NoFocus);
    viewport()->installEventFilter(this);
    m_hoverTimer = new QTimer(this);
    m_hoverTimer->setSingleShot(true);
    connect(m_hoverTimer, &QTimer::timeout, this, &RecentsList::showHoverTooltip);
  }

  bool eventFilter(QObject *obj, QEvent *ev) override {
    if (obj == viewport()) {
      if (ev->type() == QEvent::MouseMove) {
        QMouseEvent *me = static_cast<QMouseEvent *>(ev);
        QListWidgetItem *it = itemAt(me->pos());
        if (it != m_hoverItem) {
          m_hoverItem = it;
          m_lastPos = me->globalPos();
          m_hoverTimer->stop();
          if (m_hoverItem)
            m_hoverTimer->start(1000); // 1s delay
          else
            QToolTip::hideText();
        }
      } else if (ev->type() == QEvent::Leave) {
        m_hoverTimer->stop();
        m_hoverItem = nullptr;
        QToolTip::hideText();
      }
    }
    return QListWidget::eventFilter(obj, ev);
  }

private slots:
  void showHoverTooltip() {
    if (!m_hoverItem)
      return;
    QVariant v = m_hoverItem->data(Qt::UserRole);
    if (!v.isValid())
      return;
    // data stored as a map-like string: nodeId|||title|||snippet|||lastModified
    QString data = v.toString();
    QStringList parts = data.split(QStringLiteral("|||"));
    QString nodeId = parts.value(0);
    QString title = parts.value(1);
    QString snippet = parts.value(2);
    QString lastModified = parts.value(3);
    QString tt = QString("<b>%1</b><br/><i>%2</i><br/><small>%3</small>")
                     .arg(title.toHtmlEscaped())
                     .arg(nodeId.toHtmlEscaped())
                     .arg(snippet.toHtmlEscaped() + "<br/>" + lastModified.toHtmlEscaped());
    QToolTip::showText(m_lastPos, tt, this);
  }

private:
  QListWidgetItem *m_hoverItem{nullptr};
  QTimer *m_hoverTimer{nullptr};
  QPoint m_lastPos;
};

Recents::Recents(QWidget *parent) : QScrollArea(parent) {
  setWidgetResizable(true);
  auto *list = new RecentsList();
  setWidget(list);

  // Populate initially and listen for updates
  auto &store = RecentNodeStore::instance();
  auto rebuild = [list, &store]() {
    list->clear();
    for (const auto &e : store.entries()) {
      QListWidgetItem *it = new QListWidgetItem(e.title, list);
      QString data = e.nodeId + QStringLiteral("|||") + e.title + QStringLiteral("|||") + e.snippet + QStringLiteral("|||") + e.lastModified.toString(Qt::ISODate);
      it->setData(Qt::UserRole, data);
      list->addItem(it);
    }
  };

  rebuild();
  connect(&store, &RecentNodeStore::updated, this, rebuild);
  // Handle double-clicks to open the journal entry for editing
  connect(list, &QListWidget::itemDoubleClicked, this, [this](QListWidgetItem *it) {
    if (!it) return;
    QString data = it->data(Qt::UserRole).toString();
    QStringList parts = data.split(QStringLiteral("|||"));
    QString nodeId = parts.value(0);
    if (!nodeId.isEmpty())
      emit openNode(nodeId);
  });
}

#include "Recents.moc"
