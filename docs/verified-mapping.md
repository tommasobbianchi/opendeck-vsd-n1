# Verified control mapping

Measured on a physical VSD Stream Dock N1 (`5548:1002`, serial `81D0DA783809`,
firmware `V3.VSD N1.02.016`) on 2026-08-19, by pressing each control and reading the
raw `(input, state)` bytes the device produced.

## Geometry

The N1 stands in **portrait**: 15 LCD keys in 3 columns by 5 rows. This is a numpad
replacement, and the 480×854 background screen agrees. A 5-wide reading of the "3x5"
in the marketing copy is wrong and scrambles every icon.

Key numbering is plain reading order, and the display slot equals the OpenDeck grid
index plus one — for the main keys *and* the secondary strip alike.

```
grid   device code        OpenDeck 3x6 grid
 0  1  2   0x01 0x02 0x03      row 0
 3  4  5   0x04 0x05 0x06      row 1
 6  7  8   0x07 0x08 0x09      row 2
 9 10 11   0x0A 0x0B 0x0C      row 3
12 13 14   0x0D 0x0E 0x0F      row 4
15 16 17   0x1E 0x1F  --       row 5 (secondary strip)
```

## Confirmed by press

| Control | Device code | OpenDeck event |
|---|---|---|
| top-left LCD | `0x01` | `ButtonDown(0)` |
| top-middle LCD | `0x02` | `ButtonDown(1)` |
| top-right LCD | `0x03` | `ButtonDown(2)` |
| bottom-right LCD | `0x0F` | `ButtonDown(14)` |
| left top button | `0x1E` | `ButtonDown(15)` |
| knob, one click right | `0x33` | `EncoderTwist(0, +1)` |

Byte 10 carries the state: `1` press, `0` release. Both edges are reported.

Images render **upright at `Rot0` with no mirroring**, verified by pushing numbered
tiles to all 15 keys and reading them off the glass.

## Known limitations

- **One input report is lost immediately after each connect.** mirajazz's `read_input`
  drops any report that does not start with the `ACK` prefix, before the plugin's
  `process_input` is called, and the first report after connecting fails that check. In
  practice the first key pressed after a connect or a wake may do nothing. Not fixable
  from a plugin without forking mirajazz; worth reporting upstream.
- **The third secondary screen** (grid index 17) has no physical button behind it on this
  unit, so it is display-only. If your unit's two buttons turn out to be the outer two
  rather than the first two, swap the targets in `device_to_opendeck`.
- **Chording is not supported.** One report carries one key, so holding two keys at once
  reports the older one as released. Same behaviour as the upstream akp03/akp153 plugins.
- **The 480×854 background screen is not driven.** No project in this device family has
  worked out how to address it over raw HID.
