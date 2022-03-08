# espeak-embedded

Generates an exceutable ELF that can be placed into a disk image of an embedded target.
This version uses a Xous-compatible API for sending strings in and wave files back.
See https://github.com/betrusted-io/tts-backend for more details.

Espeak itself is a submodule which is pinned to a branch specific to the embedded target.

## Building

The ELF executable is built with the following command:

`cargo build --target riscv32imac-unknown-xous-elf --release`

The resulting executable file will be in `target/riscv32-imac-unknown-elf-release/espeak-embedded`.
This can be passed into the disk image creation tool for incorporation into target hardware.
