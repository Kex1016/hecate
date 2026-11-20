#!/usr/bin/env python3
"""The calendar and contacts sync behind services/apple/Calendars.qml and Contacts.qml.

    dav.py sync   [--force]  [paths]   sync every account, write events.json and contacts.json
    dav.py create            [paths]   add the event on stdin ({ title, start, end, allDay,
                                       location, notes, target }), then sync
    dav.py delete            [paths]   remove the event on stdin ({ uid, url, etag, accountId,
                                       href }), then sync

paths: --config calendars.toml --state <calendars state dir> --local <local.ics>
       --contacts <contacts.json>

Accounts come from calendars.toml (written by the dashboard's calendars page): iCloud and
other CalDAV servers, and read-only .ics subscriptions. A CalDAV account's password is read
from the keyring with `secret-tool lookup application hecate service calendars account <id>`
and kept in memory only: never written, printed or passed on a command line.

Prints one JSON line per account ({ account, state: ok|error, message, synced }) and, for
create/delete, one { ok, error, event } line. Standard library only.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import fcntl
import hashlib
import json
import os
import re
import subprocess
import sys
import tomllib
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

sys.dont_write_bytecode = True  # the shell may run from a read-only store
sys.path.insert(0, str(Path(__file__).resolve().parent))
import ical  # noqa: E402

DAV = "{DAV:}"
CAL = "{urn:ietf:params:xml:ns:caldav}"
CARD = "{urn:ietf:params:xml:ns:carddav}"
CS = "{http://calendarserver.org/ns/}"
APPLE = "{http://apple.com/ns/ical/}"

ICLOUD_CONTACTS = "https://contacts.icloud.com/"
USER_AGENT = "hecate-calendars/1 (CalDAV/CardDAV)"
TIMEOUT = 30
# How far the synced events reach: the dashboard and quick panel browse months.
BACK_DAYS = 100
AHEAD_DAYS = 400
# A cached calendar (same ctag) is reused this long before fetching it again anyway.
CACHE_DAYS = 7
CARDDAV_RECHECK = dt.timedelta(hours=24)


def now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc)


def stamp() -> str:
    return now().isoformat(timespec="seconds").replace("+00:00", "Z")


def emit(obj: dict) -> None:
    print(json.dumps(obj, ensure_ascii=False), flush=True)


def write_json(path: Path, data) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f".{path.name}.tmp")
    tmp.write_text(json.dumps(data, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def read_json(path: Path, default):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return default


class Failure(Exception):
    """An account-level problem, worded for the calendars page."""


# ── keyring ──────────────────────────────────────────────────────────────────────


def secret(account_id: str) -> str:
    try:
        r = subprocess.run(
            ["secret-tool", "lookup", "application", "hecate", "service", "calendars", "account", account_id],
            stdin=subprocess.DEVNULL, capture_output=True, timeout=30,
        )
    except FileNotFoundError:
        raise Failure("secret-tool is missing (libsecret), so the password cannot be read from the keyring.")
    except subprocess.TimeoutExpired:
        raise Failure("The keyring did not answer. Is it locked?")
    if r.returncode != 0 or not r.stdout:
        raise Failure("No password in the keyring. Edit the account to sign in again.")
    return r.stdout.decode("utf-8", "replace")


# ── HTTP ─────────────────────────────────────────────────────────────────────────


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


def same_site(a: str, b: str) -> bool:
    """Whether credentials may follow a redirect from a to b (same registrable-ish domain:
    caldav.icloud.com → p42-caldav.icloud.com)."""
    ha, hb = urllib.parse.urlsplit(a).hostname or "", urllib.parse.urlsplit(b).hostname or ""
    return ha == hb or ".".join(ha.split(".")[-2:]) == ".".join(hb.split(".")[-2:])


class Response:
    def __init__(self, status: int, body: bytes, headers, url: str):
        self.status, self.body, self.headers, self.url = status, body, headers, url

    def text(self) -> str:
        return self.body.decode("utf-8", "replace")


def request(method: str, url: str, auth: tuple[str, str] | None = None, body: str | None = None,
            headers: dict | None = None) -> Response:
    hdrs = {"User-Agent": USER_AGENT, **(headers or {})}
    data = body.encode("utf-8") if body is not None else None
    for _ in range(6):
        h = dict(hdrs)
        if auth:
            token = base64.b64encode(f"{auth[0]}:{auth[1]}".encode("utf-8")).decode("ascii")
            h["Authorization"] = f"Basic {token}"
        req = urllib.request.Request(url, data=data, method=method, headers=h)
        try:
            with OPENER.open(req, timeout=TIMEOUT) as r:
                return Response(r.status, r.read(), r.headers, url)
        except urllib.error.HTTPError as e:
            if e.code in (301, 302, 303, 307, 308) and e.headers.get("Location"):
                nxt = urllib.parse.urljoin(url, e.headers["Location"])
                if auth and not same_site(url, nxt):
                    auth = None
                if e.code == 303:
                    method, data = "GET", None
                url = nxt
                continue
            return Response(e.code, e.read() or b"", e.headers, url)
        except (urllib.error.URLError, OSError, ValueError) as e:
            raise Failure("Could not reach the server.") from e
    raise Failure("The server sent the request in circles (too many redirects).")


def http_failure(r: Response, what: str) -> Failure:
    if r.status in (401, 403):
        return Failure("The server refused the user name or password. Edit the account to sign in again.")
    if r.status == 404:
        return Failure(f"The server no longer has {what}.")
    return Failure(f"The server answered {r.status} for {what}.")


def dav(method: str, url: str, auth, body: str, depth: str | None = None) -> Response:
    headers = {"Content-Type": "application/xml; charset=utf-8"}
    if depth is not None:
        headers["Depth"] = depth
    return request(method, url, auth, body, headers)


def multistatus(r: Response) -> list[tuple[str, dict[str, ET.Element]]]:
    """[(href, {"{ns}prop": element})] of the 200 propstats of a 207 answer."""
    try:
        root = ET.fromstring(r.body)
    except ET.ParseError:
        return []
    out = []
    for resp in root.iter(f"{DAV}response"):
        href = (resp.findtext(f"{DAV}href") or "").strip()
        props: dict[str, ET.Element] = {}
        for ps in resp.iter(f"{DAV}propstat"):
            status = ps.findtext(f"{DAV}status") or ""
            if " 200" not in status:
                continue
            prop = ps.find(f"{DAV}prop")
            if prop is not None:
                for p in prop:
                    props[p.tag] = p
        out.append((urllib.parse.urljoin(r.url, href), props))
    return out


def prop_text(props: dict, tag: str) -> str:
    e = props.get(tag)
    return (e.text or "").strip() if e is not None else ""


def prop_href(props: dict, tag: str, base: str) -> str:
    e = props.get(tag)
    if e is None:
        return ""
    h = e.findtext(f"{DAV}href")
    return urllib.parse.urljoin(base, h.strip()) if h else ""


def same_url(a: str, b: str) -> bool:
    norm = lambda u: urllib.parse.unquote(u).rstrip("/")
    return norm(a) == norm(b)


# ── CalDAV ───────────────────────────────────────────────────────────────────────

CTAG_BODY = ('<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:cs="http://calendarserver.org/ns/">'
             '<d:prop><cs:getctag/><d:sync-token/></d:prop></d:propfind>')


def calendar_query(start: dt.datetime, end: dt.datetime) -> str:
    f = lambda t: t.astimezone(dt.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return ('<?xml version="1.0" encoding="utf-8"?>'
            '<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav">'
            '<d:prop><d:getetag/><c:calendar-data/></d:prop>'
            '<c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT">'
            f'<c:time-range start="{f(start)}" end="{f(end)}"/>'
            '</c:comp-filter></c:comp-filter></c:filter></c:calendar-query>')


def multiget(kind: str, hrefs: list[str]) -> str:
    ns, data = ("c", "calendar-data") if kind == "cal" else ("card", "address-data")
    uri = "urn:ietf:params:xml:ns:caldav" if kind == "cal" else "urn:ietf:params:xml:ns:carddav"
    tag = "calendar-multiget" if kind == "cal" else "addressbook-multiget"
    refs = "".join(f"<d:href>{escape_xml(urllib.parse.urlsplit(h).path)}</d:href>" for h in hrefs)
    return (f'<?xml version="1.0" encoding="utf-8"?><{ns}:{tag} xmlns:d="DAV:" xmlns:{ns}="{uri}">'
            f'<d:prop><d:getetag/><{ns}:{data}/></d:prop>{refs}</{ns}:{tag}>')


def escape_xml(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def ctag(url: str, auth) -> str:
    r = dav("PROPFIND", url, auth, CTAG_BODY, "0")
    if r.status != 207:
        raise http_failure(r, "that calendar")
    for _, props in multistatus(r):
        tag = prop_text(props, f"{CS}getctag") or prop_text(props, f"{DAV}sync-token")
        if tag:
            return tag
    return ""


def fetch_calendar(url: str, auth, start: dt.datetime, end: dt.datetime) -> list[dict]:
    """[{ url, etag, ics }] of the calendar's events in [start, end)."""
    r = dav("REPORT", url, auth, calendar_query(start, end), "1")
    if r.status != 207:
        raise http_failure(r, "that calendar")
    objects, missing = [], []
    for href, props in multistatus(r):
        if same_url(href, url):
            continue
        data = prop_text(props, f"{CAL}calendar-data")
        etag = prop_text(props, f"{DAV}getetag")
        if data:
            objects.append({"url": href, "etag": etag, "ics": data})
        else:
            missing.append(href)
    # Some servers list only hrefs in a calendar-query: fetch those in batches.
    for i in range(0, len(missing), 50):
        r2 = dav("REPORT", url, auth, multiget("cal", missing[i:i + 50]), "1")
        if r2.status != 207:
            raise http_failure(r2, "that calendar's events")
        for href, props in multistatus(r2):
            data = prop_text(props, f"{CAL}calendar-data")
            if data:
                objects.append({"url": href, "etag": prop_text(props, f"{DAV}getetag"), "ics": data})
    return objects


