pragma Singleton
pragma ComponentBehavior: Bound

// From https://git.outfoxxed.me/outfoxxed/nixnew
// It does not have a license, but the author is okay with redistribution.

import QtQml.Models
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Services.Mpris
import qs.modules.common
import "../modules/ii/mediaControls/playerPriority.js" as Priority

/**
 * A service that provides easy access to the active Mpris player.
 */
Singleton {
	id: root;
	property var players: Mpris.players.values.filter(player => isRealPlayer(player));

	// Which player the shell shows. Spotify and Cider come first, whichever of
	// them played last; a browser or mpv only when neither is open. The rule
	// itself is in playerPriority.js, which has its own check.
	// A player's stamp is when it last started or stopped playing.
	property var stamps: ({});
	property int stampRevision: 0;
	function touch(player: MprisPlayer): void {
		if (!player) return;
		stamps[player.dbusName] = Date.now();
		stampRevision++;
	}

	// Every real player, best first.
	readonly property var rankedPlayers: {
		const _ = root.stampRevision;
		const described = root.players.map((p, i) => ({
			player: p,
			identity: p.identity ?? "",
			desktopEntry: p.desktopEntry ?? "",
			dbusName: p.dbusName ?? "",
			isPlaying: p.isPlaying,
			stamp: root.stamps[p.dbusName] ?? 0,
			order: i,
		}));
		return Priority.rank(described).map(d => d.player);
	}

	// A player picked by hand (the chips in the media panel). It lasts until
	// that player closes or the same chip is picked again.
	property MprisPlayer manualPlayer: null;
	readonly property bool manualValid: manualPlayer !== null && root.players.indexOf(manualPlayer) !== -1;
	property MprisPlayer activePlayer: manualValid ? manualPlayer : (rankedPlayers[0] ?? null);
	signal trackChanged(reverse: bool);

	property bool __reverse: false;

	property var activeTrack;

	readonly property bool hasActivePlasmaIntegration: Mpris.players.values.some(
		p => p.dbusName?.startsWith('org.mpris.MediaPlayer2.plasma-browser-integration')
	)
	function isRealPlayer(player) {
        if (!Config.options.media.filterDuplicatePlayers) {
            return true;
        }
        return (
            // Remove native browser buses only if plasma-browser-integration is actually active on D-Bus
            !(hasActivePlasmaIntegration && player.dbusName.startsWith('org.mpris.MediaPlayer2.firefox')) && !(hasActivePlasmaIntegration && player.dbusName.startsWith('org.mpris.MediaPlayer2.chromium')) &&
            // playerctld just copies other buses and we don't need duplicates
            !player.dbusName?.startsWith('org.mpris.MediaPlayer2.playerctld') &&
            // Non-instance mpd bus
            !(player.dbusName?.endsWith('.mpd') && !player.dbusName.endsWith('MediaPlayer2.mpd')));
    }

	// Stamp a player whenever it starts or stops playing, and the ones that are
	// already playing when the shell starts.
	Instantiator {
		model: Mpris.players;

		Connections {
			required property MprisPlayer modelData;
			target: modelData;

			Component.onCompleted: {
				if (modelData.isPlaying) root.touch(modelData);
			}

			function onPlaybackStateChanged() {
				root.touch(modelData);
			}
		}
	}

	Connections {
		target: activePlayer

		function onPostTrackChanged() {
			root.updateTrack();
		}

		function onTrackArtUrlChanged() {
			// console.log("arturl:", activePlayer.trackArtUrl)
			// root.updateTrack();
			if (root.activePlayer.uniqueId == root.activeTrack.uniqueId && root.activePlayer.trackArtUrl != root.activeTrack.artUrl) {
				// cantata likes to send cover updates *BEFORE* updating the track info.
				// as such, art url changes shouldn't be able to break the reverse animation
				const r = root.__reverse;
				root.updateTrack();
				root.__reverse = r;

			}
		}
	}

	onActivePlayerChanged: this.updateTrack();

	function updateTrack() {
		//console.log(`update: ${this.activePlayer?.trackTitle ?? ""} : ${this.activePlayer?.trackArtists}`)
		this.activeTrack = {
			uniqueId: this.activePlayer?.uniqueId ?? 0,
			artUrl: this.activePlayer?.trackArtUrl ?? "",
			title: this.activePlayer?.trackTitle || Translation.tr("Unknown Title"),
			artist: this.activePlayer?.trackArtist || Translation.tr("Unknown Artist"),
			album: this.activePlayer?.trackAlbum || Translation.tr("Unknown Album"),
		};

		this.trackChanged(__reverse);
		this.__reverse = false;
	}

	property bool isPlaying: this.activePlayer && this.activePlayer.isPlaying;
	property bool canTogglePlaying: this.activePlayer?.canTogglePlaying ?? false;
	function togglePlaying() {
		if (this.canTogglePlaying) this.activePlayer.togglePlaying();
	}

	property bool canGoPrevious: this.activePlayer?.canGoPrevious ?? false;
	function previous() {
		if (this.canGoPrevious) {
			this.__reverse = true;
			this.activePlayer.previous();
		}
	}

	property bool canGoNext: this.activePlayer?.canGoNext ?? false;
	function next() {
		if (this.canGoNext) {
			this.__reverse = false;
			this.activePlayer.next();
		}
	}

	property bool canChangeVolume: this.activePlayer && this.activePlayer.volumeSupported && this.activePlayer.canControl;

	property bool loopSupported: this.activePlayer && this.activePlayer.loopSupported && this.activePlayer.canControl;
	property var loopState: this.activePlayer?.loopState ?? MprisLoopState.None;
	function setLoopState(loopState: var) {
		if (this.loopSupported) {
			this.activePlayer.loopState = loopState;
		}
	}

	property bool shuffleSupported: this.activePlayer && this.activePlayer.shuffleSupported && this.activePlayer.canControl;
	property bool hasShuffle: this.activePlayer?.shuffle ?? false;
	function setShuffle(shuffle: bool) {
		if (this.shuffleSupported) {
			this.activePlayer.shuffle = shuffle;
		}
	}

	// Pick a player by hand; the one already picked goes back to automatic.
	function setActivePlayer(player: MprisPlayer) {
		const current = root.activePlayer;
		if (!player || player === root.manualPlayer) {
			this.manualPlayer = null;
		} else {
			this.manualPlayer = player;
		}
		const next = root.activePlayer;
		if (next && current) {
			this.__reverse = root.players.indexOf(next) < root.players.indexOf(current);
		} else {
			this.__reverse = false;
		}
	}

	IpcHandler {
		target: "mpris"

		function pauseAll(): void {
			for (const player of Mpris.players.values) {
				if (player.canPause) player.pause();
			}
		}

		function playPause(): void { root.togglePlaying(); }
		function previous(): void { root.previous(); }
		function next(): void { root.next(); }
	}
}