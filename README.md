# OpenDeck VSD Stream Dock N1

An unofficial [OpenDeck](https://github.com/nekename/OpenDeck) device plugin for the
**VSDinside / TreasLin Stream Dock N1** (`5548:1002`), giving it native Linux support with no
vendor software, no Proton and no emulation layer.

This is what makes the N1 context-aware: OpenDeck switches profiles when you switch
applications, so the fifteen LCD keys change their icons and their actions depending on which
window has focus.

## Supported device

| | |
|---|---|
| USB ID | `5548:1002` |
| Reports as | `HOTSPOTEKUSB HID DEMO` |
| Firmware tested | `V3.VSD N1.02.016` |
| Retail names | VSDinside Stream Dock N1, TreasLin N1, ActionRing N1 |

> Mirabox also markets the **Mbox-N4** as "StreamDock N1", but that is `6603:1007` with a
> different key-code map. This plugin does not target it.

## Layout

The N1 stands in **portrait**: 15 LCD keys in 3 columns by 5 rows, with a strip of three
secondary screens above them. It is a numpad replacement, and the 480×854 background screen
agrees. OpenDeck can only register a rectangular grid, so the plugin exposes 3×6 and gives the
secondary strip **row 0**, where the hardware has it — put it last and the deck on screen is
upside down with respect to the one under your hands.

```
 0  1  2      <- secondary strip: left button, right button, knob screen (no button)
 3  4  5      <- LCD keys, device codes 0x01..0x03
 6  7  8
 9 10 11
12 13 14
15 16 17      <- device codes 0x0D..0x0F
```

On the wire the order is reversed: the device numbers the main block `1..=15` and the strip
`16..=18`, so the plugin translates between the two. Images therefore land on the key that
reports the press, which is what the round-trip test in `src/inputs.rs` pins down.

The knob is encoder 0: turning it emits encoder change events, pressing it emits down/up.

Full byte-level trace and the known limitations: [docs/verified-mapping.md](docs/verified-mapping.md).

## Protocol notes

Everything below was measured on hardware, and several points contradict the vendor
documentation.

- Transport is **hidraw via usage page `0xFFA0`** (interface 0). Interface 1 is a fake boot
  keyboard the device emulates until it is switched into software mode.
- Output reports are **1025 bytes**, input reports **513 bytes**. Shorter writes are silently
  dropped.
- Protocol version **3**: 1024-byte packets, unique per-unit serial, both key edges reported.
- **The device must be switched into software mode or it reports no input at all.** Mind the
  value: the vendor SDK documents `2` as software mode, but on this PID `2` selects the
  calculator and **`3`** is software. Sending the documented value leaves the device connected
  and permanently silent.
- **A `CRT..CONNECT` heartbeat is mandatory.** Without it the N1 drops the host and
  re-enumerates after roughly 35 seconds. Devices in the AKP03/N3 family do not need this,
  which is why the upstream plugins never call `keep_alive`.
- The device **echoes `0xFF`** in the key-code byte after every image write. It is not a
  control; treating it as one turns each repaint into a stream of `BadData`.
- Key images are JPEG: **96×96** for the main LCD keys, **64×64** for the secondary screens.
  They render upright at `Rot0` with no mirroring.

| Code | Control |
|---|---|
| `0x01`–`0x0F` | LCD keys 1–15, reading order |
| `0x1E` / `0x1F` | top button left / right |
| `0x23` | knob press |
| `0x32` / `0x33` | knob turn left / right |

Byte 10 carries the state: `0x01` press, `0x00` release.

## Install

```bash
sudo cp 40-opendeck-vsd-n1.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger
```

Replug the device, then build and install the plugin:

```bash
cargo build --release
ID=dev.native.plugins.opendeck-vsd-n1.sdPlugin
DEST=~/.config/opendeck/plugins/$ID
mkdir -p "$DEST"
cp -r assets manifest.json "$DEST/"
cp target/release/opendeck-vsd-n1 "$DEST/opendeck-vsd-n1-linux"
```

Restart OpenDeck. The device appears as "VSD Stream Dock N1".

Note that OpenDeck holds the binary open while it runs, so stop it before copying a new build
or the copy fails with "text file busy".

## Tests

```bash
cargo test
```

The suite pins the hardware-measured corner values, so a change to the grid mapping fails
loudly instead of silently scrambling every icon.

## Credits

- [4ndv](https://github.com/4ndv) for [mirajazz](https://github.com/4ndv/mirajazz) and the
  `opendeck-akp03` / `opendeck-akp153` plugins this is forked from
- [skyf/lightslinger](https://github.com/skyf/lightslinger) for the N1 protocol documentation
- [MiraboxSpace/StreamDock-Device-SDK](https://github.com/MiraboxSpace/StreamDock-Device-SDK)
  for the MIT-licensed reference implementation the key codes were extracted from

## License

GPL-3.0, inherited from `opendeck-akp03`.
