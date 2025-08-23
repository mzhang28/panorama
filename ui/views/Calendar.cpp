// GCalWeekView.cpp
#include "Calendar.h"

#include <QMouseEvent>
#include <QPaintEvent>
#include <QPainter>
#include <QResizeEvent>
#include <QScrollBar>
#include <QStyle>
#include <QStyleOption>
#include <QTime>
#include <QWheelEvent>
#include <algorithm>
#include <cmath>

static QDate weekStartFor(const QDate &any, Qt::DayOfWeek start) {
  int diff = (7 + (any.dayOfWeek() - start)) % 7;
  return any.addDays(-diff);
}

GCalWeekView::GCalWeekView(QWidget *parent)
    : QAbstractScrollArea(parent), focusDate_(QDate::currentDate()) {
  setAttribute(Qt::WA_OpaquePaintEvent, true);
  setMouseTracking(true);
  viewport()->setCursor(Qt::ArrowCursor);
  nowTimerId_ = startTimer(60'000); // update now-line every minute
  ensureScrollbars();
}

void GCalWeekView::clearEvents() {
  events_.clear();
  viewport()->update();
}
void GCalWeekView::addEvent(const Event &ev) {
  events_.push_back(ev);
  viewport()->update();
}
bool GCalWeekView::removeEvent(const QString &id) {
  auto it = std::remove_if(events_.begin(), events_.end(),
                           [&](const Event &e) { return e.id == id; });
  bool removed = it != events_.end();
  events_.erase(it, events_.end());
  if (removed)
    viewport()->update();
  return removed;
}
bool GCalWeekView::hasEvent(const QString &id) const {
  for (const auto &e : events_)
    if (e.id == id)
      return true;
  return false;
}

void GCalWeekView::setWeekStart(Qt::DayOfWeek d) {
  weekStart_ = d;
  viewport()->update();
}
Qt::DayOfWeek GCalWeekView::weekStart() const { return weekStart_; }

void GCalWeekView::setFocusDate(const QDate &d) {
  focusDate_ = d;
  viewport()->update();
}
QDate GCalWeekView::focusDate() const { return focusDate_; }

void GCalWeekView::setHourRange(int f, int t) {
  hourFrom_ = qBound(0, f, 24);
  hourTo_ = qBound(0, t, 24);
  if (hourTo_ <= hourFrom_)
    hourTo_ = hourFrom_ + 1;
  ensureScrollbars();
  viewport()->update();
}
void GCalWeekView::setTimeGridStep(int minutes) {
  gridStepMin_ = qMax(5, minutes);
  viewport()->update();
}
void GCalWeekView::set24Hour(bool on) {
  show24h_ = on;
  viewport()->update();
}
void GCalWeekView::setAllDayRowVisible(bool on) {
  showAllDay_ = on;
  viewport()->update();
}
void GCalWeekView::setHeaderHeight(int px) {
  headerH_ = qMax(20, px);
  ensureScrollbars();
  viewport()->update();
}
void GCalWeekView::setAllDayHeight(int px) {
  allDayH_ = qMax(0, px);
  ensureScrollbars();
  viewport()->update();
}
void GCalWeekView::setTimeGutterWidth(int px) {
  timeGutterW_ = qMax(32, px);
  ensureScrollbars();
  viewport()->update();
}
void GCalWeekView::setRowHeightPxPerHour(int px) {
  pxPerHour_ = qMax(24, px);
  ensureScrollbars();
  viewport()->update();
}

QSize GCalWeekView::sizeHint() const { return {900, 640}; }

void GCalWeekView::resizeEvent(QResizeEvent *) { ensureScrollbars(); }

void GCalWeekView::timerEvent(QTimerEvent *e) {
  if (e->timerId() == nowTimerId_)
    viewport()->update();
  QAbstractScrollArea::timerEvent(e);
}

void GCalWeekView::wheelEvent(QWheelEvent *e) {
#if QT_VERSION >= QT_VERSION_CHECK(5, 14, 0)
  QPoint numPixels = e->pixelDelta();
  QPoint numDegrees = e->angleDelta() / 8;
#else
  QPoint numPixels;
  QPoint numDegrees = e->angleDelta() / 8;
#endif
  int dy = 0;
  if (!numPixels.isNull())
    dy = -numPixels.y();
  else if (!numDegrees.isNull())
    dy = -(numDegrees.y() / 15) * 40;
  verticalScrollBar()->setValue(verticalScrollBar()->value() + dy);
  e->accept();
}

QRect GCalWeekView::headerRect() const {
  return QRect(0, 0, viewport()->width(), headerH_);
}
QRect GCalWeekView::allDayRect() const {
  int h = showAllDay_ ? allDayH_ : 0;
  return QRect(0, headerH_, viewport()->width(), h);
}
QRect GCalWeekView::gridRect() const {
  int y = headerH_ + (showAllDay_ ? allDayH_ : 0);
  return QRect(0, y, viewport()->width(), viewport()->height() - y);
}
QRect GCalWeekView::timeGutterRect() const {
  QRect r = gridRect();
  return QRect(r.left(), r.top(), timeGutterW_, r.height());
}
QRect GCalWeekView::dayColumnRect(int dayIndex) const {
  QRect r = gridRect();
  int left = r.left() + timeGutterW_;
  int colW = (r.width() - timeGutterW_) / 7;
  return QRect(left + dayIndex * colW, r.top(), colW, r.height());
}

double GCalWeekView::yForDateTime(const QDateTime &dt) const {
  QTime t = dt.time();
  double hours = t.hour() + t.minute() / 60.0 + t.second() / 3600.0;
  double offsetHours = hours - hourFrom_;
  return headerH_ + (showAllDay_ ? allDayH_ : 0) + offsetHours * pxPerHour_ -
         verticalScrollBar()->value();
}

QDateTime GCalWeekView::dateTimeForY(double y, int dayIndex) const {
  double gy = y + verticalScrollBar()->value() - headerH_ -
              (showAllDay_ ? allDayH_ : 0);
  double hours = hourFrom_ + gy / pxPerHour_;
  if (hours < hourFrom_)
    hours = hourFrom_;
  if (hours > hourTo_)
    hours = hourTo_;
  int h = int(std::floor(hours));
  int m = int(std::round((hours - h) * 60 / gridStepMin_)) * gridStepMin_;
  if (m >= 60) {
    h += 1;
    m = 0;
  }
  QDate d0 = weekStartFor(focusDate_, weekStart_);
  return QDateTime(d0.addDays(dayIndex), QTime(h, m)).toLocalTime();
}

void GCalWeekView::ensureScrollbars() {
  int contentH = int((hourTo_ - hourFrom_) * pxPerHour_);
  int viewH = gridRect().height();
  verticalScrollBar()->setRange(0, qMax(0, contentH - viewH));
  verticalScrollBar()->setPageStep(viewH);
}

void GCalWeekView::paintEvent(QPaintEvent *e) {
  Q_UNUSED(e);
  QPainter p(viewport());
  p.fillRect(rect(), Qt::white);

  paintHeaders(p);
  paintAllDay(p);
  paintGrid(p);

  QVector<_EventLayout> lay;
  layoutTimedEvents(lay);
  lastLayout_ = lay;
  paintEvents(p, lay);
  if (showNowLine_)
    paintNowLine(p);
}

void GCalWeekView::paintHeaders(QPainter &p) {
  QRect r = headerRect();
  p.fillRect(r, QColor(247, 247, 247));
  p.setPen(QColor(220, 220, 220));
  p.drawLine(r.bottomLeft(), r.bottomRight());

  // Weekday labels
  QDate start = weekStartFor(focusDate_, weekStart_);
  QFont f = p.font();
  f.setBold(true);
  p.setFont(f);
  p.setPen(QColor(60, 60, 60));
  for (int i = 0; i < 7; ++i) {
    QRect col = dayColumnRect(i);
    QRect cell(col.left(), r.top(), col.width(), r.height());
    QString wd =
        QLocale().dayName((start.addDays(i)).dayOfWeek(), QLocale::ShortFormat);
    QString txt = QString("%1 %2/%3")
                      .arg(wd)
                      .arg(start.addDays(i).month())
                      .arg(start.addDays(i).day());
    p.drawText(cell.adjusted(8, 0, -8, 0), Qt::AlignVCenter | Qt::AlignLeft,
               txt);
  }
}

void GCalWeekView::paintAllDay(QPainter &p) {
  if (!showAllDay_)
    return;
  QRect r = allDayRect();
  p.fillRect(r, QColor(252, 252, 252));
  p.setPen(QColor(235, 235, 235));
  p.drawLine(r.bottomLeft(), r.bottomRight());

  // draw all-day chips
  QDate start = weekStartFor(focusDate_, weekStart_);
  int colW = dayColumnRect(0).width();
  int left0 = dayColumnRect(0).left();
  int padding = 4;
  QFont f = p.font();
  f.setBold(false);
  p.setFont(f);
  for (const auto &e : events_) {
    if (!e.allDay)
      continue;
    int s = qBound(0, start.daysTo(e.start.date()), 6);
    int t = qBound(0, start.daysTo(e.end.date().addDays(-1)), 6);
    for (int d = s; d <= t; ++d) {
      QRect chip(left0 + d * colW + padding, r.top() + padding,
                 colW - 2 * padding, r.height() - 2 * padding);
      p.fillRect(chip, e.color.lighter(140));
      p.setPen(e.color.darker(140));
      p.drawRect(chip.adjusted(0, 0, -1, -1));
      p.setPen(QColor(40, 40, 40));
      p.drawText(chip.adjusted(6, 0, -6, 0), Qt::AlignVCenter | Qt::AlignLeft,
                 e.title);
    }
  }
}

void GCalWeekView::paintGrid(QPainter &p) {
  QRect r = gridRect();
  // gutter background
  p.fillRect(timeGutterRect(), QColor(250, 250, 250));
  p.setPen(QColor(235, 235, 235));
  p.drawLine(r.topLeft(), r.topRight());

  // vertical day separators
  p.setPen(QColor(230, 230, 230));
  for (int i = 0; i <= 7; ++i) {
    int x = dayColumnRect(0).left() + i * dayColumnRect(0).width();
    p.drawLine(x, r.top(), x, r.bottom());
  }

  // horizontal hour lines + labels
  int totalHours = hourTo_ - hourFrom_;
  QRect gutter = timeGutterRect();
  for (int h = 0; h <= totalHours; ++h) {
    int y = r.top() + int(h * pxPerHour_) - verticalScrollBar()->value();
    p.setPen(QColor(240, 240, 240));
    p.drawLine(gutter.right(), y, r.right(), y);

    // bold line at whole hour
    p.setPen(QColor(225, 225, 225));
    p.drawLine(gutter.right(), y, r.right(), y);

    // time label
    p.setPen(QColor(110, 110, 110));
    QTime tm(hourFrom_ + h, 0);
    QString label;
    if (show24h_)
      label = tm.toString("HH:mm");
    else
      label = tm.toString("h AP");
    p.drawText(gutter.adjusted(0, -8, -6, 0).translated(0, y - r.top() - 8),
               Qt::AlignRight | Qt::AlignVCenter, label);
  }

  // minor grid for steps
  if (gridStepMin_ < 60) {
    p.setPen(QColor(248, 248, 248));
    int stepsPerHour = 60 / gridStepMin_;
    for (int h = 0; h < totalHours; ++h) {
      for (int s = 1; s < stepsPerHour; ++s) {
        int y = r.top() + int((h + s / double(stepsPerHour)) * pxPerHour_) -
                verticalScrollBar()->value();
        p.drawLine(gutter.right(), y, r.right(), y);
      }
    }
  }
}

void GCalWeekView::layoutTimedEvents(QVector<_EventLayout> &out) const {
  out.clear();
  QDate week0 = weekStartFor(focusDate_, weekStart_);
  // collect per-day timed events
  struct Node {
    int idx;
    QDateTime s, e;
  };
  QVector<Node> byDay[7];
  for (int i = 0; i < events_.size(); ++i) {
    const auto &ev = events_[i];
    if (ev.allDay)
      continue;
    QDate sD = ev.start.date(), eD = ev.end.date();
    for (int d = 0; d < 7; ++d) {
      QDate day = week0.addDays(d);
      if (eD < day || sD > day)
        continue;
      // clamp to day
      QDateTime s = ev.start;
      QDateTime e = ev.end;
      if (s.date() < day)
        s = QDateTime(day, QTime(0, 0));
      if (e.date() > day)
        e = QDateTime(day, QTime(23, 59, 59));
      byDay[d].push_back({i, s, e});
    }
  }
  // for each day, assign columns by overlap
  for (int d = 0; d < 7; ++d) {
    auto &list = byDay[d];
    std::sort(list.begin(), list.end(), [](const Node &a, const Node &b) {
      return a.s < b.s || (a.s == b.s && a.e < b.e);
    });
    // sweep-line
    struct Active {
      int col;
      QDateTime end;
      int idx;
    };
    QVector<int> freeCols;
    QList<Active> active;
    QVector<_EventLayout> tmp;
    for (const auto &n : list) {
      // reclaim columns
      for (int i = active.size() - 1; i >= 0; --i) {
        if (active[i].end <= n.s) {
          freeCols.push_back(active[i].col);
          active.removeAt(i);
        }
      }
      int col = freeCols.isEmpty() ? active.size() : freeCols.takeLast();
      active.append({col, n.e, n.idx});
      _EventLayout L;
      L.dayIndex = d;
      L.y1 = yForDateTime(n.s);
      L.y2 = yForDateTime(n.e);
      L.colIndex = col;
      L.idx = n.idx;
      tmp.push_back(L);
    }
    // determine total columns per “cluster”
    // naive: compute for each event the max simultaneous active count
    for (auto &a : tmp) {
      int maxCols = 1;
      for (const auto &b : tmp) {
        if (&a == &b)
          continue;
        bool overlap = !(a.y2 <= b.y1 || b.y2 <= a.y1);
        if (overlap && a.dayIndex == b.dayIndex)
          maxCols = std::max(maxCols, std::max(a.colIndex, b.colIndex) + 1);
      }
      a.colCount = maxCols;
      out.push_back(a);
    }
  }
  // build final rects
  for (auto &L : out) {
    QRect colRect = dayColumnRect(L.dayIndex);
    int pad = 2;
    double top = std::max<double>(L.y1, gridRect().top());
    double bottom = std::min<double>(L.y2, gridRect().bottom());
    if (bottom < gridRect().top() || top > gridRect().bottom()) {
      L.rect = QRectF();
      continue;
    }
    double w = (colRect.width() - 2 * pad) / std::max(1, L.colCount);
    double x = colRect.left() + pad + L.colIndex * w;
    L.rect = QRectF(x, top, w - 2, std::max(8.0, bottom - top));
  }
}

void GCalWeekView::paintEvents(QPainter &p, const QVector<_EventLayout> &lay) {
  QFont f = p.font();
  f.setBold(false);
  for (const auto &L : lay) {
    if (L.rect.isNull() || L.rect.height() <= 0.5)
      continue;
    const auto &ev = events_[L.idx];
    QRectF r = L.rect;
    QColor fill = ev.color.lighter(140);
    QColor border = ev.color.darker(140);
    p.fillRect(r, fill);
    p.setPen(border);
    p.drawRect(r.adjusted(0.5, 0.5, -0.5, -0.5));
    // text
    p.setPen(QColor(40, 40, 40));
    p.setFont(f);
    QString timeStr;
    if (show24h_) {
      timeStr = QString("%1–%2").arg(ev.start.time().toString("HH:mm"),
                                     ev.end.time().toString("HH:mm"));
    } else {
      timeStr = QString("%1–%2").arg(ev.start.time().toString("h:mm AP"),
                                     ev.end.time().toString("h:mm AP"));
    }
    QString text = ev.title.isEmpty() ? timeStr : (ev.title + "  " + timeStr);
    p.save();
    p.setClipRect(r.adjusted(2, 2, -2, -2));
    p.drawText(r.adjusted(6, 4, -6, -4),
               Qt::TextWordWrap | Qt::AlignTop | Qt::AlignLeft, text);
    p.restore();
  }
}

void GCalWeekView::paintNowLine(QPainter &p) {
  QDate today = QDate::currentDate();
  QDate week0 = weekStartFor(focusDate_, weekStart_);
  int dayIdx = week0.daysTo(today);
  if (dayIdx < 0 || dayIdx > 6)
    return;
  QRect col = dayColumnRect(dayIdx);
  double y = yForDateTime(QDateTime::currentDateTime());
  if (y < gridRect().top() || y > gridRect().bottom())
    return;

  QPen pen(QColor(234, 67, 53)); // red
  pen.setWidth(2);
  p.setPen(pen);
  p.drawLine(col.left() + 1, int(y), col.right() - 1, int(y));
  // dot in gutter
  p.setBrush(QColor(234, 67, 53));
  p.drawEllipse(QPoint(col.left(), int(y)), 3, 3);
}

int GCalWeekView::dayAtPos(int x) const {
  for (int i = 0; i < 7; ++i) {
    if (dayColumnRect(i).contains(x, gridRect().top() + 1))
      return i;
  }
  return -1;
}

bool GCalWeekView::eventAtPos(const QPoint &vp, QString *outId) const {
  for (const auto &L : lastLayout_) {
    if (L.rect.contains(vp)) {
      if (outId)
        *outId = events_[L.idx].id;
      return true;
    }
  }
  return false;
}

void GCalWeekView::mousePressEvent(QMouseEvent *e) {
  QPoint vp = e->pos();
  QString id;
  if (eventAtPos(vp, &id)) {
    emit eventClicked(id);
    return;
  }
  int d = dayAtPos(vp.x());
  if (d >= 0) {
    QDateTime t = dateTimeForY(vp.y(), d);
    QDateTime t2 = t.addSecs(gridStepMin_ * 60);
    emit timeSlotClicked(t, t2);
  }
}

void GCalWeekView::mouseDoubleClickEvent(QMouseEvent *e) {
  int d = dayAtPos(e->pos().x());
  if (d >= 0)
    emit emptySpaceDoubleClicked(dateTimeForY(e->pos().y(), d));
}

// ----------------------- END -----------------------
