#!/usr/bin/env bash
# Signs in to iCloud Drive for services/apple/Drive.qml, in a terminal.
#
#   icloud-drive-signin.sh <remote> [apple-id]
#
# Creates the rclone iclouddrive remote <remote>, or signs it in again when it exists. rclone's
# own interactive flow can't be used: `rclone config create` answers every question with its
# default unless --all is given, so the two-factor code went in blank, and rclone then jumped to
# a state it doesn't have ("unknown state authenticate"). This drives rclone's non-interactive
# state machine instead (--non-interactive, then --continue --state --result) and reads every
# answer from the terminal itself.
#
# The password is read without echo, obscured through rclone's stdin and passed on only in
# that form; it is never printed or logged. rclone's error output goes to a private temporary
# file, read once for the reason and removed.

set -u

remote=${1:?usage: icloud-drive-signin.sh <remote> [apple-id]}
apple_id=${2:-}

say() { printf '%s\n' "$*"; }

tmp=""
pw=""
obscured=""
cleanup() {
    pw=""
    obscured=""
    [ -n "$tmp" ] && rm -f "$tmp"
}
trap cleanup EXIT
trap 'stty echo <&3 2>/dev/null; say; finish "Sign-in cancelled." 1' INT TERM

# Every answer comes from the controlling terminal, never from whatever stdin happens to be.
if ! { exec 3<>/dev/tty; } 2>/dev/null; then
    say "This needs a terminal to ask for the password and the two-factor code." >&2
    exit 1
fi

finish() {
    say
    say "$1"
    say
    say "Press Enter to close this window."
    IFS= read -r _ <&3 || true
    exit "${2:-0}"
}

ask() { # ask <prompt> [secret]: one non-empty line from the terminal into $answer
    answer=""
    while [ -z "$answer" ]; do
        printf '%s' "$1" >&3
        if [ -n "${2:-}" ]; then
            IFS= read -rs answer <&3 || finish "Sign-in cancelled." 1
            printf '\n' >&3
        else
            IFS= read -r answer <&3 || finish "Sign-in cancelled." 1
            answer=$(printf '%s' "$answer" | tr -d '[:space:]')
        fi
        [ -z "$answer" ] && say "This can't be blank."
    done
}

command -v rclone >/dev/null || finish "rclone is not installed." 1
command -v jq >/dev/null || finish "jq is not installed; it is needed to talk to rclone." 1

tmp=$(mktemp "${XDG_RUNTIME_DIR:-/tmp}/hecate-icloud-signin.XXXXXX") || finish "Could not create a temporary file." 1

# The last error line rclone wrote, without its "Fatal error:"/timestamp prefixes or usage text.
reason() {
    grep -iE 'error|failed' "$tmp" | tail -n 1 |
        sed -E 's/^[0-9\/]+ ([0-9:]+ )?[A-Z]+ ?: ?//; s/^(Fatal error|Error): ?//; s/^Fatal error: ?//'
}

# A readable explanation for a failure, from rclone's message.
explain() {
    local why
    why=$(reason)
    say
    say "iCloud Drive sign-in failed."
    case "$why" in
        *"incorrect username or password"* | *"-20101"* | *"401"*)
            say "Apple did not accept the Apple ID or password. Use your normal Apple ID password:"
            say "app-specific passwords don't work for iCloud Drive." ;;
        *"423"* | *PCS* | *"Advanced Data Protection"* | *ADP*)
            say "Apple refused to hand over iCloud Drive. This happens when Advanced Data Protection"
            say "is on; turn it off on your iPhone (Settings > your name > iCloud > Advanced Data"
            say "Protection), wait a few minutes and try again." ;;
        *"2FA"* | *"two-factor"* | *"code"* | *"-21669"* | *"400"*)
            say "Apple did not accept the two-factor code. Codes expire after a few minutes, so sign"
            say "in again and enter the newest one." ;;
        *"no such host"* | *"dial tcp"* | *"timeout"* | *"connection"*)
            say "Could not reach Apple's servers. Check the network and try again." ;;
        *) ;;
    esac
    [ -n "$why" ] && say "rclone said: $why"
}

# Runs one rclone config step and leaves its JSON answer in $out.
step() {
    : >"$tmp"
    out=$(rclone "$@" --non-interactive 2>"$tmp") || return 1
    jq -e . >/dev/null 2>&1 <<<"$out" || { printf '%s\n' "$out" >>"$tmp"; return 1; }
}

