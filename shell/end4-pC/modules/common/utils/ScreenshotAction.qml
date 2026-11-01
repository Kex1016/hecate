pragma ComponentBehavior: Bound
pragma Singleton
import qs.modules.common
import qs.modules.common.utils
import qs.modules.common.functions
import qs.modules.common.widgets
import qs.services
import QtQuick
import QtQuick.Controls
import Qt.labs.synchronizer
import Quickshell

Singleton {
    id: root

    enum Action {
        Copy,
        Edit,
        Search,
        CharRecognition,
        Record,
        RecordWithSound
    }

    property string imageSearchEngineBaseUrl: Config.options.search.imageSearch.imageSearchEngineBaseUrl
    property string fileUploadApiEndpoint: "https://uguu.se/upload"

    function getCommand(x, y, width, height, screenshotPath, action, saveDir = "", systemAudio = false, microphone = false) {
        // Set command for action
        const rx = Math.round(x);
        const ry = Math.round(y);
        const rw = Math.round(width);
        const rh = Math.round(height);
        const capture = `'${StringUtils.shellSingleQuoteEscape(screenshotPath)}'`
        // grim always writes PNG; say so on both ends rather than let magick guess from the file.
        const cropBase = `magick 'png:${StringUtils.shellSingleQuoteEscape(screenshotPath)}' `
            + `-crop ${rw}x${rh}+${rx}+${ry} +repage`
        const cropToStdout = `${cropBase} png:-`
        const cropInPlace = `${cropBase} 'png:${StringUtils.shellSingleQuoteEscape(screenshotPath)}'`
        const cleanup = `rm -f ${capture}`
        // Report a failed step instead of leaving an empty file or clipboard behind.
        const failed = (what, keepCapture = false) => `{ ${keepCapture ? ":" : cleanup}; notify-send -a "Screen Snip" -i image-x-generic "Screenshot failed" "${what}: $(tr '\\n' ' ' < "$err")"; rm -f "$err"; exit 1; }`
        const withErr = (cmd) => `err=$(mktemp) && ${cmd}`
        // magick cannot tell an empty or truncated capture from a format it lacks: look at the file first,
        // and when the crop still fails, say whether this ImageMagick can read PNG at all.
        const captureIsPng = `{ [ -s ${capture} ] && [ "$(head -c 8 ${capture} | od -An -tx1 | tr -d ' \\n')" = 89504e470d0a1a0a ]; }`
        const captureCheck = `${captureIsPng} || { echo "$(stat -c %s ${capture} 2>/dev/null || echo 0) bytes, not a PNG, written $(stat -c %y ${capture} 2>/dev/null | cut -d. -f1), left at ${capture}" > "$err"; ${failed("the capture is unusable", true)}; }`
        const cropFailed = (what) => `{ magick -list format 2>/dev/null | grep -qE '^ *PNG\\*? ' || echo "($(magick -version | head -n 1) has no PNG coder)" >> "$err"; ${failed(what)}; }`
        const slurpRegion = `${rx},${ry} ${rw}x${rh}`
        const uploadAndGetUrl = (filePath) => {
            return `curl -sF files[]=@'${StringUtils.shellSingleQuoteEscape(filePath)}' ${root.fileUploadApiEndpoint} | jq -r '.files[0].url'`
        }
        const audioFlags = (systemAudio ? " --sound" : "") + (microphone ? " --mic" : "")
        const annotationCommand = `${Config.options.regionSelector.annotation.useSatty ? "satty" : "swappy"} -f -`;
        switch (action) {
            case ScreenshotAction.Action.Copy:
                if (saveDir === "") {
                    // not saving the screenshot, just copy to clipboard
                    return ["bash", "-c", withErr(`${captureCheck} && ${cropToStdout} 2>"$err" > "$err.png" && [ -s "$err.png" ] && wl-copy < "$err.png" && rm -f "$err" "$err.png" && ${cleanup} || { rm -f "$err.png"; ${cropFailed("could not crop the capture")}; }`)]
                    break;
                }
                // Crop into a temp file first: a file is saved only when there is a picture to save.
                return [
                    "bash", "-c",
                    withErr(`${captureCheck} && mkdir -p '${StringUtils.shellSingleQuoteEscape(saveDir)}' && \
                    saveFileName="screenshot-$(date '+%Y-%m-%d_%H.%M.%S').png" && \
                    savePath='${StringUtils.shellSingleQuoteEscape(saveDir)}'/"$saveFileName" && \
                    { ${cropToStdout} 2>"$err" > "$savePath.part" && [ -s "$savePath.part" ] || { rm -f "$savePath.part"; ${cropFailed("could not crop the capture")}; }; } && \
                    rm -f "$err" && mv "$savePath.part" "$savePath" && wl-copy < "$savePath" && \
                    ${cleanup}`)
                ]

                break;
            case ScreenshotAction.Action.Edit:
                return ["bash", "-c", `${cropToStdout} | ${annotationCommand} && ${cleanup}`]
                break;
            case ScreenshotAction.Action.Search:
                return ["bash", "-c", `${cropInPlace} && xdg-open "${root.imageSearchEngineBaseUrl}$(${uploadAndGetUrl(screenshotPath)})" && ${cleanup}`]
                break;
            case ScreenshotAction.Action.CharRecognition:
                return ["bash", "-c", `${cropInPlace} && tesseract '${StringUtils.shellSingleQuoteEscape(screenshotPath)}' stdout -l $(tesseract --list-langs | awk 'NR>1{print $1}' | tr '\\n' '+' | sed 's/\\+$/\\n/') | wl-copy && ${cleanup}`]
                break;
            case ScreenshotAction.Action.Record:
                return ["bash", "-c", `${Directories.recordScriptPath} --region '${slurpRegion}'${audioFlags}`]
                break;
            case ScreenshotAction.Action.RecordWithSound:
                return ["bash", "-c", `${Directories.recordScriptPath} --region '${slurpRegion}' --sound${microphone ? " --mic" : ""}`]
                break;
            default:
                console.warn("[Region Selector] Unknown snip action, skipping snip.");
                return;
        }
    }
}
