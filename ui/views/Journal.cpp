#include <QLabel>
#include <QTextEdit>
#include <QToolBar>
#include <QVBoxLayout>

#include "Journal.h"
#include "qmarkdowntextedit.h"

Journal::Journal(QWidget *parent) : QWidget(parent) {
  auto *layout = new QVBoxLayout(this);
  layout->setContentsMargins(0, 0, 0, 0);
  layout->setSpacing(0);

  QLabel *label = new QLabel("Journal Entry", this);
  layout->addWidget(label);

  QMarkdownTextEdit *editor = new QMarkdownTextEdit();
  layout->addWidget(editor);
}
