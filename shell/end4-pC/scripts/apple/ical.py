"""iCalendar (RFC 5545) and vCard (RFC 6350 / 2426) reading, recurrence expansion and
event writing for dav.py. Standard library only."""

from __future__ import annotations

import datetime as dt
import re
import uuid
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

# ── content lines ────────────────────────────────────────────────────────────────


def unfold(text: str) -> list[str]:
    """Content lines with folding (CRLF + space or tab) undone."""
    out: list[str] = []
    for raw in text.replace("\r\n", "\n").replace("\r", "\n").split("\n"):
        if raw[:1] in (" ", "\t") and out:
            out[-1] += raw[1:]
        elif raw.strip():
            out.append(raw)
    return out


def parse_line(line: str) -> tuple[str, dict[str, list[str]], str] | None:
    """NAME;PARAM=a,"b";X=y:value → ("NAME", {"PARAM": ["a", "b"], "X": ["y"]}, "value").
    A vCard group prefix ("item1.EMAIL") is dropped from the name and kept as
    params["@group"]."""
    i, n, quoted = 0, len(line), False
    while i < n:
        c = line[i]
        if c == '"':
            quoted = not quoted
        elif c == ":" and not quoted:
            break
        i += 1
    if i >= n:
        return None
    head, value = line[:i], line[i + 1 :]
    parts: list[str] = []
    cur, quoted = "", False
    for c in head:
        if c == '"':
            quoted = not quoted
            cur += c
        elif c == ";" and not quoted:
            parts.append(cur)
            cur = ""
        else:
            cur += c
    parts.append(cur)
    name = parts[0].strip().upper()
    params: dict[str, list[str]] = {}
    if "." in name:
        group, name = name.rsplit(".", 1)
        params["@group"] = [group]
    for p in parts[1:]:
        if "=" in p:
            k, v = p.split("=", 1)
            vals: list[str] = []
            cur, quoted = "", False
            for c in v:
                if c == '"':
                    quoted = not quoted
                elif c == "," and not quoted:
                    vals.append(cur)
                    cur = ""
                else:
                    cur += c
            vals.append(cur)
            params.setdefault(k.strip().upper(), []).extend(vals)
        elif p.strip():
            # vCard 2.1 bare types: TEL;HOME;VOICE:...
            params.setdefault("TYPE", []).append(p.strip())
    return name, params, value


class Component:
    __slots__ = ("name", "props", "children")

    def __init__(self, name: str):
        self.name = name
        self.props: list[tuple[str, dict[str, list[str]], str]] = []
        self.children: list[Component] = []

    def get(self, name: str):
        for p in self.props:
            if p[0] == name:
                return p
        return None

    def value(self, name: str, default: str = "") -> str:
        p = self.get(name)
        return p[2] if p else default

    def all(self, name: str):
        return [p for p in self.props if p[0] == name]

    def walk(self, name: str):
        for c in self.children:
            if c.name == name:
                yield c
            yield from c.walk(name)


def parse(text: str) -> list[Component]:
    """Top-level components (VCALENDAR, VCARD, ...) of a calendar or vCard stream."""
    roots: list[Component] = []
    stack: list[Component] = []
    for line in unfold(text):
        parsed = parse_line(line)
        if not parsed:
            continue
        name, params, value = parsed
        if name == "BEGIN":
            c = Component(value.strip().upper())
            if stack:
                stack[-1].children.append(c)
            else:
                roots.append(c)
            stack.append(c)
        elif name == "END":
            want = value.strip().upper()
            while stack:
                if stack.pop().name == want:
                    break
        elif stack:
            stack[-1].props.append((name, params, value))
    return roots


def text_value(s: str) -> str:
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            nxt = s[i + 1]
            out.append("\n" if nxt in "nN" else nxt)
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def text_escape(s: str) -> str:
    return s.replace("\\", "\\\\").replace(";", "\\;").replace(",", "\\,").replace("\r\n", "\n").replace("\n", "\\n")


