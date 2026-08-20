# Verified control mapping

Measured on a physical VSD Stream Dock N1 (`5548:1002`, serial `81D0DA783809`,
firmware `V3.VSD N1.02.016`) on 2026-08-19, by pressing each control and reading the
raw `(input, state)` bytes the device produced.

## Geometry

The N1 stands in **portrait**: 15 LCD keys in 3 columns by 5 rows, with a strip of three
smaller screens **above** them. This is a numpad replacement, and the 480x854 background
screen agrees. A 5-wide reading of the "3x5" in the marketing copy is wrong and scrambles
every icon.

OpenDeck can only register a rectangular grid, so the plugin exposes 3x6 and gives the
secondary strip **row 0** — where the hardware has it. Put it in the last row instead and
OpenDeck draws the deck upside down with respect to the hands using it.

```
grid   device code          what it is
 0  1  2   0x1E 0x1F  --    secondary strip: two 64x64 screens with a button behind
                            each, then a screen with the knob under it and no button
 3  4  5   0x01 0x02 0x03   main LCD block, 96x96, plain reading order
 6  7  8   0x04 0x05 0x06
 9 10 11   0x07 0x08 0x09
12 13 14   0x0A 0x0B 0x0C
15 16 17   0x0D 0x0E 0x0F
```

**Display slots run in the opposite order to the grid.** On the wire the device numbers the
main block `1..=15` first and the secondary strip `16..=18` after it, while the grid puts the
strip first. `opendeck_to_device` in `src/inputs.rs` is that translation, and
`image_positions_round_trip_back_to_the_key_that_reports_them` is the test that keeps a key's
image on the key that reports the press.

## Confirmed by press

| Control | Device code | Grid slot | OpenDeck event |
|---|---|---|---|
| top-left LCD | `0x01` | 3 | `ButtonDown(3)` |
| top-middle LCD | `0x02` | 4 | `ButtonDown(4)` |
| top-right LCD | `0x03` | 5 | `ButtonDown(5)` |
| bottom-left LCD | `0x0D` | 15 | `ButtonDown(15)` |
| bottom-right LCD | `0x0F` | 17 | `ButtonDown(17)` |
| left strip button | `0x1E` | 0 | `ButtonDown(0)` |
| knob, one click right | `0x33` | — | `EncoderTwist(0, +1)` |

Byte 10 carries the state: `1` press, `0` release. Both edges are reported.

The codes not in that table — right strip button `0x1F`, knob press `0x23`, knob left `0x32`,
and the ten interior LCD keys — come from the MIT-licensed MiraboxSpace SDK and from
[skyf/lightslinger](https://github.com/skyf/lightslinger)'s `docs/protocol-events.md`, which
documents the same table for a second unit (serial `C771C7781126`). Every code we did press
agreed with it, so the block is consistent, but the rows above are the ones this unit produced
under a finger.

Images render **upright at `Rot0` with no mirroring**, verified by pushing numbered tiles to
all 15 keys and reading them off the glass.

## Where this contradicts the published protocol

`lightslinger/docs/protocol-init-and-images.md` documents the mode enum as
`0=KEYBOARD, 1=CALCULATOR, 2=SOFTWARE` and initialises with `switchMode(2)`. On this unit the
values are shifted by one: `MOD` takes `0x30 + mode`, **`2` selects the calculator and `3` is
software**. Sending the documented `2` leaves the device connected, accepting images, and
permanently silent on input — which looks exactly like a broken reader.

## Known limitations

- **One input report is lost immediately after each connect.** mirajazz's `DeviceState` starts
  with empty vectors, so the first diff is truncated by `zip` and the first report after
  connecting yields nothing. In practice the first key pressed after a connect or a wake may
  do nothing. Reported upstream as
  [mirajazz#21](https://github.com/4ndv/mirajazz/issues/21); the plugin works around it by
  priming the reader state.
- **The third secondary screen** (grid index 2) has no physical button behind it on this unit,
  so it is display-only and carries the knob. If your unit's two buttons turn out to be the
  outer two rather than the first two, swap the targets in `device_to_opendeck`.
- **Chording is not supported.** One report carries one key, so holding two keys at once
  reports the older one as released. Same behaviour as the upstream akp03/akp153 plugins.
- **The 480x854 background screen is not driven.** The `BGPIC` command exists in the vendor's
  `libtransport` binary and the device acknowledges it by resetting the panel to grey, but no
  project in this family has got image data to render — see lightslinger's notes. A USB capture
  from the official Windows software is what is missing.
