# Astral OS

**A modern x86_64 operating system written in Rust.**

[![Rust](https://img.shields.io/badge/Rust-Nightly-orange.svg)](https://rust-lang.org)
[![x86_64](https://img.shields.io/badge/Arch-x86__64-green.svg)](https://en.wikipedia.org/wiki/X86-64)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Features

### Core System
- **Multi-core SMP** - Per-CPU scheduling with load balancing
- **O(1) Scheduler** - Priority-based scheduling with 5 priority classes
- **Full Memory Management** - Frame allocator, 4-level paging, kernel heap, slab allocator
- **User-Mode Shell** - Syscall-based shell with real system info
- **GUI Desktop** - Window manager with double-buffered compositor

### Drivers
- **Framebuffer** - TrueType font rendering (noto-sans-mono)
- **VirtIO Block** - Modern VirtIO 1.0 disk driver
- **PS/2 Keyboard** - Full scancode translation
- **APIC/IOAPIC** - Modern interrupt handling

### Filesystem
- **PsychicFS** - Block-based filesystem with caching
- **VFS Layer** - Unified filesystem interface

---

## Quick Start

### Prerequisites

```bash
# Rust nightly
rustup toolchain install nightly
rustup default nightly
rustup component add rust-src

# Build tools (Ubuntu/Debian)
sudo apt install build-essential xorriso mtools qemu-system-x86

# Arch Linux
sudo pacman -S base-devel xorriso mtools qemu-system-x86
```

### Build & Run

```bash
make all    # Build everything
make run    # Run in QEMU
```

---

## Usage

### Boot Options
1. **Shell** - User-mode command line interface
2. **Graphics** - Desktop GUI environment
3. **Kernel Shell** - Direct kernel access (Ring 0)

### Shell Commands

| Command | Description |
|---------|-------------|
| `help` | Show all commands |
| `ls` | List files |
| `ps` | Process list |
| `mem` | Memory statistics |
| `cpu` | CPU information |
| `clear` | Clear screen |
| `reboot` | Reboot system |

---

## Architecture

```
Astral OS
├── Memory        Frame allocator, paging, heap, slab
├── Process       Scheduler, context switching, SMP
├── Interrupts    IDT, GDT, TSS, APIC/IOAPIC
├── Drivers       Framebuffer, keyboard, VirtIO, serial
├── Filesystem    VFS, PsychicFS
├── Graphics      Compositor, display server, surfaces
├── GUI           Desktop, windows, theme
├── Shell         Kernel shell, user shell
└── Usermode      Syscall interface, launcher
```

---

## Building from Source

```bash
git clone https://github.com/YOUR_USERNAME/Astral-OS.git
cd Astral-OS
make all
make run
```

### Make Targets

| Target | Description |
|--------|-------------|
| `make all` | Build kernel and create ISO |
| `make run` | Run in QEMU |
| `make run-disk` | Run with persistent disk |
| `make clean` | Remove build artifacts |

---

## Screenshots

*Coming soon*

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

## Acknowledgments

- [Limine Bootloader](https://github.com/limine-bootloader/limine)
- [OSDev Wiki](https://wiki.osdev.org)
- Rust embedded community

---

**Built with Rust 🦀**