# ── CardDAV ──────────────────────────────────────────────────────────────────────

PRINCIPAL_BODY = ('<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:">'
                  '<d:prop><d:current-user-principal/></d:prop></d:propfind>')
CARD_HOME_BODY = ('<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">'
                  '<d:prop><card:addressbook-home-set/></d:prop></d:propfind>')
BOOKS_BODY = ('<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:" xmlns:cs="http://calendarserver.org/ns/">'
              '<d:prop><d:resourcetype/><d:displayname/><cs:getctag/><d:sync-token/></d:prop></d:propfind>')
ETAGS_BODY = ('<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:">'
              '<d:prop><d:getetag/><d:resourcetype/></d:prop></d:propfind>')


def discover_books(account: dict, auth) -> list[dict]:
    """The account's address books, [{ href, name }]; [] when its server has none."""
    if account["kind"] == "icloud":
        candidates = [ICLOUD_CONTACTS]
    else:
        parts = urllib.parse.urlsplit(account.get("url", ""))
        root = f"{parts.scheme}://{parts.netloc}/"
        candidates = [account.get("url", ""), urllib.parse.urljoin(root, "/.well-known/carddav")]
    reached = False
    for start in [c for c in candidates if c]:
        try:
            r = dav("PROPFIND", start, auth, PRINCIPAL_BODY, "0")
        except Failure:
            continue
        reached = True
        if r.status in (401, 403) and account["kind"] == "icloud":
            raise http_failure(r, "your contacts")
        if r.status != 207:
            continue
        principal = next((prop_href(p, f"{DAV}current-user-principal", r.url) for _, p in multistatus(r)
                          if prop_href(p, f"{DAV}current-user-principal", r.url)), "") or r.url
        r2 = dav("PROPFIND", principal, auth, CARD_HOME_BODY, "0")
        if r2.status != 207:
            continue
        home = next((prop_href(p, f"{CARD}addressbook-home-set", r2.url) for _, p in multistatus(r2)
                     if prop_href(p, f"{CARD}addressbook-home-set", r2.url)), "")
        if not home:
            continue
        r3 = dav("PROPFIND", home, auth, BOOKS_BODY, "1")
        if r3.status != 207:
            continue
        books = []
        for href, props in multistatus(r3):
            rt = props.get(f"{DAV}resourcetype")
            if rt is not None and rt.find(f"{CARD}addressbook") is not None:
                name = prop_text(props, f"{DAV}displayname") or urllib.parse.unquote(href.rstrip("/").split("/")[-1])
                books.append({"href": href, "name": name})
        if books:
            return books
    if not reached:
        # offline: say so, so the cached address books are kept
        raise Failure("Could not reach the server.")
    return []


