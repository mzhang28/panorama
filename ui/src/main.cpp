#include <QApplication>
#include <QWidget>

int main(int argc, char *argv[]) {
  QApplication app(argc, argv);

  QWidget window;
  window.setWindowTitle("hello world");
  window.resize(400, 300); // optional, default size
  window.show();

  return app.exec();
}
