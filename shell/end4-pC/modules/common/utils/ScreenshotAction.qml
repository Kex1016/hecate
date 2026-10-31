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
        const cropBase = `magick '${StringUtils.shellSingleQuoteEscape(screenshotPath)}' `
            + `-crop ${rw}x${rh}+${rx}+${ry} +repage`
        // The temp file has no extension, so name the output format rather than leave it to be guessed.
        const cropToStdout = `${cropBase} png:-`
        const cropInPlace = `${cropBase} 'png:${StringUtils.shellSingleQuoteEscape(screenshotPath)}'`
        const cleanup = `rm -f '${StringUtils.shellSingleQuoteEscape(screenshotPath)}'`
        // Report a failed step instead of leaving an empty file or clipboard behind.
        const failed = (what) => `{ ${cleanup}; notify-send -a "Screen Snip" -i image-x-generic "Screenshot failed" "${what}: $(tr '\\n' ' ' < "$err")"; rm -f "$err"; exit 1; }`
        const withErr = (cmd) => `err=$(mktemp) && ${cmd}`
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
                    return ["bash", "-c", withErr(`${cropToStdout} 2>"$err" > "$err.png" && [ -s "$err.png" ] && wl-copy < "$err.png" && rm -f "$err" "$err.png" && ${cleanup} || { rm -f "$err.png"; ${failed("could not crop the capture")}; }`)]
                    break;
                }
                // Crop into a temp file first: a file is saved only when there is a picture to save.
                return [
                    "bash", "-c",
                    withErr(`mkdir -p '${StringUtils.shellSingleQuoteEscape(saveDir)}' && \
                    saveFileName="screenshot-$(date '+%Y-%m-%d_%H.%M.%S').png" && \
                    savePath='${StringUtils.shellSingleQuoteEscape(saveDir)}'/"$saveFileName" && \
                    { ${cropToStdout} 2>"$err" > "$savePath.part" && [ -s "$savePath.part" ] || { rm -f "$savePath.part"; ${failed("could not crop the capture")}; }; } && \
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
