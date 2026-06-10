# PS5 Drive Enabler

Make (almost) any NVMe SSD work in the PlayStation 5's M.2 expansion slot,
including drives that the PS5 normally rejects for being "too slow" (PCIe Gen3,
or Gen4 drives below Sony's speed requirement).

The tool writes a small 2 MB "enabler" image to the very start of the drive.
The PS5 reads that region during its drive check and, when the image is present,
skips the speed requirement. After that the PS5 treats the drive like any other
expansion SSD.

There are two front-ends that do the same thing:

- **GUI**: a step-by-step wizard. Recommended for most people.
- **CLI**: a command-line tool for terminal users and scripts.

Available for Windows, macOS, and Linux.

## Read this first

- **This erases the first 2 MB of whatever drive you select.** Picking the wrong
  drive can wipe data. The tool refuses to touch your system disk and warns on
  internal disks, but you are responsible for choosing the correct drive.
- Use a drive you are okay with erasing completely. Once it is in the PS5, the
  console takes the drive over completely.
- This is a community tool. It is not affiliated with Sony. Modifying your
  console's storage may not be officially supported. Use at your own risk.

## What you need

1. An NVMe M.2 SSD you want to use in the PS5.
2. A USB-to-M.2 NVMe enclosure (a small case that turns an M.2 SSD into a USB
   drive). This is the easiest and safest way to connect the SSD to your computer.
3. A computer running Windows, macOS, or Linux.
4. Administrator / root rights on that computer.

You can also use an internal M.2 slot in a desktop PC, but a USB enclosure is
strongly recommended: it keeps the drive clearly separate from your system
disk, so it is much harder to pick the wrong one.

## Quick start (GUI)

1. Put the SSD in the USB enclosure and plug it into your computer.
2. Download the GUI for your system from the
   [Releases page](https://github.com/koekden/PS5-Drive-Enabler/releases):
   - Windows: `ps5-drive-enabler-gui.exe`
   - macOS: `ps5-drive-enabler-gui` (or the `.app`)
   - Linux: `ps5-drive-enabler-gui`
3. Run it as administrator / root (see [How to run with admin rights](#how-to-run-with-admin-rights)).
4. Follow the on-screen steps:
   1. **Start**
   2. **Choose the drive**: pick the one marked *USB / external* whose size
      matches your SSD.
   3. **Confirm**: tick the two checkboxes.
   4. **Flash**: wait for "Success!".
5. Eject the drive, take the SSD out of the enclosure, and install it in the
   PS5's M.2 slot. Power on; the PS5 starts normally and the SSD appears under
   **Settings > Storage**, ready to use. No formatting step is needed.

That's it.

## Quick start (CLI)

```sh
# 1. List your disks and find the target drive
ps5-drive-enabler list

# 2. Flash it (replace <ID> with the id from the list, e.g. disk4 / nvme0n1 / 2)
ps5-drive-enabler flash --device <ID>
```

Example session:

```text
$ sudo ps5-drive-enabler list
ID         MODEL                               SIZE BUS        TYPE
------------------------------------------------------------------------------
disk0      APPLE SSD AP1024Z                1.00 TB Apple Fab… SYSTEM  <-- do NOT use
disk4      Samsung SSD 980 1TB              1.00 TB USB        external/USB (expected)

$ sudo ps5-drive-enabler flash --device disk4
Target drive:
  ID:    disk4
  Model: Samsung SSD 980 1TB
  Size:  1.00 TB
  Bus:   USB
  Type:  external/USB (expected)

This will ERASE the first 2 MB of the drive above.
Type the disk id ('disk4') to continue, or anything else to abort:
> disk4

Writing enabler image [####################] 100%
Enabler image written.
Verifying [####################] 100%
Verification passed.

Done. Next steps:
  1. Safely eject the drive and remove it from the enclosure.
  2. Install it into the PS5's M.2 expansion slot.
  3. Power on. The SSD shows up under Settings > Storage, ready to use.
```

### CLI commands

| Command   | What it does                                                       |
|-----------|--------------------------------------------------------------------|
| `list`    | Show all detected disks so you can identify the target.            |
| `flash`   | Write the enabler image to a disk (erases its first 2 MB).         |
| `verify`  | Read the first 2 MB back and confirm the enabler image is present. |

### `flash` options

| Option            | Meaning                                                       |
|-------------------|---------------------------------------------------------------|
| `-d, --device`    | Target disk id or path (from `list`). **Required.**           |
| `-y, --yes`       | Skip the typed confirmation prompt.                           |
| `--force`         | Allow writing to internal disks (the system disk is always refused). |
| `--no-verify`     | Skip the read-back verification step.                         |
| `--image <FILE>`  | Use a custom image file instead of the built-in one.          |

## How to run with admin rights

Writing directly to a disk requires elevated privileges.

**Windows**

- Right-click the program, then **Run as administrator**.
- Or open Windows Terminal / PowerShell as administrator, then run the CLI.

**macOS**

- For the CLI: prefix with `sudo`, e.g. `sudo ps5-drive-enabler list`.
- For the GUI: launch from a terminal with `sudo /path/to/ps5-drive-enabler-gui`.
- On first run, macOS Gatekeeper may block an unsigned download. Allow it under
  **System Settings > Privacy & Security**, or remove the quarantine flag:
  `xattr -dr com.apple.quarantine /path/to/ps5-drive-enabler-gui`.

**Linux**

- Prefix with `sudo`, e.g. `sudo ps5-drive-enabler list`.
- Make the file executable first if needed: `chmod +x ps5-drive-enabler`.

## How it works

The PS5 checks the first part of an inserted M.2 drive when deciding whether it
meets its storage requirements. A specific 2 MB payload written at the very
beginning of the drive (logical block address 0) causes the console to skip the
PCIe Gen4 speed check, allowing slower drives to be used.

That payload is simply the **first ~2 MB of a drive that the PS5 itself already
formatted** (taken from a supported Gen4 drive). Copying it onto an
otherwise-rejected Gen2/Gen3 drive makes the console accept that drive too.

This tool writes the embedded payload (`gen3_enabler_bringus.img`) to offset 0
of the drive you choose, then reads it back to make sure every byte landed
correctly. When you boot the PS5 with the drive installed, it will recognize it
under **Settings → Storage** (it may initialize/format the drive on first use).

Low-level disk access is implemented per platform:

| Platform | Enumeration            | Raw write target            |
|----------|------------------------|-----------------------------|
| Linux    | `/sys/block`           | `/dev/nvme0n1`, `/dev/sdX`  |
| macOS    | `diskutil`             | `/dev/rdiskN` (unmounted)   |
| Windows  | PowerShell `Get-Disk`  | `\\.\PhysicalDriveN` (offline) |

## Building from source

You need the [Rust toolchain](https://rustup.rs) (a recent stable release).

```sh
git clone https://github.com/koekden/PS5-Drive-Enabler
cd PS5-Drive-Enabler

# Build both tools, optimized
cargo build --release

# Binaries land in target/release/
#   CLI:  ps5-drive-enabler        (.exe on Windows)
#   GUI:  ps5-drive-enabler-gui    (.exe on Windows)
```

Build just one:

```sh
cargo build --release -p ps5de-cli    # CLI only
cargo build --release -p ps5de-gui    # GUI only
```

Run the tests:

```sh
cargo test
```

### Project layout

```text
core/   Shared library: disk enumeration + flash/verify logic (image embedded here)
cli/    Command-line front-end (clap)
gui/    Graphical front-end (egui/eframe)
gen3_enabler_bringus.img   The 2 MB payload, embedded into the binaries at build time
```

## Troubleshooting

**"permission denied" / "Run as administrator"**
You didn't start the tool with elevated rights. See
[How to run with admin rights](#how-to-run-with-admin-rights).

**My drive isn't in the list**
- Make sure the USB enclosure is firmly connected and the SSD is seated.
- Click **Refresh** (GUI) or re-run `list` (CLI).
- A brand-new SSD with no partitions still shows up. Look for the right *size*.

**"device is busy"**
Close any program or window using the drive (file managers, backup software),
then try again. The tool already unmounts/offlines the drive automatically.

**The drive is marked "internal" but it really is my external SSD**
Some USB enclosures report as SATA/NVMe rather than USB. Double-check the size
and model, then add `--force` (CLI). The system disk is always refused.

**The PS5 still rejects the drive**
- Re-run with `verify` to confirm the image is present.
- Make sure you flashed the actual SSD you installed (not a different drive).
- Confirm the SSD is a supported M.2 2230/2242/2260/2280/22110 form factor and
  is properly seated and screwed down in the PS5.

## FAQ

**Does this damage my SSD?**
No. It writes 2 MB to the start of the drive. If you ever want to use the SSD
somewhere else again, just reformat it.

**Do I need to keep this tool after flashing?**
No. Once the PS5 has accepted the drive, you're done.

**Can I undo it?**
There's nothing to undo. If you want to use the SSD in a PC again, just
reformat it.

**Is it safe for my computer's data?**
The tool refuses to write to your system disk and warns on internal disks. As
long as you pick the external drive that matches your SSD's size, your computer
is untouched.

## Credits

This trick was discovered and refined by the **PS5 Linux Discord** community.
Credit belongs to them - all this project does is wrap their method in a safe,
cross-platform flasher.

The story, as told in the video below:

- **Windfox** — made the original discovery: restoring an M.2 backup image (from
  the "Netflix-and-hack" jailbreak setup) onto a Gen2/Gen3 drive caused the PS5
  to **bypass its drive-speed check**.
- **HiPmp5** — did the experimentation that made it practical: shrinking the
  required data from a full 256 GB dump down to ~94 MB and finally to just
  **1.7 MB**, and showing that the first ~2 MB of *any* drive already formatted
  by the PS5 (from a working Gen4 drive) is all you need.
- [Jon Bringus / Bringus Studios](https://www.youtube.com/@JonBringus) —
  documented and popularized the method, produced the `gen3_enabler_bringus.img`
  dump that this tool embeds, and brought it to a wide audience:
  ["I'm about to save a lot of PS5 owners some money"](https://youtu.be/Uds315QBUnE).

This project exists so nobody has to break out `dd` by hand and risk targeting
the wrong disk.

## License

MIT, see [LICENSE](LICENSE).

This project is provided as-is, without warranty. It is not affiliated with,
endorsed by, or supported by Sony Interactive Entertainment. "PlayStation" and
"PS5" are trademarks of Sony Interactive Entertainment Inc.