say "Sign in to iCloud Drive"
say
say "Before you start:"
say "  - Advanced Data Protection must be OFF for this Apple ID. With it on, Apple doesn't let"
say "    anything but its own devices reach iCloud Drive. (iPhone: Settings > your name >"
say "    iCloud > Advanced Data Protection.)"
say "  - Use your normal Apple ID password. App-specific passwords do not work for iCloud Drive."
say "  - Keep a trusted Apple device nearby: Apple sends it a two-factor code."
say

existing=no
rclone listremotes --long 2>/dev/null | grep -q "^$remote:[[:space:]]*iclouddrive" && existing=yes

if [ -z "$apple_id" ] && [ "$existing" = yes ]; then
    apple_id=$(rclone config dump 2>/dev/null | jq -r --arg r "$remote" '.[$r].apple_id // empty')
fi
if [ -n "$apple_id" ]; then
    say "Apple ID: $apple_id"
else
    ask "Apple ID (email): "
    apple_id=$answer
fi

if [ "$existing" = yes ]; then
    say "iCloud Drive is already set up; signing in again."
    printf 'Apple ID password (Enter keeps the saved one): ' >&3
    IFS= read -rs pw <&3 || finish "Sign-in cancelled." 1
    printf '\n' >&3
else
    ask "Apple ID password: " secret
    pw=$answer
fi
answer=""

if [ -n "$pw" ]; then
    obscured=$(printf '%s' "$pw" | rclone obscure - 2>/dev/null) || finish "rclone could not obscure the password." 1
    pw=""
fi

say
say "Signing in to Apple..."
if [ "$existing" = yes ]; then
    if [ -n "$obscured" ]; then
        step config update "$remote" apple_id="$apple_id" password="$obscured" --no-obscure
    else
        step config update "$remote"
    fi
else
    step config create "$remote" iclouddrive apple_id="$apple_id" password="$obscured" --no-obscure
fi
ok=$?
obscured=""

if [ "$ok" -ne 0 ]; then
    explain
    # A remote that never signed in would make the next attempt look like a re-sign-in.
    [ "$existing" = no ] && rclone config delete "$remote" >/dev/null 2>&1
    finish "Nothing was changed. Try again from the shell's Apple settings." 1
fi

# rclone's questions, one at a time, until it says there are none left.
while :; do
    state=$(jq -r '.State // ""' <<<"$out")
    err=$(jq -r '.Error // ""' <<<"$out")
    [ -n "$err" ] && say "$err"
    [ -z "$state" ] && break
    # Only reachable with a blank answer, which ask() never sends; rclone has no such state.
    if [ "$state" = authenticate ]; then
        [ "$existing" = no ] && rclone config delete "$remote" >/dev/null 2>&1
        finish "rclone lost track of the sign-in. Start it again." 1
    fi

    if [ "$(jq -r '.Option != null' <<<"$out")" = true ]; then
        say
        jq -r '.Option.Help // ""' <<<"$out"
        count=$(jq -r '.Option.Examples // [] | length' <<<"$out")
        if [ "$count" -gt 0 ]; then
            jq -r '.Option.Examples | to_entries[] | "  \(.key + 1)) \(.value.Help // .value.Value)"' <<<"$out"
            while :; do
                ask "Choose 1-$count: "
                case "$answer" in
                    *[!0-9]*) ;;
                    *) [ "$answer" -ge 1 ] && [ "$answer" -le "$count" ] && break ;;
                esac
                say "Type a number from 1 to $count."
            done
            result=$(jq -r --argjson i "$((answer - 1))" '.Option.Examples[$i].Value' <<<"$out")
        elif [ "$(jq -r '.Option.IsPassword // false' <<<"$out")" = true ]; then
            ask "> " secret
            result=$answer
        else
            ask "> "
            result=$answer
        fi
    else
        result=$(jq -r '.Result // ""' <<<"$out")
    fi
    answer=""

    if ! step config update "$remote" --continue --state "$state" --result "$result"; then
        result=""
        explain
        [ "$existing" = no ] && rclone config delete "$remote" >/dev/null 2>&1
        finish "Sign in again to get a new code." 1
    fi
    result=""
done

say
say "Signed in. Checking iCloud Drive..."
: >"$tmp"
if rclone lsf "$remote:" --max-depth 1 >/dev/null 2>"$tmp"; then
    finish "iCloud Drive is ready. The shell mounts it at ~/iCloud in a moment."
fi
explain
finish "The sign-in was saved, but iCloud Drive could not be opened." 1