def fetch_book(book: str, auth, cached: dict[str, dict]) -> dict[str, dict]:
    """{ url: { etag, vcf } } of an address book, reusing cached cards whose etag is the same."""
    r = dav("PROPFIND", book, auth, ETAGS_BODY, "1")
    if r.status != 207:
        raise http_failure(r, "that address book")
    listed = {}
    for href, props in multistatus(r):
        rt = props.get(f"{DAV}resourcetype")
        if same_url(href, book) or (rt is not None and len(rt)):
            continue
        listed[href] = prop_text(props, f"{DAV}getetag")
    out = {h: cached[h] for h, e in listed.items() if h in cached and e and cached[h].get("etag") == e}
    todo = [h for h in listed if h not in out]
    for i in range(0, len(todo), 100):
        r2 = dav("REPORT", book, auth, multiget("card", todo[i:i + 100]), "1")
        if r2.status != 207:
            raise http_failure(r2, "that address book's cards")
        for href, props in multistatus(r2):
            data = prop_text(props, f"{CARD}address-data")
            if data:
                out[href] = {"etag": prop_text(props, f"{DAV}getetag"), "vcf": data}
    return out


# ── the sync ─────────────────────────────────────────────────────────────────────


class Sync:
    def __init__(self, args):
        self.config = Path(args.config).expanduser()
        self.state = Path(args.state).expanduser()
        self.local = Path(args.local).expanduser()
        self.contacts_path = Path(args.contacts).expanduser()
        self.cache = self.state / "cache"
        self.force = getattr(args, "force", False)
        self.accounts = self.read_accounts()
        today = now().replace(hour=0, minute=0, second=0, microsecond=0)
        self.win_start = today - dt.timedelta(days=BACK_DAYS)
        self.win_end = today + dt.timedelta(days=AHEAD_DAYS)
        self.passwords: dict[str, str] = {}

    def read_accounts(self) -> list[dict]:
        try:
            with open(self.config, "rb") as f:
                data = tomllib.load(f)
        except FileNotFoundError:
            return []
        except (OSError, tomllib.TOMLDecodeError) as e:
            print(f"dav.py: cannot read {self.config}: {e}", file=sys.stderr)
            return []
        out = []
        for a in data.get("account", []):
            if isinstance(a, dict) and a.get("id") and a.get("kind") in ("icloud", "caldav", "ics"):
                a.setdefault("name", a["id"])
                a["calendars"] = [c for c in a.get("calendar", []) if isinstance(c, dict) and c.get("href")]
                out.append(a)
        return out

    def auth(self, account: dict) -> tuple[str, str]:
        if account["id"] not in self.passwords:
            self.passwords[account["id"]] = secret(account["id"])
        return account.get("user", ""), self.passwords[account["id"]]

    def cache_file(self, key: str) -> Path:
        return self.cache / (hashlib.sha1(key.encode("utf-8")).hexdigest()[:20] + ".json")

    # calendars

    def caldav_objects(self, account: dict, cal: dict) -> list[dict]:
        path = self.cache_file(f"{account['id']}|{cal['href']}")
        cached = read_json(path, {})
        auth = self.auth(account)
        tag = ctag(cal["href"], auth)
        fresh = (cached.get("fetched", "") > (now() - dt.timedelta(days=CACHE_DAYS)).isoformat())
        if tag and cached.get("ctag") == tag and fresh and not self.force:
            return cached.get("objects", [])
        objects = fetch_calendar(cal["href"], auth, self.win_start - dt.timedelta(days=CACHE_DAYS),
                                 self.win_end + dt.timedelta(days=CACHE_DAYS))
        write_json(path, {"ctag": tag, "fetched": now().isoformat(), "objects": objects})
        return objects

    def ics_objects(self, account: dict, cal: dict) -> list[dict]:
        path = self.cache_file(f"{account['id']}|{cal['href']}")
        cached = read_json(path, {})
        refresh = dt.timedelta(minutes=max(15, int(account.get("refresh") or 1440)))
        if cached.get("ics") and not self.force and cached.get("fetched", "") > (now() - refresh).isoformat():
            return [{"url": "", "etag": "", "ics": cached["ics"]}]
        headers = {}
        if cached.get("ics") and cached.get("etag"):
            headers["If-None-Match"] = cached["etag"]
        if cached.get("ics") and cached.get("modified"):
            headers["If-Modified-Since"] = cached["modified"]
        url = re.sub(r"^webcal://", "https://", cal["href"], flags=re.I)
        r = request("GET", url, headers=headers)
        if r.status == 304:
            cached["fetched"] = now().isoformat()
            write_json(path, cached)
            return [{"url": "", "etag": "", "ics": cached["ics"]}]
        if r.status != 200 or "BEGIN:VCALENDAR" not in r.text():
            raise http_failure(r, "that calendar") if r.status != 200 else Failure("That address no longer answers with a calendar.")
        write_json(path, {"fetched": now().isoformat(), "etag": r.headers.get("ETag", ""),
                          "modified": r.headers.get("Last-Modified", ""), "ics": r.text()})
        return [{"url": "", "etag": "", "ics": r.text()}]

    def cached_objects(self, account: dict, cal: dict) -> list[dict]:
        cached = read_json(self.cache_file(f"{account['id']}|{cal['href']}"), {})
        if "objects" in cached:
            return cached["objects"]
        return [{"url": "", "etag": "", "ics": cached["ics"]}] if cached.get("ics") else []

    def instances(self, objects, account, cal, writable) -> list[dict]:
        out = []
        for o in objects:
            try:
                found = ical.events_from_ics(o["ics"], self.win_start, self.win_end)
            except Exception as e:  # one broken event must not cost the calendar
                print(f"dav.py: skipped an unreadable event in {cal.get('name')}: {type(e).__name__}", file=sys.stderr)
                continue
            for ev in found:
                ev.update({
                    "account": account["name"], "accountId": account["id"],
                    "calendar": cal.get("name", ""), "href": cal["href"],
                    "url": o.get("url", ""), "etag": o.get("etag", ""),
                    "color": cal.get("color", ""), "writable": writable,
                })
                out.append(ev)
        return out

    def local_events(self) -> list[dict]:
        try:
            text = self.local.read_text(encoding="utf-8")
        except OSError:
            return []
        cal = {"name": "local", "href": "local", "color": ""}
        acc = {"name": "local", "id": "local"}
        return self.instances([{"url": "", "etag": "", "ics": text}], acc, cal, True)

    def sync_calendars(self) -> None:
        events = self.local_events()
        for account in self.accounts:
            shown = [c for c in account["calendars"] if c.get("shown", True)
                     and "VEVENT" in (c.get("components") or ["VEVENT"])]
            error = ""
            for cal in shown:
                writable = account["kind"] != "ics" and bool(cal.get("writable", True))
                try:
                    if account["kind"] == "ics":
                        objects = self.ics_objects(account, cal)
                    else:
                        objects = self.caldav_objects(account, cal)
                except Failure as e:
                    error = error or str(e)
                    objects = self.cached_objects(account, cal)  # keep the last good copy
                events += self.instances(objects, account, cal, writable)
                if error and "password" in error:
                    # the rest of this account will be refused too
                    for rest in shown[shown.index(cal) + 1:]:
                        events += self.instances(self.cached_objects(account, rest), account, rest,
                                                 account["kind"] != "ics" and bool(rest.get("writable", True)))
                    break
            if error:
                emit({"account": account["id"], "state": "error", "message": error})
            else:
                emit({"account": account["id"], "state": "ok", "message": "", "synced": stamp()})
        # all-day starts have no offset: fromisoformat reads them as local midnight
        events.sort(key=lambda e: (dt.datetime.fromisoformat(e["start"]).timestamp(), e["title"]))
        write_json(self.state / "events.json", events)
        self.prune_cache()

    def prune_cache(self) -> None:
        keep = {self.cache_file(f"{a['id']}|{c['href']}").name for a in self.accounts for c in a["calendars"]}
        keep |= {self.cache_file(f"cards|{a['id']}").name for a in self.accounts}
        if not self.cache.is_dir():
            return
        for f in self.cache.glob("*.json"):
            if f.name not in keep:
                f.unlink(missing_ok=True)

    # contacts

    def sync_contacts(self) -> None:
        contacts = []
        for account in self.accounts:
            if account["kind"] == "ics":
                continue
            path = self.cache_file(f"cards|{account['id']}")
            cached = read_json(path, {})
            books = cached.get("books")
            cards = cached.get("cards", {})
            try:
                auth = self.auth(account)
                checked = cached.get("checked", "")
                if books is None or self.force or checked < (now() - CARDDAV_RECHECK).isoformat():
                    books = discover_books(account, auth)
                    checked = now().isoformat()
                new_cards = {}
                for b in books:
                    new_cards[b["href"]] = fetch_book(b["href"], auth, cards.get(b["href"], {}))
                cards = new_cards
                write_json(path, {"checked": checked, "books": books, "cards": cards})
            except Failure as e:
                print(f"dav.py: contacts of {account['name']}: {e}", file=sys.stderr)
                books = books or []
            for b in books or []:
                for item in cards.get(b["href"], {}).values():
                    for c in ical.contacts_from_vcf(item.get("vcf", "")):
                        c.update({"account": account["name"], "accountId": account["id"], "addressbook": b["name"]})
                        contacts.append(c)
        contacts.sort(key=lambda c: c["name"].casefold())
        write_json(self.contacts_path, contacts)

    # changes

    def target(self, name: str) -> tuple[dict | None, dict | None]:
        if not name or name == "local":
            return None, None
        for a in self.accounts:
            for c in a["calendars"]:
                if f"{a['name']}: {c.get('name', '')}" == name:
                    return a, c
        raise Failure(f'There is no calendar "{name}". Pick another under calendars.')

    def create(self, req: dict) -> dict:
        all_day = bool(req.get("allDay"))
        try:
            if all_day:
                start = dt.date.fromisoformat(str(req["start"])[:10])
                end = dt.date.fromisoformat(str(req.get("end") or "")[:10]) if req.get("end") else start + dt.timedelta(days=1)
                if end <= start:
                    end = start + dt.timedelta(days=1)
            else:
                start = dt.datetime.fromisoformat(str(req["start"]).replace("Z", "+00:00"))
                end = dt.datetime.fromisoformat(str(req["end"]).replace("Z", "+00:00")) if req.get("end") else start + dt.timedelta(hours=1)
                if start.tzinfo is None:
                    start = start.astimezone()
                if end.tzinfo is None:
                    end = end.astimezone()
                if end <= start:
                    end = start + dt.timedelta(hours=1)
        except (KeyError, ValueError):
            raise Failure("The event's start or end is not a date.")
        title = str(req.get("title") or "event").strip()
        uid, text = ical.new_event(title, start, end, all_day, str(req.get("location") or ""), str(req.get("notes") or ""))
        account, cal = self.target(str(req.get("target") or "local"))
        if account is None:
            self.local_add(text)
            url, etag, acc_name, acc_id, cal_name, href = "", "", "local", "local", "local", "local"
        else:
            if account["kind"] == "ics" or not cal.get("writable", True):
                raise Failure(f"{account['name']}: {cal.get('name')} is read-only.")
            url = cal["href"].rstrip("/") + "/" + urllib.parse.quote(uid.replace("@", "-")) + ".ics"
            r = request("PUT", url, self.auth(account), text,
                        {"Content-Type": "text/calendar; charset=utf-8", "If-None-Match": "*"})
            if r.status not in (200, 201, 204):
                raise http_failure(r, "the new event")
            etag = r.headers.get("ETag", "")
            acc_name, acc_id, cal_name, href = account["name"], account["id"], cal.get("name", ""), cal["href"]
            self.cache_file(f"{account['id']}|{cal['href']}").unlink(missing_ok=True)
        ev = ical.events_from_ics(text, dt.datetime(1900, 1, 1, tzinfo=dt.timezone.utc), dt.datetime(9000, 1, 1, tzinfo=dt.timezone.utc))[0]
        ev.update({"account": acc_name, "accountId": acc_id, "calendar": cal_name, "href": href,
                   "url": url, "etag": etag, "color": (cal or {}).get("color", ""), "writable": True})
        return ev

    def delete(self, req: dict) -> None:
        uid = str(req.get("uid") or "")
        if req.get("accountId") == "local" or (not req.get("url") and req.get("href") == "local"):
            if not self.local_remove(uid):
                raise Failure("That event is not in the local calendar any more.")
            return
        account = next((a for a in self.accounts if a["id"] == req.get("accountId")), None)
        if account is None or account["kind"] == "ics" or not req.get("url"):
            raise Failure("That calendar is read-only.")
        headers = {"If-Match": req["etag"]} if req.get("etag") else {}
        r = request("DELETE", str(req["url"]), self.auth(account), None, headers)
        if r.status == 412:
            raise Failure("The event changed on the server. Sync and try again.")
        if r.status not in (200, 204, 404):
            raise http_failure(r, "that event")
        self.cache_file(f"{account['id']}|{req.get('href', '')}").unlink(missing_ok=True)

    def local_add(self, event_text: str) -> None:
        body = re.search(r"BEGIN:VEVENT.*END:VEVENT\r?\n", event_text, re.S).group(0)
        try:
            text = self.local.read_text(encoding="utf-8")
        except FileNotFoundError:
            text = "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//hecate//haiiro shell//EN\r\nX-WR-CALNAME:local\r\nEND:VCALENDAR\r\n"
        i = text.rfind("END:VCALENDAR")
        if i < 0:
            raise Failure(f"{self.local} is not a calendar file.")
        self.write_local(text[:i] + body + text[i:])

    def local_remove(self, uid: str) -> bool:
        try:
            text = self.local.read_text(encoding="utf-8")
        except FileNotFoundError:
            return False
        kept, removed, block = [], False, None
        for line in text.replace("\r\n", "\n").split("\n"):
            if line.strip() == "BEGIN:VEVENT":
                block = [line]
                continue
            if block is not None:
                block.append(line)
                if line.strip() == "END:VEVENT":
                    unfolded = ical.unfold("\n".join(block))
                    if any(re.fullmatch(r"UID(;[^:]*)?:" + re.escape(uid), b.strip()) for b in unfolded):
                        removed = True
                    else:
                        kept += block
                    block = None
                continue
            kept.append(line)
        if removed:
            self.write_local("\r\n".join(kept).rstrip("\r\n") + "\r\n")
        return removed

    def write_local(self, text: str) -> None:
        self.local.parent.mkdir(parents=True, exist_ok=True)
        tmp = self.local.with_name(f".{self.local.name}.tmp")
        tmp.write_text(text, encoding="utf-8")
        os.replace(tmp, self.local)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["sync", "create", "delete"])
    ap.add_argument("--force", action="store_true", help="fetch everything again, ignoring the cache")
    ap.add_argument("--no-contacts", action="store_true")
    home = Path.home()
    ap.add_argument("--config", default=str(home / ".config/hecate/calendars.toml"))
    ap.add_argument("--state", default=str(home / ".local/state/quickshell/user/calendars"))
    ap.add_argument("--local", default=str(home / ".local/share/hecate/calendars/local.ics"))
    ap.add_argument("--contacts", default=str(home / ".local/state/quickshell/user/contacts/contacts.json"))
    args = ap.parse_args()

    state = Path(args.state).expanduser()
    state.mkdir(parents=True, exist_ok=True)
    # One run at a time: a create while the timer's sync runs waits for it.
    with open(state / ".lock", "w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        s = Sync(args)
        if args.command in ("create", "delete"):
            try:
                req = json.loads(sys.stdin.readline() or "{}")
                if args.command == "create":
                    emit({"ok": True, "error": "", "event": s.create(req)})
                else:
                    s.delete(req)
                    emit({"ok": True, "error": ""})
            except Failure as e:
                emit({"ok": False, "error": str(e)})
                return 0
            except (ValueError, TypeError) as e:
                emit({"ok": False, "error": f"That event could not be read ({type(e).__name__})."})
                return 0
        s.sync_calendars()
        if args.command == "sync" and not args.no_contacts:
            s.sync_contacts()
    return 0


if __name__ == "__main__":
    sys.exit(main())
