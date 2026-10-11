# Name candidates

Witch-themed, short, easy to type as a binary and config dir. "Clash" notes
known projects with the same name; none are checked against nixpkgs or
crates.io yet, so do that before deciding.

## Top picks

| name         | why it fits                                                    | clash |
| ------------ | -------------------------------------------------------------- | ----- |
| **cauldron** | a compositor is literally a pot where surfaces get mixed        | a few small unrelated repos |
| **besom**    | the witch's broom; sweeps windows around a stacking desk       | Pulumi's Scala SDK "Besom" |
| **glamour**  | the spell that changes how things look: rounded corners, blur  | Charm's Go markdown renderer `glamour` (a library, no binary clash) |
| **coven**    | the gathering of clients around the compositor                 | some small repos |
| **sigil**    | a drawn mark with power; short binary                          | Sigil EPUB editor (binary `sigil`, in nixpkgs) |
| **scry**     | scrying = seeing through a surface; screencopy-friendly         | a few small CLIs |

## More

| name          | note                                                         |
| ------------- | ------------------------------------------------------------ |
| hex           | current placeholder; `hex` is taken by hex editors/utilities, hence `wl-hex` |
| grimoire      | spellbook; nice for a config-driven project, long to type     |
| familiar      | the companion that does your bidding                          |
| athame        | ritual knife; cuts windows into shape                         |
| cantrip       | a tiny spell; fits a small, focused compositor                |
| wyrd          | fate; short and odd                                           |
| sabbat        | the witches' gathering                                        |
| hecate        | goddess of witchcraft; Hecate is also a hex editor            |
| circe         | the sorceress; Circe is a Scala JSON lib                      |
| morgana       | Morgan le Fay                                                 |
| nightshade    | poison plant; long                                            |
| belladonna    | as above                                                      |
| mandrake      | screaming root; was a Linux distro name                       |
| mugwort       | witch herb, cute                                              |
| vervain       | witch herb                                                    |
| wormwood      | absinthe herb                                                 |
| hemlock       | poison; also a Lisp-ish project                               |
| pentacle      | five-point star                                               |
| ouija         | spirit board; talks to clients                                |
| tarot         | cards, i.e. windows                                           |
| broomstick    | longer besom                                                  |
| moonwell      | where witches scry                                            |
| hexagram      | keeps the "hex" link                                          |
| witchlight    | soft glow; borders                                            |

## Naming knobs once picked

Binary, config dir (`~/.config/<name>/`), env prefix (`<NAME>_SOCKET`,
`<NAME>_LOG`), `XDG_CURRENT_DESKTOP`, session file, Nix attr
(`programs.<name>`), crate name. The repo can stay `wl-hex` or be renamed.