def fold(line: str) -> str:
    """Folds a content line at 75 octets, never inside a UTF-8 sequence."""
    data = line.encode("utf-8")
    if len(data) <= 75:
        return line
    chunks, start, limit = [], 0, 75
    while start < len(data):
        end = min(start + limit, len(data))
        while end < len(data) and (data[end] & 0xC0) == 0x80:
            end -= 1
        chunks.append(data[start:end].decode("utf-8"))
        start, limit = end, 74
    return "\r\n ".join(chunks)


# ── time zones and date-times ────────────────────────────────────────────────────

WINDOWS_ZONES = {
    "central europe standard time": "Europe/Budapest",
    "central european standard time": "Europe/Warsaw",
    "w. europe standard time": "Europe/Berlin",
    "romance standard time": "Europe/Paris",
    "gmt standard time": "Europe/London",
    "greenwich standard time": "Atlantic/Reykjavik",
    "e. europe standard time": "Europe/Chisinau",
    "fle standard time": "Europe/Kiev",
    "gtb standard time": "Europe/Bucharest",
    "russian standard time": "Europe/Moscow",
    "eastern standard time": "America/New_York",
    "central standard time": "America/Chicago",
    "mountain standard time": "America/Denver",
    "pacific standard time": "America/Los_Angeles",
    "tokyo standard time": "Asia/Tokyo",
    "china standard time": "Asia/Shanghai",
    "india standard time": "Asia/Kolkata",
    "aus eastern standard time": "Australia/Sydney",
    "utc": "UTC",
}


class Zones:
    """TZID → tzinfo, from the IANA database, Windows names, the last path segments of
    vendor ids ("/mozilla.org/20050126_1/Europe/Budapest"), or a calendar's VTIMEZONE
    (its standard offset, as a last resort). Unknown ids are floating (local) time."""

    def __init__(self, vtimezones: list[Component] | None = None):
        self.cache: dict[str, dt.tzinfo | None] = {}
        self.vtz = {c.value("TZID"): c for c in vtimezones or []}

    def get(self, tzid: str | None) -> dt.tzinfo | None:
        if not tzid:
            return None
        if tzid in self.cache:
            return self.cache[tzid]
        zone = self._find(tzid.strip().strip('"'))
        self.cache[tzid] = zone
        return zone

    def _find(self, tzid: str) -> dt.tzinfo | None:
        candidates = [tzid]
        bits = [b for b in tzid.split("/") if b]
        if len(bits) >= 2:
            candidates += ["/".join(bits[-3:]), "/".join(bits[-2:])]
        if tzid.lower() in WINDOWS_ZONES:
            candidates.insert(0, WINDOWS_ZONES[tzid.lower()])
        for c in candidates:
            try:
                return ZoneInfo(c)
            except (ZoneInfoNotFoundError, ValueError, OSError):
                continue
        comp = self.vtz.get(tzid)
        if comp:
            for part in list(comp.walk("STANDARD")) + list(comp.walk("DAYLIGHT")):
                off = parse_offset(part.value("TZOFFSETTO"))
                if off is not None:
                    return dt.timezone(off)
        return None


def parse_offset(s: str) -> dt.timedelta | None:
    m = re.fullmatch(r"([+-])(\d\d)(\d\d)(\d\d)?", s.strip())
    if not m:
        return None
    sign = -1 if m.group(1) == "-" else 1
    return sign * dt.timedelta(hours=int(m.group(2)), minutes=int(m.group(3)), seconds=int(m.group(4) or 0))


def parse_dt(value: str, params: dict[str, list[str]], zones: Zones):
    """A DATE (dt.date) or DATE-TIME (aware dt.datetime; floating times get the local
    zone). None when unreadable."""
    v = value.strip()
    try:
        if params.get("VALUE", [""])[0].upper() == "DATE" or re.fullmatch(r"\d{8}", v):
            return dt.date(int(v[0:4]), int(v[4:6]), int(v[6:8]))
        m = re.fullmatch(r"(\d{4})(\d\d)(\d\d)T(\d\d)(\d\d)(\d\d)?(Z)?", v)
        if not m:
            return None
        naive = dt.datetime(*(int(m.group(i) or 0) for i in range(1, 7)))
        if m.group(7):
            return naive.replace(tzinfo=dt.timezone.utc)
        zone = zones.get(params.get("TZID", [None])[0])
        return local(naive, zone)
    except ValueError:
        return None


