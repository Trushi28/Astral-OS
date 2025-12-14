# Astral OS v0.3.1

**A reality-aware operating system with causal tracking, dream states, SMP support, and intent-based computing.**

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
- **Slab Allocator**: O(1) allocation with 8 size classes (32B-4KB)

### Process Management
- **AstralScheduler**: O(1) priority-based scheduler with 5 priority classes
- **Intent-Aware Scheduling**: Processes declare intent (Graphics, IO, Compute, etc.)
- **Dream-Mode Optimization**: Background tasks run during system idle
- **SMP Support**: Multi-core processing with per-CPU run queues
- **Full Context Switching**: Complete register preservation including FPU state
- **Process Isolation**: Per-process page tables with kernel/user separation

### Filesystem
- **Virtual Filesystem (VFS)**: Unified interface for all filesystem operations
- **PsychicFS**: Predictive filesystem with block cache and access pattern learning
- **Reality-Aware Storage**: Files tracked across timeline branches
- **Path Caching**: Fast path resolution for improved performance

### Graphics
- **Double-Buffered Compositor**: Tear-free rendering with damage tracking
- **Astral Display Server**: Wayland-inspired display server architecture
- **Window Management**: Server-side decorations and compositing
- **Alpha Blending**: Optimized Porter-Duff compositing

### Synchronization
- **Spinlock & Mutex**: IRQ-safe locking primitives
- **RwLock**: Reader-writer locks for concurrent access
- **Semaphore**: Counting semaphores with bounded support
- **Condvar & Barrier**: Thread coordination primitives

### Drivers
- **Framebuffer**: TrueType font rendering via `noto-sans-mono-bitmap`
- **VirtIO Block**: Full modern VirtIO 1.0 implementation with proper queue management
- **Serial Port**: COM1 debugging output
- **PS/2 Keyboard**: Full scancode translation with modifier support
- **APIC/IOAPIC**: Modern interrupt handling with SMP support

---

## 🏗️ Architecture

```
Astral OS
├── Memory Layer
│   ├── Physical Frame Allocator (lock-free bitmap)
│   ├── Page Table Manager (4-level paging)
│   ├── Kernel Heap (linked-list allocator)
│   └── Slab Allocator (8 size classes)
├── Process Layer
│   ├── Process Control Blocks
│   ├── AstralScheduler (O(1) priority-based)
│   ├── Per-CPU Run Queues (SMP)
│   └── Context Switching (assembly)
├── Interrupt Layer
│   ├── IDT with 256 entries
│   ├── GDT & TSS (proper ring transitions)
│   ├── APIC & IOAPIC
│   └── Exception/IRQ handlers
├── Synchronization Layer
│   ├── Spinlock, Mutex, RwLock
│   ├── Semaphore, Condvar, Barrier
│   └── Once (one-time initialization)
├── Driver Layer
│   ├── Framebuffer (noto-sans-mono-bitmap)
│   ├── VirtIO Block (modern spec)
│   ├── Serial (COM1 debug)
│   └── Keyboard (PS/2)
├── Filesystem Layer
│   ├── Virtual Filesystem (VFS)
│   └── PsychicFS (predictive FS)
├── Graphics Layer
│   ├── Compositor (double-buffered)
│   ├── Astral Display Server
│   └── Surface Management
├── Reality Engine
│   ├── Causality Tracker
│   ├── Dream State Manager
│   └── Intent Resolver
├── GUI Layer
│   ├── Desktop Environment
│   └── Window Manager
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

#### GUI Commands
```bash
gui                     # Launch desktop environment
desktop                 # Same as gui
graphics                # Graphics system demo
```

#### Filesystem Commands
```bash
format                  # Format PsychicFS (WARNING: erases data)
mount                   # Mount filesystem
sync                    # Sync filesystem to disk
ls                      # List files
cat <filename>          # Display file contents
write <file> <text>     # Write text to file
touch <filename>        # Create empty file
rm <filename>           # Delete file
```

#### SMP Commands
```bash
smp                     # Show SMP status and per-CPU info
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
- **Partial userspace**: Syscall-based shell (Ring 0 with syscall API)
- **Limited processes**: Max 256 processes
- **Basic networking**: TCP state machine stub only

### Recently Implemented ✅
- ✅ **SMP Support** - Multi-core processing with per-CPU scheduling
- ✅ **APIC/IOAPIC** - Modern interrupt handling
- ✅ **Slab Allocator** - O(1) kernel allocations
- ✅ **VFS Layer** - Unified filesystem interface
- ✅ **AstralScheduler** - O(1) priority scheduling
- ✅ **Double-buffered Compositor** - Tear-free rendering
- ✅ **Synchronization Primitives** - Semaphore, Condvar, Barrier, Once
- ✅ **Desktop GUI Environment** - Window manager with mouse support
- ✅ **Syscall-based User Shell** - Shell using syscall API for I/O
- ✅ **Real System Info** - mem, ps, cpu show live kernel data

### Planned Features (Not Yet Implemented)
- ⏳ True Ring 3 userspace (ELF loader)
- ❌ Full TCP/IP network stack
- ❌ Self-rewriting kernel
- ❌ Parallel-reality execution (only tracking)
- ❌ Dream-generated UI
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

