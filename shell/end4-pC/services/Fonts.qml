pragma Singleton

import Quickshell
import QtQuick

Singleton {
    id: root
    property string iconMaterialFamily: materialSymbolsLoader.name

    FontLoader {
        id: materialSymbolsLoader
        source: Qt.resolvedUrl(`${Quickshell.shellPath("assets/fonts")}/MaterialSymbolsLite.ttf`)
    }
}