def local(naive: dt.datetime, zone: dt.tzinfo | None) -> dt.datetime:
    if zone is None:
        return naive.astimezone()  # floating: this machine's zone
    return naive.replace(tzinfo=zone)


def parse_duration(s: str) -> dt.timedelta | None:
    m = re.fullmatch(r"([+-])?P(?:(\d+)W)?(?:(\d+)D)?(?:T(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?)?", s.strip())
    if not m:
        return None
    w, d, h, mi, se = (int(x or 0) for x in m.groups()[1:])
    out = dt.timedelta(weeks=w, days=d, hours=h, minutes=mi, seconds=se)
    return -out if m.group(1) == "-" else out


# ── recurrence (RFC 5545 3.3.10) ─────────────────────────────────────────────────

WEEKDAYS = {"MO": 0, "TU": 1, "WE": 2, "TH": 3, "FR": 4, "SA": 5, "SU": 6}
MAX_INSTANCES = 5000


def parse_rrule(value: str) -> dict:
    rule: dict = {}
    for part in value.split(";"):
        if "=" not in part:
            continue
        k, v = part.split("=", 1)
        rule[k.strip().upper()] = v.strip()
    return rule


def _ints(s: str | None) -> list[int]:
    out = []
    for x in (s or "").split(","):
        x = x.strip()
        if re.fullmatch(r"[+-]?\d+", x):
            out.append(int(x))
    return out


def _bydays(s: str | None) -> list[tuple[int | None, int]]:
    out = []
    for x in (s or "").split(","):
        m = re.fullmatch(r"([+-]?\d+)?(MO|TU|WE|TH|FR|SA|SU)", x.strip().upper())
        if m:
            out.append((int(m.group(1)) if m.group(1) else None, WEEKDAYS[m.group(2)]))
    return out


def _month_days(year: int, month: int) -> int:
    nxt = dt.date(year + (month == 12), month % 12 + 1, 1)
    return (nxt - dt.date(year, month, 1)).days


def _days_by_weekday(days: list[dt.date], bydays) -> list[dt.date]:
    """Days of a period (in order) that match BYDAY, ordinals counted within it."""
    out = set()
    for ordinal, wd in bydays:
        matching = [d for d in days if d.weekday() == wd]
        if ordinal is None:
            out.update(matching)
        elif 0 < ordinal <= len(matching):
            out.add(matching[ordinal - 1])
        elif ordinal < 0 and -ordinal <= len(matching):
            out.add(matching[ordinal])
    return sorted(out)


def _month_candidates(year, month, rule, start_day) -> list[dt.date]:
    n = _month_days(year, month)
    days = [dt.date(year, month, d) for d in range(1, n + 1)]
    bymd = _ints(rule.get("BYMONTHDAY"))
    byday = _bydays(rule.get("BYDAY"))
    if bymd:
        want = {d if d > 0 else n + d + 1 for d in bymd}
        days = [d for d in days if d.day in want]
    if byday:
        if bymd:
            wds = {wd for _, wd in byday}
            days = [d for d in days if d.weekday() in wds]
        else:
            days = _days_by_weekday(days, byday)
    if not bymd and not byday:
        days = [dt.date(year, month, start_day)] if start_day <= n else []
    return days


