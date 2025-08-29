#include "Toolbar.h"
#include "MainWindow.h"
#include "ads_globals.h"

#include <QCalendarWidget>
#include <QDate>
#include <QIcon>
#include <QLabel>
#include <QMenu>
#include <QStyle>
#include <QToolButton>
#include <QWidgetAction>

MainToolBar::MainToolBar(MainWindow *parent)
    : QToolBar("Main Toolbar", parent) {
  setMovable(false);
  setFloatable(false);

  // Create a tool button with a calendar icon. The button's default action
  // opens today's daily note; the popup menu contains a calendar allowing
  // the user to pick any date to open.
  QToolButton *dailyNoteBtn = new QToolButton(this);

  QIcon calIcon = QIcon::fromTheme("calendar");
  if (calIcon.isNull()) {
    calIcon = style()->standardIcon(QStyle::SP_FileDialogDetailedView);
  }

  // Default action: open today's note
  QAction *openToday = new QAction(calIcon, "Daily Note", dailyNoteBtn);
  connect(openToday, &QAction::triggered, parent, [parent]() {
    auto today = QDate::currentDate();
    auto url = QStringLiteral("/journal/%1").arg(today.toString(Qt::ISODate));
    emit parent->openUrl(url.toStdString());
  });

  // Calendar in a popup menu to pick arbitrary dates
  QMenu *menu = new QMenu(dailyNoteBtn);
  QWidgetAction *calendarAction = new QWidgetAction(menu);
  QCalendarWidget *calendar = new QCalendarWidget(menu);
  calendar->setGridVisible(true);
  calendarAction->setDefaultWidget(calendar);
  menu->addAction(calendarAction);

  connect(calendar, &QCalendarWidget::activated, parent,
          [parent, menu](const QDate &date) {
            qDebug() << "Selected date: " << date.toString(Qt::ISODate);
            auto url =
                QStringLiteral("/journal/%1").arg(date.toString(Qt::ISODate));
            parent->openUrl(url.toStdString());
            menu->hide();
          });

  dailyNoteBtn->setDefaultAction(openToday);
  dailyNoteBtn->setMenu(menu);
  dailyNoteBtn->setPopupMode(QToolButton::MenuButtonPopup);

  addWidget(dailyNoteBtn);
}
