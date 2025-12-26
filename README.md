# Astral OS

**A Revolutionary Self-Assembling Operating System**

## 🌟 Vision

Astral OS is an ambitious operating system featuring:
- 🔄 **Self-Assembling Kernel** - Reassembles differently per boot
- ✍️ **Self-Rewriting Kernel** - Real-time code evolution
- 🌌 **Parallel-Reality Execution** - Timeline branching and merging
- 🧠 **Fractal Memory Architecture** - Infinite recursive memory model
- 🔮 **Psychic Filesystem** - Predictive, self-reorganizing storage
- 💭 **Dream-State OS Mode** - Idle optimization and evolution
- 🎨 **Dream-Interface UI** - Emotionally adaptive interface
- 🔐 **Evolving Security** - Behavior-based permission adaptation

## 🏗️ Current Status

**Phase 0-1: Foundation Complete** ✅
- Rust no_std kernel environment
- Limine bootloader integration
- x86_64 bootstrap (GDT, IDT, paging)
- Serial and framebuffer output
- Basic memory management with heap allocator

## 🛠️ Building

### Prerequisites
- Rust (nightly)
- `xorriso` (for ISO creation)
- `qemu-system-x86_64` (for testing)
- `git` (for Limine download)

### Build Commands

```bash
# Build the kernel
make build

# Create bootable ISO
make iso

# Run in QEMU
make run

# Run in QEMU with UEFI
make run-uefi

# Clean build artifacts
make clean
```

## 🚀 Quick Start

```bash
# Clone and build
git clone <repo-url>
cd Astral
make run
```

## 📁 Project Structure

```
Astral/
├── kernel/               # Main kernel crate
│   └── src/
│       ├── main.rs      # Kernel entry point
│       ├── arch/        # Architecture-specific code
│       ├── memory/      # Memory management
│       └── output/      # Serial and framebuffer drivers
├── linker.ld            # Linker script
├── limine.conf          # Bootloader configuration
├── Makefile             # Build system
└── README.md            # This file
```

## 📚 Documentation

See the `docs/` directory and artifact files:
- [Implementation Plan](docs/implementation_plan.md)
- [Task List](docs/task.md)
- [OSDev Reference](docs/osdev_reference.md)

## 🎯 Development Roadmap

- [x] Phase 0: Foundation & Build System
- [x] Phase 1: Core Kernel Bootstrap
- [ ] Phase 2: Memory Management Foundation
- [ ] Phase 3: Interrupt & Exception Handling
- [ ] Phase 4: Process & Scheduling
- [ ] Phase 5+: Advanced Features (Reality Branching, Fractal Memory, etc.)

## 🤝 Contributing

This is currently a learning/experimental project. Contributions and ideas welcome!

## 📝 License

TBD

## 🙏 Acknowledgments

- [OSDev Wiki](https://wiki.osdev.org/) - Essential OS development resource
- [Phil Opp's Blog](https://os.phil-opp.com/) - "Writing an OS in Rust"
- [Limine Bootloader](https://github.com/limine-bootloader/limine) - Modern bootloader

---

**Astral OS** - Where reality meets code ✨
