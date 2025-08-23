#pragma once

#include <QAbstractScrollArea>
#include <QColor>
#include <QDate>
#include <QDateTime>
#include <QFont>
#include <QVector>

class GCalWeekView : public QAbstractScrollArea {
  Q_OBJECT
public:
  struct Event {
    QString id;
    QDateTime start;
    QDateTime end;
    QString title;
    QColor color = QColor(66, 133, 244); // Google-ish blue
    bool allDay = false;
  };

  explicit GCalWeekView(QWidget *parent = nullptr);

  // Data API
  void clearEvents();
  void addEvent(const Event &ev); // start < end, same timezone
  bool removeEvent(const QString &id);
  bool hasEvent(const QString &id) const;

  // View config
  void setWeekStart(Qt::DayOfWeek d); // default: Monday
  Qt::DayOfWeek weekStart() const;

  void setFocusDate(const QDate &d); // which week to show
  QDate focusDate() const;

  void setHourRange(int fromHour, int toHour); // [0..24], default 0..24
  void setTimeGridStep(int minutes);           // 15/30/60; default 30
  void set24Hour(bool on);                     // default true
  void setAllDayRowVisible(bool on);           // default true

  // Appearance
  void setHeaderHeight(int px);       // default 36
  void setAllDayHeight(int px);       // default 28
  void setTimeGutterWidth(int px);    // default 54
  void setRowHeightPxPerHour(int px); // default 64
  void setShowNowLine(bool on);       // default true

  QSize sizeHint() const override;

signals:
  void timeSlotClicked(const QDateTime &start, const QDateTime &end);
  void eventClicked(const QString &id);
  void emptySpaceDoubleClicked(const QDateTime &when);

protected:
  void paintEvent(QPaintEvent *e) override;
  void resizeEvent(QResizeEvent *e) override;
  void mousePressEvent(QMouseEvent *e) override;
  void mouseDoubleClickEvent(QMouseEvent *e) override;
  void wheelEvent(QWheelEvent *e) override;
  void timerEvent(QTimerEvent *e) override;

private:
  struct _EventLayout {
    int dayIndex;  // 0..6
    double y1, y2; // viewport coords
    int colIndex;
    int colCount;
    QRectF rect; // final rect (set late)
    int idx;     // index into events_
  };

  // geometry helpers
  QRect headerRect() const;
  QRect allDayRect() const;
  QRect gridRect() const;
  QRect dayColumnRect(int dayIndex) const;
  QRect timeGutterRect() const;
  double yForDateTime(const QDateTime &dt) const;
  QDateTime dateTimeForY(double y, int dayIndex) const;

  // layout + paint
  void ensureScrollbars();
  void layoutTimedEvents(QVector<_EventLayout> &out) const;
  void paintHeaders(QPainter &p);
  void paintAllDay(QPainter &p);
  void paintGrid(QPainter &p);
  void paintEvents(QPainter &p, const QVector<_EventLayout> &lay);
  void paintNowLine(QPainter &p);

  // hit testing
  int dayAtPos(int x) const;
  bool eventAtPos(const QPoint &vp, QString *outId) const;

  // data
  QVector<Event> events_;
  QDate focusDate_; // any date within the shown week
  Qt::DayOfWeek weekStart_ = Qt::Monday;

  int headerH_ = 36;
  int allDayH_ = 28;
  int timeGutterW_ = 54;
  int pxPerHour_ = 64;
  int gridStepMin_ = 30;
  int hourFrom_ = 0, hourTo_ = 24;
  bool show24h_ = true;
  bool showAllDay_ = true;
  bool showNowLine_ = true;

  mutable QVector<_EventLayout> lastLayout_; // for hit testing
  int nowTimerId_ = 0;
};
