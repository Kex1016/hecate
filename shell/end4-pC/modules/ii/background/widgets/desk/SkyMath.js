.pragma library

// Calendar, moon and sun arithmetic for the desktop widgets. No network:
// everything is computed from the date and, for the sun, a latitude and
// longitude (fallback: sunrise 06:00, sunset 18:00 local).

var SYNODIC = 29.530588853
var CHALDEAN = ["saturn", "jupiter", "mars", "sun", "venus", "mercury", "moon"]
// ruler of the first hour of each weekday, Sunday first
var DAY_RULER = ["sun", "moon", "mars", "mercury", "jupiter", "venus", "saturn"]

function dayOfYear(d) {
    var start = new Date(d.getFullYear(), 0, 0)
    return Math.floor((d - start) / 86400000)
}

function daysInYear(d) {
    var y = d.getFullYear()
    return (y % 4 === 0 && y % 100 !== 0) || y % 400 === 0 ? 366 : 365
}

function isoWeek(d) {
    var t = new Date(Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()))
    var day = t.getUTCDay() || 7
    t.setUTCDate(t.getUTCDate() + 4 - day)
    var yearStart = new Date(Date.UTC(t.getUTCFullYear(), 0, 1))
    return Math.ceil(((t - yearStart) / 86400000 + 1) / 7)
}

function dayProgress(d) {
    return (d.getHours() * 3600 + d.getMinutes() * 60 + d.getSeconds()) / 86400
}

// age in days, illuminated fraction 0..1 and the phase name
function moon(d) {
    var jd = d.getTime() / 86400000 + 2440587.5
    var age = (((jd - 2451550.1) % SYNODIC) + SYNODIC) % SYNODIC
    var illum = (1 - Math.cos(2 * Math.PI * age / SYNODIC)) / 2
    var name
    if (age < 1.85) name = "new moon"
    else if (age < 5.54) name = "waxing crescent"
    else if (age < 9.22) name = "first quarter"
    else if (age < 12.91) name = "waxing gibbous"
    else if (age < 16.61) name = "full moon"
    else if (age < 20.3) name = "waning gibbous"
    else if (age < 23.99) name = "last quarter"
    else if (age < 27.68) name = "waning crescent"
    else name = "new moon"
    return { age: age, illum: illum, name: name }
}

var MOON_LINES = {
    "new moon": "The sky is dark and the stars are loud.",
    "waxing crescent": "The moon is a thin smile tonight.",
    "first quarter": "Half a moon, and the other half is keeping a secret.",
    "waxing gibbous": "The moon is nearly there. Nearly.",
    "full moon": "Full moon. Somebody somewhere is howling.",
    "waning gibbous": "The moon is letting go, slowly.",
    "last quarter": "Half a moon, drifting off to bed.",
    "waning crescent": "A last sliver of moon before the dark."
}

function moonGlyph(name) {
    return name === "new moon" ? "○" : name === "full moon" ? "●" : "☾"
}

// Sunrise and sunset (NOAA approximation) for the local calendar day of `d`,
// as Dates, or null at polar latitudes.
function sun(d, lat, lon) {
    var n = dayOfYear(d)
    var g = 2 * Math.PI / 365 * (n - 1)
    var eq = 229.18 * (0.000075 + 0.001868 * Math.cos(g) - 0.032077 * Math.sin(g)
        - 0.014615 * Math.cos(2 * g) - 0.040849 * Math.sin(2 * g))
    var decl = 0.006918 - 0.399912 * Math.cos(g) + 0.070257 * Math.sin(g)
        - 0.006758 * Math.cos(2 * g) + 0.000907 * Math.sin(2 * g)
        - 0.002697 * Math.cos(3 * g) + 0.00148 * Math.sin(3 * g)
    var r = Math.PI / 180
    var c = (Math.cos(90.833 * r) / (Math.cos(lat * r) * Math.cos(decl))) - Math.tan(lat * r) * Math.tan(decl)
    if (c < -1 || c > 1) return null
    var ha = Math.acos(c) / r
    var base = Date.UTC(d.getFullYear(), d.getMonth(), d.getDate())
    var rise = base + (720 - 4 * (lon + ha) - eq) * 60000
    var set = base + (720 - 4 * (lon - ha) - eq) * 60000
    // keep both on the calendar day the user is living in
    var local = d.getTimezoneOffset() * 60000
    var dayStart = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()
    while (rise - dayStart > 86400000 * 0.75) { rise -= 86400000; set -= 86400000 }
    while (rise - dayStart < -86400000 * 0.25) { rise += 86400000; set += 86400000 }
    return { rise: new Date(rise), set: new Date(set) }
}

function sunDay(d, loc) {
    var s = loc && loc.valid ? sun(d, loc.lat, loc.lon) : null
    if (s) return s
    return {
        rise: new Date(d.getFullYear(), d.getMonth(), d.getDate(), 6, 0),
        set: new Date(d.getFullYear(), d.getMonth(), d.getDate(), 18, 0)
    }
}

function clock(d) {
    return ("0" + d.getHours()).slice(-2) + ":" + ("0" + d.getMinutes()).slice(-2)
}

// What the sky shows now: the next set and rise, and the planetary hour.
function sky(now, loc) {
    var events = []
    for (var k = -1; k <= 1; k++) {
        var day = new Date(now.getFullYear(), now.getMonth(), now.getDate() + k, 12)
        var s = sunDay(day, loc)
        events.push({ t: s.rise.getTime(), kind: "rise" })
        events.push({ t: s.set.getTime(), kind: "set" })
    }
    events.sort(function (a, b) { return a.t - b.t })
    var nowT = now.getTime()
    var i = 0
    while (i < events.length - 2 && events[i + 1].t <= nowT) i++
    var from = events[i], to = events[i + 1]
    var isDay = from.kind === "rise"
    // the weekday that rules the hours is the one the last sunrise fell on
    var sunriseAt = isDay ? from.t : (i > 0 ? events[i - 1].t : from.t - 12 * 3600000)
    var len = (to.t - from.t) / 12
    var k2 = Math.max(0, Math.min(11, Math.floor((nowT - from.t) / len)))
    var hourNo = (isDay ? 0 : 12) + k2
    var ruler = DAY_RULER[new Date(sunriseAt).getDay()]
    var planet = CHALDEAN[(CHALDEAN.indexOf(ruler) + hourNo) % 7]
    var nextSet, nextRise
    for (var j = 0; j < events.length; j++) {
        if (events[j].t > nowT) {
            if (events[j].kind === "set" && nextSet === undefined) nextSet = events[j].t
            if (events[j].kind === "rise" && nextRise === undefined) nextRise = events[j].t
        }
    }
    return {
        isDay: isDay,
        planet: planet,
        planetUntil: new Date(from.t + (k2 + 1) * len),
        nextSet: new Date(nextSet),
        nextRise: new Date(nextRise)
    }
}
