# Overall patch progress

The patching dialog displays one percentage bar for the whole operation. It
stays inside the dialog, keeps the same message throughout, and hands over to
the completion or error popup. Disc analysis retains its activity animation.
That dialog is reserved for selecting a disc. Changing spawn/drop settings
refreshes the output estimate in the background, with a small **Updating output
size…** hint. Settings remain usable; patching waits for the latest verdict.
Stale responses are ignored and actual errors still open a popup.

Core patch entry points report overall work out of 10,000. Fixed work ranges
cover preparation, copying, archive extension, applying changes, verification
and hashing. Compressed images reserve additional ranges for unpacking,
repacking and the complete verification round trip. The underlying writer
continues to use phase-local counters, with separate extension/hash phases.

These percentages estimate work completed; they are not elapsed-time or
remaining-time estimates. The frontend guards against stale events moving
backwards and reserves completion for the verified success event. Every new
attempt starts at zero, including retries after a failure.

Frontend tests exercise repeated phase names, stale/invalid counters, the
99% cap before success, Escape handling and success/error popup handoff. The
opt-in full-disc resource matrix checks that native overall counters start at
zero, never decrease and finish at 10,000, including compressed round trips.

Validation passed on five USA PS2 ISO builds covering the spawn/drop
combinations and all three settings together, plus two USA GameCube RVZ builds
with colours disabled/enabled alongside the other settings. Their output
hashes match the earlier builds. A native desktop ISO run also displayed
increasing percentages from 0 to 99 and then the **Ready to play** popup.

Recovery metadata preparation reports the original-image checksum incrementally
in 8 MiB chunks, followed by manifest capture and compression. This work now
advances the preparation range before copying starts. In-place updates also
report unpacking, clean-image restoration and its checksum verification before
rebuilding, so their preparation does not remain at zero.
