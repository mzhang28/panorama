#include "MainWindow.h"
#include <QApplication>

int main(int argc, char *argv[]) {
  QApplication app(argc, argv);

  MainWindow window;
  window.setWindowTitle("Panorama");
  window.show();

  return app.exec();
}