def _period_candidates(freq, anchor: dt.date, rule, start: dt.date, wkst: int) -> list[dt.date]:
    bymonth = _ints(rule.get("BYMONTH"))
    byday = _bydays(rule.get("BYDAY"))
    bymd = _ints(rule.get("BYMONTHDAY"))
    if freq == "DAILY":
        days = [anchor]
        if bymd:
            n = _month_days(anchor.year, anchor.month)
            want = {d if d > 0 else n + d + 1 for d in bymd}
            days = [d for d in days if d.day in want]
        if byday:
            days = [d for d in days if d.weekday() in {wd for _, wd in byday}]
    elif freq == "WEEKLY":
        first = anchor - dt.timedelta(days=(anchor.weekday() - wkst) % 7)
        week = [first + dt.timedelta(days=i) for i in range(7)]
        wds = {wd for _, wd in byday} or {start.weekday()}
        days = [d for d in week if d.weekday() in wds]
    elif freq == "MONTHLY":
        days = _month_candidates(anchor.year, anchor.month, rule, start.day)
    elif freq == "YEARLY":
        year = anchor.year
        byyd = _ints(rule.get("BYYEARDAY"))
        if byyd:
            n = 366 if _month_days(year, 2) == 29 else 365
            days = sorted(dt.date(year, 1, 1) + dt.timedelta(days=(d if d > 0 else n + d + 1) - 1) for d in byyd if -n <= d <= n and d)
        elif byday and not bymonth and not bymd:
            # e.g. FREQ=YEARLY;BYDAY=20MO: the ordinal counts within the year
            first = dt.date(year, 1, 1)
            n = 366 if _month_days(year, 2) == 29 else 365
            days = _days_by_weekday([first + dt.timedelta(days=i) for i in range(n)], byday)
        else:
            days = []
            for m in bymonth or [start.month]:
                if 1 <= m <= 12:
                    if not byday and not bymd:
                        days += [dt.date(year, m, start.day)] if start.day <= _month_days(year, m) else []
                    else:
                        days += _month_candidates(year, m, rule, start.day)
    else:
        days = [anchor]
    if bymonth and freq != "YEARLY":
        days = [d for d in days if d.month in bymonth]
    setpos = _ints(rule.get("BYSETPOS"))
    if setpos and days:
        picked = set()
        for p in setpos:
            if 0 < p <= len(days):
                picked.add(days[p - 1])
            elif p < 0 and -p <= len(days):
                picked.add(days[p])
        days = sorted(picked)
    return days


