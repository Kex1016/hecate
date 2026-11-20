import QtQuick
import QtQuick.Layouts
import qs.modules.common.widgets.haiiro as H

// The body of a settings page: two columns of rows (SetColumn), scrolled when
// they do not fit. ↑ ↓ move between rows, column by column; the dashboard
// asks it to reveal a row by name after a search.
//   SettingsPage {
//       SetColumn { SetSection { text: "background" } SetChoice { ... } }
//       SetColumn { ... }
//   }
Flickable {
    id: root

    default property alias columns: cols.data
    // what the page saves to, for the window's footer
    property string note: "saved to ~/.config/illogical-impulse/config.json"
    signal leaveRequested()

    contentWidth: width
    contentHeight: cols.implicitHeight
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: contentHeight > height

    RowLayout {
        id: cols
        width: root.width
        spacing: 40
        uniformCellSizes: true
    }

    // Every SetRow on the page, left column first.
    function rows() {
        const out = [];
        const walk = item => {
            for (const c of item.children) {
                if (!c.visible) continue;
                if (c.isSetRow === true) out.push(c);
                else walk(c);
            }
        };
        walk(cols);
        return out;
    }

    function focusFirst() {
        for (const r of root.rows())
            if (r.enabled && r.takeFocus()) {
                root.ensureVisible(r);
                return true;
            }
        return false;
    }

    function navigateFrom(row, dir) {
        const list = root.rows().filter(r => r.enabled && r.focusTarget() !== null);
        const i = list.indexOf(row);
        const next = list[i + dir];
        if (next) {
            next.takeFocus();
            root.ensureVisible(next);
        } else if (dir < 0) {
            root.leaveRequested();
        }
    }

    function ensureVisible(item) {
        const p = item.mapToItem(cols, 0, 0);
        if (p.y < root.contentY) root.contentY = Math.max(0, p.y - 8);
        else if (p.y + item.height > root.contentY + root.height)
            root.contentY = Math.min(root.contentHeight - root.height, p.y + item.height - root.height + 8);
    }

    // Light up the row called `name` and bring it into view.
    function reveal(name) {
        const wanted = String(name).toLowerCase();
        for (const r of root.rows()) {
            if (String(r.name).toLowerCase() === wanted) {
                root.ensureVisible(r);
                r.blink();
                if (r.enabled) r.takeFocus();
                return true;
            }
        }
        return false;
    }
}
