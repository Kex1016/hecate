import QtQuick
import Quickshell
import Quickshell.Io
import qs.modules.common
import qs.modules.common.functions

Process {
    id: screenshotProc
    running: true
    property string screenshotDir: Directories.screenshotTemp
    required property ShellScreen screen
    property string screenshotPath: `${screenshotDir}/image-${screen.name}.png`
    // A failed capture used to go unnoticed until the saved file turned out empty: say what grim said.
    command: ["bash", "-c", `mkdir -p '${StringUtils.shellSingleQuoteEscape(screenshotDir)}' && err=$(mktemp) && `
        + `if grim -o '${StringUtils.shellSingleQuoteEscape(screen.name)}' '${StringUtils.shellSingleQuoteEscape(screenshotPath)}' 2>"$err"; then rm -f "$err"; `
        + `else notify-send -a "Screen Snip" -i image-x-generic "Screenshot failed" "grim could not capture ${StringUtils.shellSingleQuoteEscape(screen.name)}: $(tr '\\n' ' ' < "$err")"; rm -f "$err" '${StringUtils.shellSingleQuoteEscape(screenshotPath)}'; exit 1; fi`]
}