def _next_anchor(freq, anchor: dt.date, interval: int) -> dt.date:
    if freq == "DAILY":
        return anchor + dt.timedelta(days=interval)
    if freq == "WEEKLY":
        return anchor + dt.timedelta(weeks=interval)
    if freq == "MONTHLY":
        m = anchor.month - 1 + interval
        return dt.date(anchor.year + m // 12, m % 12 + 1, 1)
    if freq == "YEARLY":
        return dt.date(anchor.year + interval, 1, 1)
    return anchor + dt.timedelta(days=interval)


def expand(start, rule: dict, until_limit, from_limit=None) -> list:
    """Instance starts of a recurring event (`start` is a date or an aware datetime) up
    to `until_limit`, and from `from_limit` on when given (same kind as start), DTSTART
    included. Wall-clock times stay put across daylight-saving changes, as RFC 5545
    asks."""
    freq = rule.get("FREQ", "").upper()
    interval = max(1, int(rule["INTERVAL"])) if rule.get("INTERVAL", "").isdigit() else 1
    count = int(rule["COUNT"]) if rule.get("COUNT", "").isdigit() else None
    wkst = WEEKDAYS.get(rule.get("WKST", "MO").upper(), 0)
    is_date = not isinstance(start, dt.datetime)
    until = None
    if rule.get("UNTIL"):
        u = parse_dt(rule["UNTIL"], {}, Zones())
        if u is not None:
            if is_date:
                until = u.date() if isinstance(u, dt.datetime) else u
            else:
                until = u if isinstance(u, dt.datetime) else dt.datetime.combine(u, dt.time(23, 59, 59), tzinfo=start.tzinfo)
    end = min(until_limit, until) if until is not None else until_limit
    out: list = []
    found = 0

    def keep(t) -> bool:
        """Counts an instance; False once the rule is done."""
        nonlocal found
        found += 1
        if from_limit is None or t >= from_limit:
            out.append(t)
        return not (count and found >= count) and len(out) < MAX_INSTANCES

    if freq in ("HOURLY", "MINUTELY", "SECONDLY") and not is_date:
        step = {"HOURLY": dt.timedelta(hours=interval), "MINUTELY": dt.timedelta(minutes=interval), "SECONDLY": dt.timedelta(seconds=max(interval, 60))}[freq]
        t = start
        if count is None and from_limit is not None and from_limit > start:
            t = start + step * max(0, (from_limit - start) // step - 1)
        while t <= end and keep(t):
            t = t + step
        return out
    if freq not in ("DAILY", "WEEKLY", "MONTHLY", "YEARLY"):
        return [start]

    d0 = start if is_date else start.date()
    wall = None if is_date else start.replace(tzinfo=None).time()
    tz = None if is_date else start.tzinfo

    def at(day: dt.date):
        if is_date:
            return day
        return dt.datetime.combine(day, wall).replace(tzinfo=tz)

    if start > end or not keep(start):
        return out
    anchor = d0
    if freq == "MONTHLY":
        anchor = dt.date(d0.year, d0.month, 1)
    elif freq == "YEARLY":
        anchor = dt.date(d0.year, 1, 1)
    # Without COUNT, nothing before the window matters: jump to just before it.
    if count is None and from_limit is not None:
        f = from_limit if is_date else from_limit.date()
        if freq == "DAILY":
            skip = max(0, (f - anchor).days // interval - 1)
            anchor += dt.timedelta(days=skip * interval)
        elif freq == "WEEKLY":
            skip = max(0, (f - anchor).days // (7 * interval) - 1)
            anchor += dt.timedelta(weeks=skip * interval)
        elif freq == "MONTHLY":
            months = (f.year - anchor.year) * 12 + f.month - anchor.month
            skip = max(0, months // interval - 1)
            anchor = _next_anchor(freq, anchor, skip * interval) if skip else anchor
        elif freq == "YEARLY":
            skip = max(0, (f.year - anchor.year) // interval - 1)
            anchor = dt.date(anchor.year + skip * interval, 1, 1)
    limit_day = end if is_date else end.date()
    for _ in range(100000):
        for day in _period_candidates(freq, anchor, rule, d0, wkst):
            if day <= d0:
                continue
            t = at(day)
            if t > end:
                return out
            if not keep(t):
                return out
        anchor = _next_anchor(freq, anchor, interval)
        if anchor > limit_day + dt.timedelta(days=7):
            break
    return out


# ── events ───────────────────────────────────────────────────────────────────────


def _as_instant(t):
    """A comparable key for an instance start, date or datetime."""
    if isinstance(t, dt.datetime):
        return ("t", t.astimezone(dt.timezone.utc).replace(tzinfo=None))
    return ("d", t)


def iso(t) -> str:
    if isinstance(t, dt.datetime):
        return t.isoformat(timespec="seconds")
    return t.isoformat() + "T00:00:00"


def _in_window(start, end, win_start: dt.datetime, win_end: dt.datetime) -> bool:
    s = start if isinstance(start, dt.datetime) else dt.datetime.combine(start, dt.time()).astimezone()
    e = end if isinstance(end, dt.datetime) else dt.datetime.combine(end, dt.time()).astimezone()
    if e <= s:
        e = s + dt.timedelta(seconds=1)
    return s < win_end and e > win_start


def events_from_ics(text: str, win_start: dt.datetime, win_end: dt.datetime) -> list[dict]:
    """Every VEVENT instance of a calendar object (or whole .ics file) that overlaps
    the window: { uid, title, start, end, allDay, location, notes, recurring }."""
    out: list[dict] = []
    for cal in parse(text):
        if cal.name != "VCALENDAR":
            continue
        zones = Zones(list(cal.walk("VTIMEZONE")))
        groups: dict[str, list[Component]] = {}
        for ev in cal.children:
            if ev.name == "VEVENT":
                groups.setdefault(ev.value("UID") or f"anon-{id(ev)}", []).append(ev)
        for uid, comps in groups.items():
            out += _expand_group(uid, comps, zones, win_start, win_end)
    return out


def _times(ev: Component, zones: Zones):
    p = ev.get("DTSTART")
    if not p:
        return None
    start = parse_dt(p[2], p[1], zones)
    if start is None:
        return None
    end = None
    pe = ev.get("DTEND")
    if pe:
        end = parse_dt(pe[2], pe[1], zones)
        if end is not None and isinstance(end, dt.datetime) != isinstance(start, dt.datetime):
            end = None
    if end is None:
        dur = parse_duration(ev.value("DURATION")) if ev.get("DURATION") else None
        if dur is not None:
            end = start + dur
        elif isinstance(start, dt.datetime):
            end = start
        else:
            end = start + dt.timedelta(days=1)
    return start, end


def _instance(uid, ev, start, end, recurring):
    all_day = not isinstance(start, dt.datetime)
    return {
        "uid": uid,
        "title": text_value(ev.value("SUMMARY")).strip() or "(no title)",
        "start": iso(start),
        "end": iso(end),
        "allDay": all_day,
        "location": text_value(ev.value("LOCATION")).strip(),
        "notes": text_value(ev.value("DESCRIPTION")).strip(),
        "recurring": recurring,
    }


def _expand_group(uid, comps, zones, win_start, win_end) -> list[dict]:
    master = next((c for c in comps if not c.get("RECURRENCE-ID")), None)
    overrides = [c for c in comps if c.get("RECURRENCE-ID")]
    out = []
    replaced = set()
    for ov in overrides:
        rid = ov.get("RECURRENCE-ID")
        r = parse_dt(rid[2], rid[1], zones)
        if r is not None:
            replaced.add(_as_instant(r))
            if isinstance(r, dt.datetime):
                replaced.add(("d", r.date()))  # all-day master with a dated override
        if ov.value("STATUS").upper() == "CANCELLED":
            continue
        times = _times(ov, zones)
        if times and _in_window(*times, win_start, win_end):
            out.append(_instance(uid, ov, *times, True))
    if master is None or master.value("STATUS").upper() == "CANCELLED":
        return out
    times = _times(master, zones)
    if not times:
        return out
    start, end = times
    duration = end - start
    rrule = master.get("RRULE")
    rdates = master.all("RDATE")
    if not rrule and not rdates:
        if _in_window(start, end, win_start, win_end):
            out.append(_instance(uid, master, start, end, False))
        return out
    if isinstance(start, dt.datetime):
        limit = win_end.astimezone(start.tzinfo)
    else:
        limit = win_end.date()
    if isinstance(start, dt.datetime):
        early = (win_start - duration).astimezone(start.tzinfo)
    else:
        early = (win_start - duration).date() - dt.timedelta(days=1)
    starts = expand(start, parse_rrule(rrule[2]), limit, early) if rrule else [start]
    for _, params, value in rdates:
        for v in value.split(","):
            if params.get("VALUE", [""])[0].upper() == "PERIOD" or "/" in v:
                v = v.split("/")[0]
            t = parse_dt(v, params, zones)
            if t is not None and isinstance(t, dt.datetime) == isinstance(start, dt.datetime):
                starts.append(t)
    excluded = set()
    for _, params, value in master.all("EXDATE"):
        for v in value.split(","):
            t = parse_dt(v, params, zones)
            if t is not None:
                excluded.add(_as_instant(t))
    seen = set()
    for s in sorted(starts, key=_as_instant):
        key = _as_instant(s)
        if key in seen or key in excluded or key in replaced:
            continue
        seen.add(key)
        e = s + duration
        if _in_window(s, e, win_start, win_end):
            out.append(_instance(uid, master, s, e, True))
    return out


def new_event(title, start, end, all_day, location="", notes="", uid=None) -> tuple[str, str]:
    """(uid, VCALENDAR text) of a new event. Timed events are written in UTC, which
    every server and client reads without a VTIMEZONE."""
    uid = uid or f"{uuid.uuid4()}@hecate"
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")

    def when(name, t):
        if all_day:
            return f"{name};VALUE=DATE:{t.strftime('%Y%m%d')}"
        return f"{name}:{t.astimezone(dt.timezone.utc).strftime('%Y%m%dT%H%M%SZ')}"

    lines = [
        "BEGIN:VCALENDAR",
        "VERSION:2.0",
        "PRODID:-//hecate//haiiro shell//EN",
        "CALSCALE:GREGORIAN",
        "BEGIN:VEVENT",
        f"UID:{uid}",
        f"DTSTAMP:{stamp}",
        f"CREATED:{stamp}",
        f"LAST-MODIFIED:{stamp}",
        when("DTSTART", start),
        when("DTEND", end),
        f"SUMMARY:{text_escape(title)}",
    ]
    if location:
        lines.append(f"LOCATION:{text_escape(location)}")
    if notes:
        lines.append(f"DESCRIPTION:{text_escape(notes)}")
    lines += ["END:VEVENT", "END:VCALENDAR"]
    return uid, "\r\n".join(fold(line) for line in lines) + "\r\n"


# ── vCards ───────────────────────────────────────────────────────────────────────

LABELS = {"cell": "mobile", "iphone": "mobile", "mobile": "mobile", "home": "home", "work": "work", "main": "main", "other": "other", "pager": "pager", "fax": "fax"}


def _label(params, labels: dict[str, str]) -> str:
    group = params.get("@group", [""])[0]
    if group and group in labels:
        return labels[group]
    for t in params.get("TYPE", []):
        for one in t.lower().split(","):
            if one in LABELS:
                return LABELS[one]
    return ""


def _ablabel(v: str) -> str:
    m = re.fullmatch(r"_\$!<(.+)>!\$_", v.strip())
    return (m.group(1) if m else v.strip()).lower()


def contact_from_vcard(card: Component) -> dict | None:
    kind = (card.value("KIND") or card.value("X-ADDRESSBOOKSERVER-KIND")).lower()
    if kind == "group":
        return None
    labels = {p[1]["@group"][0]: _ablabel(text_value(p[2])) for p in card.all("X-ABLABEL") if "@group" in p[1]}
    # N is last;first;middle;prefix;suffix, split on unescaped ; only
    n = [text_value(x) for x in re.split(r"(?<!\\);", card.value("N"))] if card.get("N") else []
    last = (n[0] if len(n) > 0 else "").strip()
    first = (n[1] if len(n) > 1 else "").strip()
    org = text_value(re.split(r"(?<!\\);", card.value("ORG"))[0]).strip() if card.get("ORG") else ""
    name = text_value(card.value("FN")).strip() or " ".join(x for x in (first, last) if x) or org
    if not name:
        return None
    emails = [{"type": _label(p[1], labels), "value": text_value(p[2]).strip()} for p in card.all("EMAIL") if p[2].strip()]
    phones = [{"type": _label(p[1], labels), "value": text_value(p[2]).strip()} for p in card.all("TEL") if p[2].strip()]
    bday = card.value("BDAY").strip()
    m = re.fullmatch(r"(\d{4}|--)-?(\d\d)-?(\d\d).*", bday)
    birthday = ""
    if m:
        year = m.group(1)
        # Apple writes 1604 for "no year"
        birthday = f"--{m.group(2)}-{m.group(3)}" if year in ("--", "1604") else f"{year}-{m.group(2)}-{m.group(3)}"
    return {
        "uid": card.value("UID").strip(),
        "name": name,
        "first": first,
        "last": last,
        "nickname": text_value(card.value("NICKNAME")).strip(),
        "org": org,
        "title": text_value(card.value("TITLE")).strip(),
        "emails": emails,
        "phones": phones,
        "birthday": birthday,
        "note": text_value(card.value("NOTE")).strip(),
    }


def contacts_from_vcf(text: str) -> list[dict]:
    out = []
    for c in parse(text):
        if c.name == "VCARD":
            contact = contact_from_vcard(c)
            if contact:
                out.append(contact)
    return out
