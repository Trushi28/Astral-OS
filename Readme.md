# Astral OS v0.3.0

**A reality-aware operating system with causal tracking, dream states, and intent-based computing.**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: Nightly](https://img.shields.io/badge/Rust-Nightly-orange.svg)](https://rust-lang.org)
[![Architecture: x86_64](https://img.shields.io/badge/Arch-x86__64-green.svg)](https://en.wikipedia.org/wiki/X86-64)

---

## 🌟 Features

### Core Reality Engine
- **Reality Branching**: Fork and merge parallel execution timelines
- **Causal Event Tracking**: Complete causality chain for every system event
- **Dream State Engine**: Idle-time optimization and predictive file access
- **Intent-Based Syscalls**: High-level intent resolution instead of explicit syscalls

### Memory Management
- **Physical Frame Allocator**: Lock-free bitmap-based allocator supporting up to 4GB RAM
- **Page Table Manager**: Full 4-level paging with recursive mapping
- **Kernel Heap**: Proper linked-list allocator with coalescing (100MB default)

### Process Management
- **Cooperative Scheduler**: Round-robin scheduling with time slices
- **Full Context Switching**: Complete register preservation including FPU state
- **Process Isolation**: Per-process page tables with kernel/user separation

### Filesystem
- **PsychicFS**: Predictive filesystem with access pattern learning
- **Reality-Aware Storage**: Files tracked across timeline branches
- **Simple Design**: 8KB max file size, 256 files, designed for demos

### Drivers
- **Framebuffer**: TrueType font rendering via `fontdue` (SpaceMono)
- **VirtIO Block**: Full modern VirtIO 1.0 implementation with proper queue management
- **Serial Port**: COM1 debugging output
- **PS/2 Keyboard**: Full scancode translation with modifier support

---

## 🏗️ Architecture

```
Astral OS
├── Memory Layer
│   ├── Physical Frame Allocator (lock-free bitmap)
│   ├── Page Table Manager (4-level paging)
│   └── Kernel Heap (linked-list allocator)
├── Process Layer
│   ├── Process Control Blocks
│   ├── Scheduler (round-robin)
│   └── Context Switching (assembly)
├── Interrupt Layer
│   ├── IDT with 256 entries
│   ├── GDT & TSS (proper ring transitions)
│   ├── PIC (8259) initialization
│   └── Exception/IRQ handlers
├── Driver Layer
│   ├── Framebuffer (fontdue TrueType)
│   ├── VirtIO Block (modern spec)
│   ├── Serial (COM1 debug)
│   └── Keyboard (PS/2)
├── Filesystem Layer
│   └── PsychicFS (predictive FS)
├── Reality Engine
│   ├── Causality Tracker
│   ├── Dream State Manager
│   └── Intent Resolver
└── Shell
    └── Interactive command interface
```

---

## 🔧 Building

### Prerequisites

```bash
# Rust nightly toolchain
rustup toolchain install nightly
rustup default nightly
rustup component add rust-src --toolchain nightly

# Build tools
# For apt based linux
sudo apt install build-essential xorriso mtools

# For Arch-based linux
sudo pacman -S base-devel xorriso mtools

```

### Required Font

Place `SpaceMono-Regular.ttf` in the `kernel/` directory:

```bash
# Download from Google Fonts
wget https://github.com/googlefonts/spacemono/raw/main/fonts/ttf/SpaceMono-Regular.ttf \
  -O kernel/SpaceMono-Regular.ttf
```

### Build & Run

```bash

# To Build via Make at source file
make all

# else only Build kernel
cd kernel
cargo +nightly build --release

# Create bootable ISO
cd .. 
make iso

# Run in QEMU
make run

# Run with persistent VirtIO disk
make run-disk
```

---

## 🎮 Usage

### Boot Process

1. Limine bootloader loads kernel
2. Framebuffer initialized with TrueType rendering
3. Memory management configured
4. Reality engine initialized (root reality = 0)
5. Interrupts enabled (timer + keyboard)
6. VirtIO disk detected and initialized
7. Shell starts

### Shell Commands

#### System Commands
```bash
help                    # Show all commands
clear                   # Clear screen
info                    # System information
mem                     # Memory statistics
ps                      # Process list
reboot                  # Reboot system
```

#### Reality Engine Commands
```bash
reality status          # Show current reality
reality fork            # Create new reality branch
reality merge           # Merge back to root reality

causality show [N]      # Show recent N events (default: 10)
causality trace <ID>    # Show causal chain for event ID
causality stats         # Causality log statistics

dream status            # Dream engine status
dream force             # Force enter dream state
dream wake              # Wake from dream state

timeline show           # Display timeline branches
timeline jump <ID>      # Jump to reality ID
timeline branch         # Create new branch
```

#### Display Commands
```bash
fb stats                # Framebuffer statistics
fb test                 # Test color rendering

theme reality           # Reality mode theme (default)
theme dream             # Dream mode theme (purple)
```

#### Filesystem Commands
```bash
format                  # Format PsychicFS (WARNING: erases data)
mount                   # Mount filesystem
ls                      # List files
cat <filename>          # Display file contents
write <file> <text>     # Write text to file
touch <filename>        # Create empty file
rm <filename>           # Delete file
```

---

## 📊 Memory Layout

```
0x0000_0000_0000_0000  ┌─────────────────────┐
                       │   Reserved (NULL)   │
0x0000_0000_0040_0000  ├─────────────────────┤
                       │   User Space        │
                       │   (up to 512GB)     │
0x0000_7FFF_FFFF_FFFF  ├─────────────────────┤
                       │   Kernel Hole       │
0xFFFF_8000_0000_0000  ├─────────────────────┤
                       │   Kernel Heap       │
                       │   (100MB)           │
0xFFFF_8006_4000_0000  ├─────────────────────┤
                       │   Kernel Code/Data  │
0xFFFF_FFFF_8000_0000  ├─────────────────────┤
                       │   HHDM Mapping      │
                       │   (Physical Memory) │
0xFFFF_FFFF_FFFF_FFFF  └─────────────────────┘
```

---

## 🧪 Testing

### Memory Allocator Test
```bash
# Boot and observe output
# Should show:
#   Vec: 5 elements ✓
#   String: "Astral OS" ✓
#   Box: 42 ✓
```

### VirtIO Disk Test
```bash
# After boot:
mount
write test.txt "Hello from Astral OS!"
ls                    # Should show test.txt
cat test.txt          # Should display content
```

### Reality Engine Test
```bash
reality fork          # Creates reality 1
causality show 5      # Shows recent events including fork
reality merge         # Back to reality 0
timeline show         # Shows branch count
```

---

## 🐛 Known Issues & Limitations

### Current Limitations
- **Single-core only**: No SMP support
- **No userspace**: All code runs in ring 0 (kernel mode)
- **Small files**: Max 8KB per file (8 blocks × 512 bytes)
- **Limited processes**: Max 256 processes
- **No networking**: No network stack
- **No ACPI**: Uses legacy PIC instead of APIC

### Planned Features (Not Yet Implemented)
- ❌ Self-rewriting kernel
- ❌ Parallel-reality execution (only tracking)
- ❌ Multiverse OS layer merging
- ❌ Fractal memory (spatial allocation)
- ❌ Psychic filesystem prediction (only tracking)
- ❌ Dream-generated UI
- ❌ Contextual runtime evolution
- ❌ Capability-based security
- ❌ AstralScript language

---

## 🔬 Technical Details

### Interrupt Handling
- **IDT**: 256 entries, properly configured for x86_64
- **Exceptions**: 0-31 CPU exceptions with error codes
- **IRQs**: 32-47 hardware interrupts via PIC
- **Syscalls**: INT 0x80 for user→kernel transitions
- **Wrappers**: Naked assembly preserves all registers

### VirtIO Implementation
- **Specification**: VirtIO 1.0 modern device
- **Virtqueue**: 128 descriptors, split layout
- **DMA**: Direct memory access with proper barriers
- **Notifications**: MMIO-based queue notifications
- **PCI**: Full PCI configuration space parsing

### Context Switching
- **Naked Functions**: Direct assembly for zero overhead
- **Register Preservation**: All GP registers + flags
- **Page Table Switching**: CR3 reload on context switch
- **Stack Management**: Separate kernel stacks per process

---

## 📁 Project Structure

```
astral-os/
├── kernel/
│   ├── src/
│   │   ├── main.rs              # Entry point
│   │   ├── lib.rs               # Kernel library
│   │   ├── util.rs              # Utility functions
│   │   ├── memory/
│   │   │   ├── mod.rs
│   │   │   ├── frame.rs         # Physical allocator
│   │   │   ├── paging.rs        # Page tables
│   │   │   └── heap.rs          # Kernel heap
│   │   ├── process/
│   │   │   ├── mod.rs
│   │   │   ├── scheduler.rs     # Scheduler
│   │   │   └── context.rs       # Context switching
│   │   ├── interrupts/
│   │   │   ├── mod.rs
│   │   │   ├── idt.rs           # IDT setup
│   │   │   ├── pic.rs           # PIC driver
│   │   │   └── handlers.rs      # Exception/IRQ handlers
│   │   ├── drivers/
│   │   │   ├── mod.rs
│   │   │   ├── framebuffer.rs   # Fontdue rendering
│   │   │   ├── serial.rs        # COM1 debug
│   │   │   ├── keyboard.rs      # PS/2 keyboard
│   │   │   └── virtio/
│   │   │       ├── mod.rs
│   │   │       ├── pci.rs       # PCI enumeration
│   │   │       ├── queue.rs     # Virtqueue
│   │   │       └── block.rs     # Block device
│   │   ├── fs/
│   │   │   ├── mod.rs
│   │   │   └── psychicfs.rs     # Filesystem
│   │   ├── reality/
│   │   │   ├── mod.rs
│   │   │   ├── causality.rs     # Causal tracking
│   │   │   ├── dream.rs         # Dream state
│   │   │   └── intent.rs        # Intent syscalls
│   │   └── shell/
│   │       ├── mod.rs
│   │       └── commands.rs      # Shell commands
│   ├── Cargo.toml
│   ├── build.rs
│   ├── x86_64-astral.json
│   ├── linker.ld
│   └── SpaceMono-Regular.ttf    # Required font
├── Makefile
├── limine.conf
└── README.md
```

---

## 🤝 Contributing

Contributions welcome! Areas needing work:

1. **SMP Support**: Multi-core processing
2. **APIC/ACPI**: Modern interrupt handling
3. **Userspace**: Ring 3 execution
4. **Network Stack**: TCP/IP implementation
5. **More Filesystems**: ext2, FAT32
6. **GUI**: Dream-interface rendering
7. **AstralScript**: Custom scripting language

---

## 📜 License

MIT License - see [LICENSE](LICENSE) file for details.

---

## 🙏 Acknowledgments

- **Limine Bootloader**: Modern, spec-compliant bootloader
- **OSDev Community**: Invaluable documentation and support
- **Fontdue**: Pure-Rust TrueType rasterization
- **Rust Team**: Amazing language and tooling

---

## 📞 Contact

**Project**: Astral OS  
**Version**: 0.3.0  
**Architecture**: x86_64  
**Bootloader**: Limine v8.x

For questions, issues, or contributions, please open an issue on GitHub.

---
**Note: This is the modular version of Astral OS. The single-file “godfile” version is available in the Godfile-Version branch.**
**Note: This OS is a hybrid one so it has some normal features as well as complex and bootable in BIOS & UEFI 64 and about RISC-V is not yet made for it**
**Built with 🦀 Rust and ☕ caffeine**
