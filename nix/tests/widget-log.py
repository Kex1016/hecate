"""What the shell's journal must not hold once the desktop widgets have loaded.

Used by vm-shell.nix, which loads the module by path, and runnable on its own: running this file
runs the unit tests below, which the `shell-widget-log` check does without a VM.

The shell logs a QML problem as `<file url>:<line>:<column>: <message>`, so a message about a
widget names a file under background/widgets/ (the window widgets, free/ and the kit's use inside
them are all there) with a line number after it. The loader and the widget state service also say
in their own words when a widget did not load; those go through console.warn, which carries no
file. Their text is matched here, so a rename in WidgetsLoader.qml or DesktopWidgets.qml has to be
followed by RULES.

Logging that is not a defect is not matched: `qml: [...]` lines from console.log, the icon
theme's chatter, and a runtime tool the VM lacks (a player, a network) unless it comes with a
QML location in a widget file. Add a pattern to IGNORE with its reason when a widget logs
something legitimate there.
"""

import re
import sys
import unittest

RULES = [
    # Qt's location prefix on a message about a widget file: any error or warning from it.
    re.compile(r"background/widgets/\S+\.qml:\d+"),
    # WidgetsLoader.qml: a free form that exists and did not load, or a window that did not.
    re.compile(r"\[WidgetsLoader\] .*(failed to load|could not load)"),
    # DesktopWidgets.qml: the saved state could not be used.
    re.compile(r"\[DesktopWidgets\] (state file is not JSON|could not read)"),
]

IGNORE = [
    re.compile(r"icon", re.I),
]


def widget_problems(log):
    """The lines of a journal excerpt that report a widget failing, in order."""
    return [
        line.strip()
        for line in log.splitlines()
        if any(r.search(line) for r in RULES) and not any(i.search(line) for i in IGNORE)
    ]


class Tests(unittest.TestCase):
    P = "file:///nix/store/abc-hecate-shell/modules/ii/background/widgets"

    def test_qml_error_in_a_free_widget(self):
        line = f"Nov 22 10:00:01 m hecate-shell[88]: WARN scene: {self.P}/free/NoteFree.qml:23:5: TypeError: Cannot read property 'x' of undefined"
        self.assertEqual(widget_problems(line), [line.strip()])

    def test_qml_error_in_a_window_widget(self):
        line = f"hecate-shell[88]: ERROR scene: {self.P}/desk/DisksWidget.qml:9:1: Type Foo unavailable"
        self.assertEqual(widget_problems(line), [line])

    def test_loader_fallback(self):
        line = "hecate-shell[88]: WARN qml: [WidgetsLoader] free form of disks failed to load, showing the window: free/DisksFree.qml"
        self.assertEqual(widget_problems(line), [line])
        line = "hecate-shell[88]: WARN qml: [WidgetsLoader] could not load desk/DisksWidget.qml"
        self.assertEqual(widget_problems(line), [line])

    def test_state_file(self):
        line = "hecate-shell[88]: WARN qml: [DesktopWidgets] state file is not JSON, starting over: SyntaxError"
        self.assertEqual(widget_problems(line), [line])

    def test_quiet_lines(self):
        quiet = "\n".join(
            [
                "hecate-shell[88]: INFO qml: [To Do] File not found, creating new file.",
                "hecate-shell[88]: INFO qml: [DesktopWidgets] saved",
                f"hecate-shell[88]: WARN scene: {self.P}/free/NoteFree.qml has an icon: missing icon x.svg",
                "hecate-shell[88]: WARN scene: file:///nix/store/abc-hecate-shell/modules/ii/bar/Bar.qml:12:3: TypeError: x",
                "",
            ]
        )
        self.assertEqual(widget_problems(quiet), [])

    def test_only_the_bad_lines_come_back(self):
        good = "hecate-shell[88]: INFO qml: [WidgetsLoader] ready"
        bad = f"hecate-shell[88]: WARN scene: {self.P}/free/SkyFree.qml:3:1: Unable to assign QString to int"
        self.assertEqual(widget_problems(f"{good}\n{bad}\n{good}"), [bad])


if __name__ == "__main__":
    sys.argv = sys.argv[:1]
    unittest.main(verbosity=2)
