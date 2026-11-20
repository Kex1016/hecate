#!/usr/bin/env python3
"""Checks for ical.py: python3 scripts/apple/test_ical.py"""

import datetime as dt
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True  # the shell may run from a read-only store
sys.path.insert(0, str(Path(__file__).resolve().parent))
import ical  # noqa: E402

Z = dt.timezone.utc
WIN = (dt.datetime(2026, 10, 1, tzinfo=Z), dt.datetime(2027, 2, 1, tzinfo=Z))


def starts(body: str) -> list[str]:
    text = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\n" + body.strip().replace("\n", "\r\n") + "\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n"
    return sorted(e["start"] for e in ical.events_from_ics(text, *WIN))


class Recurrence(unittest.TestCase):
    def test_weekly_keeps_wall_time_across_dst(self):
        self.assertEqual(starts("UID:a\nDTSTART;TZID=Europe/Budapest:20261020T090000\nRRULE:FREQ=WEEKLY;UNTIL=20261105T000000Z"),
                         ["2026-10-20T09:00:00+02:00", "2026-10-27T09:00:00+01:00", "2026-11-03T09:00:00+01:00"])

    def test_last_friday(self):
        self.assertEqual(starts("UID:a\nDTSTART:20260925T170000Z\nRRULE:FREQ=MONTHLY;BYDAY=-1FR;COUNT=4"),
                         ["2026-10-30T17:00:00+00:00", "2026-11-27T17:00:00+00:00", "2026-12-25T17:00:00+00:00"])

    def test_last_weekday_bysetpos(self):
        self.assertEqual(starts("UID:a\nDTSTART:20261030T080000Z\nRRULE:FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;COUNT=3"),
                         ["2026-10-30T08:00:00+00:00", "2026-11-30T08:00:00+00:00", "2026-12-31T08:00:00+00:00"])

    def test_monthly_31st_skips_short_months(self):
        self.assertEqual(starts("UID:a\nDTSTART:20260831T100000Z\nRRULE:FREQ=MONTHLY;COUNT=5"),
                         ["2026-10-31T10:00:00+00:00", "2026-12-31T10:00:00+00:00", "2027-01-31T10:00:00+00:00"])

    def test_exdate_and_moved_instance(self):
        self.assertEqual(starts("UID:a\nDTSTART:20261101T100000Z\nRRULE:FREQ=DAILY;COUNT=4\nEXDATE:20261102T100000Z\nEND:VEVENT\n"
                                "BEGIN:VEVENT\nUID:a\nRECURRENCE-ID:20261103T100000Z\nDTSTART:20261103T150000Z"),
                         ["2026-11-01T10:00:00+00:00", "2026-11-03T15:00:00+00:00", "2026-11-04T10:00:00+00:00"])

    def test_all_day_biweekly(self):
        self.assertEqual(starts("UID:a\nDTSTART;VALUE=DATE:20261102\nRRULE:FREQ=WEEKLY;INTERVAL=2;BYDAY=MO,FR;UNTIL=20261130"),
                         ["2026-11-02T00:00:00", "2026-11-06T00:00:00", "2026-11-16T00:00:00", "2026-11-20T00:00:00", "2026-11-30T00:00:00"])

    def test_old_unbounded_series_reaches_the_window(self):
        self.assertEqual(len(starts("UID:a\nDTSTART:20100101T080000Z\nRRULE:FREQ=DAILY")), 123)

    def test_yearly_nth_weekday(self):
        self.assertEqual(starts("UID:a\nDTSTART;VALUE=DATE:20201126\nRRULE:FREQ=YEARLY;BYMONTH=11;BYDAY=4TH"), ["2026-11-26T00:00:00"])


class Text(unittest.TestCase):
    def test_new_event_round_trip(self):
        uid, text = ical.new_event("lunch, with Anna; " + "x" * 80, dt.datetime(2026, 11, 24, 11, tzinfo=Z),
                                   dt.datetime(2026, 11, 24, 12, tzinfo=Z), False, "Bartók", "a\nb")
        ev = ical.events_from_ics(text, *WIN)[0]
        self.assertEqual((ev["uid"], ev["title"], ev["notes"]), (uid, "lunch, with Anna; " + "x" * 80, "a\nb"))
        self.assertTrue(all(len(line.encode()) <= 75 for line in text.split("\r\n")))

    def test_icloud_vcard(self):
        c = ical.contacts_from_vcf("BEGIN:VCARD\nVERSION:3.0\nFN:Kovács Anna\nN:Kovács;Anna;;;\n"
                                   "item1.EMAIL;type=INTERNET;type=pref:anna@example.hu\nitem1.X-ABLabel:_$!<Other>!$_\n"
                                   "TEL;type=CELL;type=VOICE:+36 30 123 4567\nBDAY;VALUE=date:1604-03-14\nEND:VCARD\n"
                                   "BEGIN:VCARD\nVERSION:3.0\nFN:family\nX-ADDRESSBOOKSERVER-KIND:group\nEND:VCARD\n")
        self.assertEqual(len(c), 1)
        self.assertEqual((c[0]["first"], c[0]["emails"][0]["type"], c[0]["phones"][0]["type"], c[0]["birthday"]),
                         ("Anna", "other", "mobile", "--03-14"))


if __name__ == "__main__":
    unittest.main()
