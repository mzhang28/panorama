#include "MainWindow.h"
#include <QApplication>
#include <QCommandLineParser>

#include "panorama-core/src/lib.rs.cc"
#include "panorama-core/src/lib.rs.h"

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
    run_bridge(argc, argv);
    return 0;
  } else {
    qDebug() << "Running GUI...";
    MainWindow window;
    window.setWindowTitle("Panorama");
    window.show();

    return app.exec();
  }
}
