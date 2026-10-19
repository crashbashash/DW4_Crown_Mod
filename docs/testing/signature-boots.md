# Signed-image boot/load check

Verified on 19 October 2026 using newly signed v0.11.0 images, with Better Loot,
four-player spawns and random crown colours enabled. Each image's metadata checksum,
release and options were checked before launch.

| Platform | Release | Runtime identity | Observed game output | Result |
| --- | --- | --- | --- | --- |
| PS2 | USA | SLUS-20836 | Title screen | Pass |
| PS2 | Europe | SLES-53405 | Opening animation | Pass |
| PS2 | Japan | SLPS-25453 | Japanese game health notice | Pass |
| GameCube | USA | GDJEB2 | Game memory-card check | Pass |
| GameCube | Japan | GDJJB2 | Japanese game health notice | Pass |

PCSX2 and Dolphin launched directly from the signed images in separate test
profiles. No savestate was loaded. Original discs, memory cards and regular
emulator profiles were preserved. Emulator sockets supplied control/capture;
no desktop mouse or keyboard input was used. All test instances were stopped.

The requested scope was image loading and visible startup output. This does not
claim menu traversal, a gameplay session or physical-console verification.

Evidence is retained outside the repository in the shared development workspace
at `../outputs/crown-signature-boots/`: `Results.txt`, `results.json`, `images.json`,
per-release emulator logs, launch records and screenshots. For context, the same
signature change also passed the [data/container validation](../image-signature.md).
