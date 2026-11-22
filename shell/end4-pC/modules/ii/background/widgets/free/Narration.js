.pragma library

// The narration line the free display clock and `fortune` share: one line
// about now, picked from what is true. It only changes when the hour does.
//   Narration.line(now, { uptime: DateTime.uptime, weather: Weather.data?.description,
//                         battery: Battery, nextEvent: DesktopWidgets.nextEvent })

// "2d, 5h, 3m" as whole hours
function uptimeHours(uptime) {
    const u = String(uptime ?? "");
    const d = Number((u.match(/(\d+)d/) ?? [])[1] ?? 0);
    const h = Number((u.match(/(\d+)h/) ?? [])[1] ?? 0);
    return d * 24 + h;
}

function line(now, facts) {
    const hours = uptimeHours(facts.uptime);
    const weather = String(facts.weather ?? "").toLowerCase();
    const battery = facts.battery;
    const hour = now.getHours();
    const seed = now.getFullYear() * 1000 + Math.floor(now.getTime() / 3600000);
    const options = [];
    if (hours >= 3)
        options.push("You have been at this for " + (hours >= 24 ? Math.floor(hours / 24) + " days" : hours + " hours") + ". The tea has gone cold.");
    if (battery?.available && battery.isLowAndNotCharging)
        options.push("The battery is thin. The charger is somewhere nearby.");
    else if (battery?.available && battery.isFullAndCharging)
        options.push("The battery is full. It may be unplugged now.");
    if (weather.includes("rain") || weather.includes("drizzle"))
        options.push("It is raining out there. A good reason to stay.");
    else if (weather.includes("snow"))
        options.push("It is snowing. The world has gone quiet.");
    const ev = facts.nextEvent;
    if (ev && ev.start - now.getTime() > 0 && ev.start - now.getTime() < 2 * 3600000)
        options.push(ev.title + " is close. It will not wait for you.");
    if (hour >= 1 && hour < 5)
        options.push("It is very late. The code will still be there tomorrow.");
    else if (hour >= 5 && hour < 9)
        options.push("The day is new. Nothing has gone wrong yet.");
    else if (hour >= 22)
        options.push("The evening is nearly spent.");
    if (options.length === 0)
        options.push("Nothing to report. That is also news.");
    return options[seed % options.length];
}
