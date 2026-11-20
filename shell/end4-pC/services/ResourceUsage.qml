pragma Singleton
pragma ComponentBehavior: Bound

import qs.modules.common
import QtQuick
import Quickshell
import Quickshell.Io

/**
 * Polled resource usage: RAM, swap, CPU, GPU, network throughput and disks.
 * Nothing is read while `consumers` is 0; a widget bumps it while it shows.
 */
Singleton {
    id: root
    property real memoryTotal: 1
    property real memoryFree: 0
    property real memoryUsed: memoryTotal - memoryFree
    property real memoryUsedPercentage: memoryUsed / memoryTotal
    property real swapTotal: 1
    property real swapFree: 0
    property real swapUsed: swapTotal - swapFree
    property real swapUsedPercentage: swapTotal > 0 ? (swapUsed / swapTotal) : 0
    property real cpuUsage: 0
    property var previousCpuStats

    property string maxAvailableMemoryString: kbToGbString(ResourceUsage.memoryTotal)
    property string maxAvailableSwapString: kbToGbString(ResourceUsage.swapTotal)
    property string maxAvailableCpuString: "--"

    readonly property int historyLength: Config?.options.resources.historyLength ?? 60
    property list<real> cpuUsageHistory: []
    property list<real> memoryUsageHistory: []
    property list<real> swapUsageHistory: []

    property real cpuTemp: 0

    property int diskTick: 0
    property int consumers: 0
    property real diskTotal: 1
    property real diskUsed: 0
    property real diskFree: 0
    property real diskUsedPercentage: diskTotal > 0 ? diskUsed / diskTotal : 0
    property list<real> diskUsageHistory: []
    property string maxAvailableDiskString: kbToGbString(diskTotal)

    property string thermalPath: ""

    // GPU busy fraction, 0-1. AMD and other DRM drivers that expose gpu_busy_percent are read
    // from sysfs; NVIDIA through nvidia-smi. Intel exposes no busy figure without root, so
    // gpuAvailable stays false there and widgets leave the GPU out.
    property bool gpuAvailable: false
    property real gpuUsage: 0
    property list<real> gpuUsageHistory: []
    // "file:<path>", "nvidia" or "" while unknown or absent
    property string gpuSource: ""

    // Network: the default route's interface and its throughput in bytes per second.
    // History covers the last two minutes, whatever the update interval is.
    property string netInterface: ""
    property real netRxRate: 0
    property real netTxRate: 0
    readonly property int netHistoryLength: Math.max(10, Math.ceil(120000 / (Config.options?.resources?.updateInterval ?? 3000)))
    property list<real> netRxHistory: []
    property list<real> netTxHistory: []
    property var previousNetStats: null

    // Mounted block devices: [{ mount, source, fstype, total, used, free, usedPercentage }],
    // sizes in KiB, "/" first. One row per device, at its shortest mount point, so btrfs
    // subvolumes and bind mounts (/nix/store) do not repeat a disk.
    property var disks: []

    Process {
        id: findThermalPathProc
        running: true
        command: ["sh", "-c", "for h in /sys/class/hwmon/hwmon*; do [ -d \"$h\" ] || continue; for l in \"$h\"/temp*_label; do [ -f \"$l\" ] || continue; if grep -qE 'Package id 0|Tctl|Tdie' \"$l\" 2>/dev/null; then inp=\"${l%_label}_input\"; [ -f \"$inp\" ] && echo \"$inp\" && exit 0; fi; done; done; for z in /sys/class/thermal/thermal_zone*; do [ -d \"$z\" ] || continue; type=$(cat \"$z/type\" 2>/dev/null); case \"$type\" in x86_pkg_temp|cpu*|TCPU) [ -f \"$z/temp\" ] && echo \"$z/temp\" && exit 0;; esac; done; for t in /sys/class/hwmon/hwmon*/temp1_input /sys/class/thermal/thermal_zone0/temp; do [ -f \"$t\" ] && echo \"$t\" && exit 0; done"]
        stdout: StdioCollector {
            onStreamFinished: {
                const foundPath = text.trim();
                if (foundPath.length > 0) {
                    root.thermalPath = foundPath;
                    fileTemp.reload();
                }
            }
        }
    }

    FileView {
        id: fileTemp
        path: root.thermalPath
        printErrors: false
        onLoaded: {
            const raw = parseFloat(fileTemp.text().trim());
            if (!isNaN(raw) && raw > 0) {
                root.cpuTemp = raw > 200 ? Math.round(raw / 100) / 10 : raw;
            }
        }
    }

    Process {
        id: tempProcFallback
        command: ["bash", "-c", "sensors 2>/dev/null | grep -E 'Package id 0|Tctl|Tdie' | grep -oP '\\+\\K[0-9.]+(?=°C)' | head -1"]
        stdout: StdioCollector {
            onStreamFinished: {
                const parsed = parseFloat(text.trim());
                if (!isNaN(parsed) && parsed > 0) {
                    root.cpuTemp = parsed;
                }
            }
        }
    }

    Process {
        id: diskProc
        command: ["df", "-k", "/"]
        stdout: StdioCollector {
            onStreamFinished: {
                const lines = text.trim().split("\n");
                if (lines.length >= 2) {
                    const parts = lines[1].trim().split(/\s+/).map(Number);
                    if (parts.length >= 4) {
                        root.diskTotal = parts[1];
                        root.diskUsed  = parts[2];
                        root.diskFree  = parts[3];
                    }
                }
            }
        }
    }

    function kbToGbString(kb) {
        return (kb / (1024 * 1024)).toFixed(1) + " GB"
    }

    function updateMemoryUsageHistory() {
        memoryUsageHistory = [...memoryUsageHistory, memoryUsedPercentage]
        if (memoryUsageHistory.length > historyLength) memoryUsageHistory.shift()
    }
    function updateSwapUsageHistory() {
        swapUsageHistory = [...swapUsageHistory, swapUsedPercentage]
        if (swapUsageHistory.length > historyLength) swapUsageHistory.shift()
    }
    function updateCpuUsageHistory() {
        cpuUsageHistory = [...cpuUsageHistory, cpuUsage]
        if (cpuUsageHistory.length > historyLength) cpuUsageHistory.shift()
    }
    function updateDiskUsageHistory() {
        diskUsageHistory = [...diskUsageHistory, diskUsedPercentage]
        if (diskUsageHistory.length > historyLength) diskUsageHistory.shift()
    }
    function updateGpuUsageHistory() {
        if (!gpuAvailable) return
        gpuUsageHistory = [...gpuUsageHistory, gpuUsage]
        if (gpuUsageHistory.length > historyLength) gpuUsageHistory.shift()
    }
    function updateHistories() {
        updateMemoryUsageHistory()
        updateSwapUsageHistory()
        updateCpuUsageHistory()
        updateDiskUsageHistory()
        updateGpuUsageHistory()
    }

    function formatRate(bytesPerSecond) {
        const units = ["B/s", "KB/s", "MB/s", "GB/s"]
        let v = Math.max(0, bytesPerSecond)
        let i = 0
        while (v >= 1000 && i < units.length - 1) { v /= 1000; i++ }
        return (i === 0 || v >= 100 ? Math.round(v) : v.toFixed(1)) + " " + units[i]
    }
    // KiB to "412G" / "1.7T", the way df -h prints it
    function formatSize(kb) {
        const units = ["K", "M", "G", "T", "P"]
        let v = Math.max(0, kb)
        let i = 0
        while (v >= 1024 && i < units.length - 1) { v /= 1024; i++ }
        return (v >= 10 || i === 0 ? Math.round(v) : v.toFixed(1)) + units[i]
    }

    function updateGpu() {
        if (root.gpuSource.startsWith("file:")) {
            fileGpu.reload()
            const raw = parseFloat(fileGpu.text().trim())
            if (!isNaN(raw)) root.gpuUsage = Math.max(0, Math.min(1, raw / 100))
        } else if (root.gpuSource === "nvidia" && !nvidiaProc.running) {
            nvidiaProc.running = true
        }
    }

    function updateNetwork() {
        fileNetRoute.reload()
        fileNetDev.reload()
        const route = fileNetRoute.text().split("\n").slice(1).map(l => l.trim().split(/\s+/))
        const def = route.find(f => f.length > 2 && f[1] === "00000000")
        const devs = {}
        for (const line of fileNetDev.text().split("\n").slice(2)) {
            const m = line.match(/^\s*([^:\s]+):\s*(.*)$/)
            if (!m) continue
            const f = m[2].trim().split(/\s+/).map(Number)
            devs[m[1]] = { rx: f[0] || 0, tx: f[8] || 0 }
        }
        let iface = def ? def[0] : ""
        if (!devs[iface]) {
            // no default route: the busiest interface that is not loopback
            iface = Object.keys(devs).filter(n => n !== "lo")
                .sort((a, b) => (devs[b].rx + devs[b].tx) - (devs[a].rx + devs[a].tx))[0] ?? ""
        }
        const now = Date.now()
        const cur = devs[iface]
        if (cur && root.previousNetStats && root.previousNetStats.iface === iface) {
            const dt = (now - root.previousNetStats.time) / 1000
            if (dt > 0) {
                root.netRxRate = Math.max(0, cur.rx - root.previousNetStats.rx) / dt
                root.netTxRate = Math.max(0, cur.tx - root.previousNetStats.tx) / dt
            }
        } else {
            root.netRxRate = 0
            root.netTxRate = 0
        }
        if (iface !== root.netInterface) {
            root.netInterface = iface
            root.netRxHistory = []
            root.netTxHistory = []
        }
        root.previousNetStats = cur ? { iface, time: now, rx: cur.rx, tx: cur.tx } : null
        root.netRxHistory = [...root.netRxHistory, root.netRxRate].slice(-root.netHistoryLength)
        root.netTxHistory = [...root.netTxHistory, root.netTxRate].slice(-root.netHistoryLength)
    }

    Timer {
        interval: 1
        running: root.consumers > 0
        repeat: true
        onTriggered: {
            fileMeminfo.reload()
            fileStat.reload()

            if (root.thermalPath.length > 0) {
                fileTemp.reload()
                const raw = parseFloat(fileTemp.text().trim())
                if (!isNaN(raw) && raw > 0) {
                    root.cpuTemp = raw > 200 ? Math.round(raw / 100) / 10 : raw
                }
            } else if (!findThermalPathProc.running) {
                tempProcFallback.running = false
                tempProcFallback.running = true
            }

            if (root.diskTick % 10 === 0) {
                diskProc.running = false
                diskProc.running = true
                disksProc.running = false
                disksProc.running = true
            }
            root.diskTick++

            const textMeminfo = fileMeminfo.text()
            memoryTotal = Number(textMeminfo.match(/MemTotal: *(\d+)/)?.[1] ?? 1)
            memoryFree  = Number(textMeminfo.match(/MemAvailable: *(\d+)/)?.[1] ?? 0)
            swapTotal   = Number(textMeminfo.match(/SwapTotal: *(\d+)/)?.[1] ?? 1)
            swapFree    = Number(textMeminfo.match(/SwapFree: *(\d+)/)?.[1] ?? 0)

            const textStat = fileStat.text()
            const cpuLine  = textStat.match(/^cpu\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(\d+)/)
            if (cpuLine) {
                const stats = cpuLine.slice(1).map(Number)
                const total = stats.reduce((a, b) => a + b, 0)
                const idle  = stats[3]
                if (previousCpuStats) {
                    const totalDiff = total - previousCpuStats.total
                    const idleDiff  = idle  - previousCpuStats.idle
                    cpuUsage = totalDiff > 0 ? (1 - idleDiff / totalDiff) : 0
                }
                previousCpuStats = { total, idle }
            }

            root.updateGpu()
            root.updateNetwork()

            root.updateHistories()
            interval = Config.options?.resources?.updateInterval ?? 3000
        }
    }

    FileView { id: fileMeminfo; path: "/proc/meminfo" }
    FileView { id: fileNetDev; path: "/proc/net/dev"; printErrors: false }
    FileView { id: fileNetRoute; path: "/proc/net/route"; printErrors: false }
    FileView {
        id: fileGpu
        path: root.gpuSource.startsWith("file:") ? root.gpuSource.slice(5) : ""
        printErrors: false
    }

    Process {
        id: findGpuProc
        running: true
        command: ["sh", "-c", "for f in /sys/class/drm/card[0-9]*/device/gpu_busy_percent; do [ -r \"$f\" ] && echo \"file:$f\" && exit 0; done; command -v nvidia-smi >/dev/null 2>&1 && echo nvidia"]
        stdout: StdioCollector {
            onStreamFinished: {
                root.gpuSource = text.trim()
                root.gpuAvailable = root.gpuSource.length > 0
            }
        }
    }

    Process {
        id: nvidiaProc
        command: ["nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits"]
        stdout: StdioCollector {
            onStreamFinished: {
                // one line per GPU: show the busiest
                const values = text.trim().split("\n").map(v => parseFloat(v)).filter(v => !isNaN(v))
                if (values.length > 0) root.gpuUsage = Math.max(0, Math.min(1, Math.max(...values) / 100))
            }
        }
        onExited: (exitCode, exitStatus) => {
            if (exitCode !== 0) {
                root.gpuSource = ""
                root.gpuAvailable = false
            }
        }
    }

    Process {
        id: disksProc
        environment: ({ LANG: "C", LC_ALL: "C" })
        command: ["df", "-k", "--output=source,fstype,size,used,avail,target"]
        stdout: StdioCollector {
            onStreamFinished: {
                const skipTypes = ["squashfs", "iso9660", "overlay", "tmpfs", "devtmpfs", "efivarfs", "ramfs"]
                const bySource = {}
                for (const line of text.trim().split("\n").slice(1)) {
                    const m = line.match(/^(\S+)\s+(\S+)\s+(\d+)\s+(\d+)\s+(\d+)\s+(.+)$/)
                    if (!m) continue
                    const [, source, fstype, size, used, avail, mount] = m
                    if (!source.startsWith("/dev/") || source.startsWith("/dev/loop")) continue
                    if (skipTypes.includes(fstype)) continue
                    if (mount === "/boot" || mount.startsWith("/boot/") || mount === "/efi") continue
                    if (Number(size) <= 0) continue
                    const prev = bySource[source]
                    if (prev && prev.mount.length <= mount.length) continue
                    const total = Number(size)
                    bySource[source] = {
                        mount, source, fstype, total,
                        used: Number(used),
                        free: Number(avail),
                        // what df -h shows as Use%: used out of what the user can fill
                        usedPercentage: Number(used) / Math.max(1, Number(used) + Number(avail)),
                    }
                }
                root.disks = Object.values(bySource).sort((a, b) =>
                    a.mount === "/" ? -1 : b.mount === "/" ? 1 : a.mount.localeCompare(b.mount))
            }
        }
    }
    FileView { id: fileStat;    path: "/proc/stat" }

    Process {
        id: findCpuMaxFreqProc
        environment: ({ LANG: "C", LC_ALL: "C" })
        command: ["bash", "-c", "lscpu | grep 'CPU max MHz' | awk '{print $4}'"]
        running: true
        stdout: StdioCollector {
            id: outputCollector
            onStreamFinished: {
                root.maxAvailableCpuString = (parseFloat(outputCollector.text) / 1000).toFixed(0) + " GHz"
            }
        }
    }
}
