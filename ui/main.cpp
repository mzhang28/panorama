#include "MainWindow.h"
#include <QApplication>
#include <QCommandLineParser>

int main(int argc, char *argv[]) {
  QApplication app(argc, argv);

  QCommandLineParser parser;
  parser.setApplicationDescription("panorama");
  parser.addHelpOption();

  QCommandLineOption daemonOption("daemon", "Run as background daemon");
  parser.addOption(daemonOption);
  parser.process(app);

  if (parser.isSet(daemonOption)) {
    qDebug() << "Running as daemon...";
    // start your daemon loop here
    return app.exec();
  } else {
    qDebug() << "Running GUI...";
    MainWindow window;
    window.setWindowTitle("Panorama");
    window.show();

    return app.exec();
  }
}
