import QtQuick

// Holds one of a polling service's `consumers` while `active`, so the service only
// polls while something shows its numbers.
//   ConsumerTicket { service: ResourceUsage; active: root.live }
QtObject {
    id: root

    property var service: null
    property bool active: true
    property bool held: false

    function sync() {
        if (!root.service) return;
        if (root.active && !root.held) {
            root.service.consumers++;
            root.held = true;
        } else if (!root.active && root.held) {
            root.service.consumers--;
            root.held = false;
        }
    }

    onActiveChanged: sync()
    Component.onCompleted: sync()
    Component.onDestruction: {
        if (root.held && root.service) root.service.consumers--;
    }
}