### Context Switching & Scheduling
- **AstralScheduler**: O(1) dispatch using priority bitmap
- **5 Priority Classes**: RealTime, Interactive, Normal, Background, Dream
- **Intent-Aware**: Processes declare intent for scheduling optimization
- **Per-CPU Queues**: Scalable scheduling for SMP
- **Page Table Switching**: CR3 reload on context switch
- **Stack Management**: Separate kernel stacks per process

---

## 📁 Project Structure

```
Astral-OS/
├── astral.iso
├── kernel
│   ├── build.rs
│   ├── Cargo.lock
│   ├── Cargo.toml
│   ├── linker.ld
│   ├── src
│   │   ├── acpi
│   │   │   ├── madt.rs
│   │   │   └── mod.rs
│   │   ├── apps
│   │   │   ├── editor.rs
│   │   │   └── mod.rs
│   │   ├── arch
│   │   │   ├── mod.rs
│   │   │   └── x86_64
│   │   │       ├── apic.rs
│   │   │       ├── ap_trampoline.s
│   │   │       ├── cpu.rs
│   │   │       ├── gdt.rs
│   │   │       ├── ioapic.rs
│   │   │       ├── mod.rs
│   │   │       ├── smp.rs
│   │   │       ├── syscall.rs
│   │   │       ├── tss.rs
│   │   │       └── usermode.rs
│   │   ├── auth
│   │   │   ├── login.rs
│   │   │   ├── mod.rs
│   │   │   ├── session.rs
│   │   │   └── users.rs
│   │   ├── boot
│   │   │   ├── menu.rs
│   │   │   └── mod.rs
│   │   ├── display
│   │   │   ├── cursor.rs
│   │   │   ├── mod.rs
│   │   │   └── renderer.rs
│   │   ├── drivers
│   │   │   ├── framebuffer.rs
│   │   │   ├── keyboard.rs
│   │   │   ├── mod.rs
│   │   │   ├── mouse.rs
│   │   │   ├── rtc.rs
│   │   │   ├── serial.rs
│   │   │   └── virtio
│   │   │       ├── block.rs
│   │   │       ├── mod.rs
│   │   │       ├── net.rs
│   │   │       ├── pci.rs
│   │   │       └── queue.rs
│   │   ├── fs
│   │   │   ├── mod.rs
│   │   │   ├── psychicfs.rs
│   │   │   └── vfs.rs
│   │   ├── graphics
│   │   │   ├── compositor.rs
│   │   │   ├── mod.rs
│   │   │   ├── protocol.rs
│   │   │   └── surface.rs
│   │   ├── gui
│   │   │   ├── desktop.rs
│   │   │   ├── font.rs
│   │   │   ├── mod.rs
│   │   │   ├── theme.rs
│   │   │   └── window.rs
│   │   ├── interrupts
│   │   │   ├── handlers.rs
│   │   │   ├── idt.rs
│   │   │   ├── mod.rs
│   │   │   └── pic.rs
│   │   ├── ipc
│   │   │   └── mod.rs
│   │   ├── lib.rs
│   │   ├── main.rs
│   │   ├── memory
│   │   │   ├── fractal.rs
│   │   │   ├── frame.rs
│   │   │   ├── heap.rs
│   │   │   ├── mod.rs
│   │   │   ├── paging.rs
│   │   │   └── slab.rs
│   │   ├── network
│   │   │   ├── arp.rs
│   │   │   ├── device.rs
│   │   │   ├── ethernet.rs
│   │   │   ├── ip.rs
│   │   │   ├── mod.rs
│   │   │   ├── socket.rs
│   │   │   ├── tcp.rs
│   │   │   └── udp.rs
│   │   ├── power
│   │   │   └── mod.rs
│   │   ├── process
│   │   │   ├── context.rs
│   │   │   ├── mod.rs
│   │   │   └── scheduler.rs
│   │   ├── reality
│   │   │   ├── causality.rs
│   │   │   ├── dream.rs
│   │   │   ├── intent.rs
│   │   │   └── mod.rs
│   │   ├── security
│   │   │   ├── capability.rs
│   │   │   ├── intent.rs
│   │   │   ├── mod.rs
│   │   │   └── sandbox.rs
│   │   ├── shell
│   │   │   ├── commands.rs
│   │   │   └── mod.rs
│   │   ├── sync
│   │   │   └── mod.rs
│   │   ├── usermode
│   │   │   ├── elf.rs
│   │   │   ├── launcher.rs
│   │   │   ├── loader.rs
│   │   │   ├── mod.rs
│   │   │   ├── shell.rs
│   │   │   ├── syscall.rs
│   │   │   ├── test.rs
│   │   │   └── usys.rs
│   │   └── util.rs
│   └── x86_64-astral.json
├── limine.conf
├── Makefile
└── Readme.md
```

---

## 🤝 Contributing

Contributions welcome! Areas needing work:

1. **True Ring 3 Userspace** - ELF loader for user binaries
2. **Network Stack** - TCP/IP implementation
3. **More Filesystems** - ext2, FAT32 support
4. **GUI Improvements** - More applications, themes
5. **AstralScript** - Custom scripting language

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
**Version**: 0.3.1  
**Architecture**: x86_64 (SMP)  
**Bootloader**: Limine v8.x

For questions, issues, or contributions, please open an issue on GitHub.

---
**Note: This is the modular version of Astral OS. The single-file “godfile” version is available in the Godfile-Version branch.**

**Note: This OS is a hybrid one so it has some normal features as well as complex and bootable in BIOS & UEFI 64 and about RISC-V is not yet made for it**

**Built with 🦀 Rust and ☕ caffeine**
