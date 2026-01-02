# Astral OS - Complete Development Roadmap

> **Status**: Phase 0-5 complete. Ready for usermode transition.
> 
> **Critical Path**: Phase 4.5 → 4.6 → 4.7 (all prerequisites for usermode)

---

## ✅ COMPLETED PHASES

### Phase 0-3: Core Infrastructure ✅
- [x] Boot (Limine bootloader)
- [x] Serial output
- [x] Framebuffer graphics
- [x] Physical memory management (Buddy allocator)
- [x] Virtual memory (basic paging, HHDM)
- [x] Heap allocator
- [x] Interrupts (IDT, PIC, APIC)
- [x] Timer (PIT, APIC timer)
- [x] Keyboard input
- [x] SMP (multicore support)

### Phase 4: Process & Scheduling ✅
- [x] Process structure (TCB)
- [x] Context switching (inline assembly)
- [x] Work-stealing scheduler
- [x] Kernel thread spawning
- [x] Timer preemption
- [x] Syscall interface (clone, exit, write, etc.)
- [x] Stack pool management
- [x] Process cleanup

### Phase 5: Fractal Memory ✅
- [x] Fractal regions (hierarchical)
- [x] Spatial allocator
- [x] Integration with buddy allocator
- [x] Hybrid allocation interface

---

## 🚧 CRITICAL PATH TO USERMODE

These phases MUST be done in order - each depends on the previous:

### Phase 4.5: Virtual Memory Isolation 🔒

**WHY**: Without this, applications run in kernel space - unsafe!

- [ ] **GDT User Segments**
  - [ ] Add Ring 3 code segment (DPL=3)
  - [ ] Add Ring 3 data segment (DPL=3)
  - [ ] Update GDT reload in boot

- [ ] **TSS (Task State Segment)**
  - [ ] Create TSS structure
  - [ ] Set up kernel stack pointer (RSP0)
  - [ ] Load TSS into GDT
  - [ ] Configure for interrupt stack switching

- [ ] **Per-Process Page Tables**
  - [ ] Allocate PML4 per process
  - [ ] Clone kernel mappings to upper half
  - [ ] Store CR3 in Process struct
  - [ ] Switch CR3 on context switch

- [ ] **Address Space Layout**
  ```
  User:   0x0000_0000_0000_0000 - 0x0000_7FFF_FFFF_FFFF
  Kernel: 0xFFFF_8000_0000_0000 - 0xFFFF_FFFF_FFFF_FFFF
  ```
  - [ ] Map kernel to upper half in all page tables
  - [ ] Reserve user space in lower half
  - [ ] Set USER_ACCESSIBLE bit for user pages

- [ ] **Memory Protection**
  - [ ] Guard pages (unmapped for stack overflow)
  - [ ] Read-only code pages (.text)
  - [ ] NX (No-Execute) for data pages
  - [ ] SMEP/SMAP if available

- [ ] **Page Fault Handler**
  - [ ] Parse CR2 (fault address)
  - [ ] Parse error code (present, protection, user/kernel)
  - [ ] Demand paging for heap
  - [ ] Stack growth
  - [ ] Guard page detection → panic
  - [ ] Segfault handling

- [ ] **SYSRET/IRETQ Logic**
  - [ ] MSR configuration (IA32_STAR, IA32_LSTAR, IA32_FMASK)
  - [ ] Syscall entry point (save user context)
  - [ ] Sysret exit (restore user context)
  - [ ] iretq for interrupts from userspace
  - [ ] Kernel stack per CPU for syscalls

**Deliverable**: Can spawn process with isolated virtual memory

---

### Phase 4.6: ELF Loader & Applications 📦

**WHY**: OS is useless without programs to run!

- [ ] **ELF Parser**
  - [ ] Verify ELF magic (0x7F 'E' 'L' 'F')
  - [ ] Parse program headers
  - [ ] Find loadable segments (PT_LOAD)
  - [ ] Read entry point

- [ ] **ELF Loader**
  - [ ] Create process with page table
  - [ ] Map segments to user virtual memory:
    - [ ] .text (code) → Read + Execute
    - [ ] .data (initialized data) → Read + Write + NX
    - [ ] .rodata (read-only data) → Read + NX
    - [ ] .bss (zero-initialized) → Read + Write + NX
  - [ ] Allocate user stack with guard page
  - [ ] Set up initial stack (argc, argv, envp)

- [ ] **Ring 3 Transition**
  - [ ] Push user SS, RSP, RFLAGS, CS, RIP
  - [ ] iretq to jump to userspace
  - [ ] OR: Use sysret if coming from syscall

- [ ] **InitramFS**
  - [ ] Parse TAR format
  - [ ] Extract to ramdisk on boot
  - [ ] Simple file lookup by path

- [ ] **Userspace Build System**
  - [ ] Create `userspace/` crate
  - [ ] Compile to freestanding ELF (`no_std`)
  - [ ] Link script for userspace layout
  - [ ] Bundle apps into TAR, embed in kernel

- [ ] **Core Applications**
  - [ ] `init` - PID 1, spawns shell
  - [ ] `shell` - Simple command line
  - [ ] `echo` - Test program
  - [ ] `ls` - List files (once VFS works)

**Deliverable**: Can load and execute ELF binaries in Ring 3

---

### Phase 4.7: System Plumbing & Abstractions 🔧

**WHY**: Connect everything together - make subsystems talk to each other

- [ ] **VFS (Virtual File System)**
  - [ ] Traits:
    ```rust
    trait File { fn read(), write(), seek() }
    trait Directory { fn list(), lookup() }
    trait Vnode { fn stat(), chmod() }
    trait Filesystem { fn mount(), unmount() }
    ```
  - [ ] File descriptor table per process
  - [ ] Path resolution ("/dev/tty")
  - [ ] Syscalls: open, read, write, close, lseek

- [ ] **DevFS (Device Filesystem)**
  - [ ] `/dev/null` - Discard writes, return EOF on read
  - [ ] `/dev/zero` - Infinite zeros
  - [ ] `/dev/tty` - Current terminal
  - [ ] `/dev/urandom` - Random bytes (simple RNG)

- [ ] **TTY Subsystem**
  - [ ] TTY state machine (ANSI escape codes)
  - [ ] Cell buffer (character + color)
  - [ ] Render to framebuffer
  - [ ] Input buffering (canonical mode)
  - [ ] Line discipline (backspace, ctrl-C, etc.)

- [ ] **IPC Primitives**
  - [ ] Message passing (send/recv syscalls)
  - [ ] Ports/channels for inter-process communication
  - [ ] Example: Shell → Display manager

- [ ] **PCIe Bus Enumeration**
  - [ ] Scan bus 0, enumerate devices
  - [ ] Read vendor ID, device ID, class code
  - [ ] Log discovered devices
  - [ ] Simple driver matching (for future)

**Deliverable**: Cohesive system where apps can use files, devices, TTY

---

### Phase 4.8: Storage & Persistent Filesystem 💾

**WHY**: Need to save files permanently, not just in RAM

- [ ] **Block Device Layer**
  - [ ] BlockDevice trait (read_block, write_block)
  - [ ] ATA PIO driver (simple, works on all hardware)
  - [ ] IDE controller detection
  - [ ] Read/write sectors

- [ ] **Partition Table**
  - [ ] MBR parsing
  - [ ] GPT parsing (GUID Partition Table)
  - [ ] Enumerate partitions

- [ ] **Filesystem: TarFS (Read-Only)**
  - [ ] Parse TAR headers
  - [ ] File lookup by path
  - [ ] Read file contents
  - [ ] Use for initramfs (already embedded)

- [ ] **Filesystem: ext2 (Read-Write)**
  - [ ] Superblock parsing
  - [ ] Inode reading/writing
  - [ ] Directory traversal
  - [ ] File create/delete/modify
  - [ ] Block allocation bitmap

- [ ] **VFS Integration**
  - [ ] Mount filesystem to path
  - [ ] Root filesystem (/)
  - [ ] Multiple mount points
  - [ ] File operations route to correct FS

**Deliverable**: Can save files to disk, persist across reboots

---

### Phase 4.9: Essential Drivers 🔌

**WHY**: Hardware interaction beyond basics

- [ ] **Keyboard Driver (Enhanced)**
  - [ ] Scancode set 2/3 support
  - [ ] Modifier keys (shift, ctrl, alt)
  - [ ] Keyboard layouts (US, intl)

- [ ] **Mouse/Touchpad**
  - [ ] PS/2 mouse driver
  - [ ] USB mouse (via USB stack)
  - [ ] Cursor position tracking

- [ ] **RTC (Real-Time Clock)**
  - [ ] Read current date/time
  - [ ] CMOS/ACPI time
  - [ ] Timezone support

- [ ] **Disk (Enhanced)**
  - [ ] AHCI driver (modern SATA)
  - [ ] DMA transfers
  - [ ] Async I/O

- [ ] **USB Stack (Basic)**
  - [ ] xHCI host controller
  - [ ] USB device enumeration
  - [ ] HID (keyboard, mouse)
  - [ ] Mass storage (USB drives)

**Deliverable**: Full hardware support for common devices

---

### Phase 5.5: Networking 🌐

**WHY**: Network connectivity is essential for modern OS

- [ ] **Network Card Drivers**
  - [ ] RTL8139 (simple, common in VMs)
  - [ ] Intel e1000 (widely supported)
  - [ ] Virtio-net (for QEMU/KVM)

- [ ] **Network Stack**
  - [ ] Ethernet frames
  - [ ] ARP (Address Resolution Protocol)
  - [ ] IP (Internet Protocol)
  - [ ] ICMP (ping)
  - [ ] UDP (User Datagram Protocol)
  - [ ] TCP (Transmission Control Protocol)

- [ ] **Socket API**
  - [ ] socket(), bind(), listen(), accept()
  - [ ] connect(), send(), recv()
  - [ ] File descriptor integration

- [ ] **Network Applications**
  - [ ] ping utility
  - [ ] Simple HTTP client/server
  - [ ] DNS resolver

**Deliverable**: Can ping, browse web (basic), network communication

---

### Phase 5.6: Power Management & ACPI ⚡

**WHY**: Proper shutdown, reboot, power states

- [ ] **ACPI Tables**
  - [ ] Parse RSDP (Root System Description Pointer)
  - [ ] Parse RSDT/XSDT
  - [ ] Parse FADT (Fixed ACPI Description Table)
  - [ ] Parse MADT (already done for APIC)

- [ ] **Power States**
  - [ ] Shutdown (ACPI method)
  - [ ] Reboot (keyboard controller or ACPI)
  - [ ] Sleep/suspend (S3 state)
  - [ ] Hibernate (S4 state)

- [ ] **CPU Power Management**
  - [ ] Halt instruction (already used)
  - [ ] C-states (idle power saving)
  - [ ] P-states (frequency scaling)

- [ ] **Syscalls**
  - [ ] shutdown(flags)
  - [ ] reboot(flags)

**Deliverable**: Can shutdown/reboot properly, not just halt

---

### Phase 6: Self-Assembling Kernel 🧩

**WHY**: Modular architecture, dynamic loading

- [ ] **Module System**
  - [ ] Kernel module format (.ko files)
  - [ ] ELF relocatable objects
  - [ ] Symbol resolution
  - [ ] Module dependencies

- [ ] **Dynamic Loading**
  - [ ] Load modules at runtime
  - [ ] Unload modules
  - [ ] Module parameters

- [ ] **Driver Framework**
  - [ ] Device driver interface
  - [ ] Hot-plug support
  - [ ] Driver matching (PCI ID, USB ID)

**Deliverable**: Modular kernel, can load drivers on-demand

---

### Phase 7: Reality Branching (Copy-on-Write) 🌿

**WHY**: Efficient process forking, snapshots

- [ ] **COW Implementation**
  - [ ] Mark pages read-only on fork
  - [ ] Page fault on write → copy page
  - [ ] Reference counting for shared pages

- [ ] **Fork System Call**
  - [ ] Clone parent page table
  - [ ] Share physical pages (COW)
  - [ ] Clone open files
  - [ ] Return 0 in child, PID in parent

- [ ] **Snapshots**
  - [ ] Save process state
  - [ ] Checkpoint/restore
  - [ ] Timeline branching

**Deliverable**: Efficient fork(), process snapshots

---

### Phase 8: Advanced Features 🚀

**WHY**: Polish and performance

- [ ] **Graphics**
  - [ ] Framebuffer optimization
  - [ ] Hardware acceleration (GPU drivers)
  - [ ] Window system

- [ ] **Audio**
  - [ ] AC'97 or HDA driver
  - [ ] Sound API
  - [ ] Mixer

- [ ] **Testing & Debugging**
  - [ ] Unit tests (kernel, userspace)
  - [ ] Integration tests
  - [ ] GDB stub (kernel debugging)
  - [ ] QEMU gdbstub integration

- [ ] **Performance**
  - [ ] Profiling tools
  - [ ] Benchmarking
  - [ ] Optimization passes

- [ ] **Security**
  - [ ] Capabilities
  - [ ] Secure boot
  - [ ] Sandboxing

---

## 📋 FULL DEPENDENCY GRAPH (Updated)

```
Phase 0-3 (Boot, Memory, Interrupts) ✅
    ↓
Phase 4 (Scheduler, Syscalls) ✅
    ↓
Phase 5 (Fractal Memory) ✅
    ↓
┌───────────────────────────────────────────┐
│ CRITICAL PATH (MUST DO IN ORDER)         │
├───────────────────────────────────────────┤
│ Phase 4.5: VM Isolation                   │ ← START HERE
│   ↓                                       │
│ Phase 4.6: ELF Loader + Apps              │
│   ↓                                       │
│ Phase 4.7: VFS, TTY, IPC, PCIe            │
│   ↓                                       │
│ Phase 4.8: Storage & Filesystem           │
│   ↓                                       │
│ Phase 4.9: Essential Drivers              │
└───────────────────────────────────────────┘
    ↓
┌───────────────────────────────────────────┐
│ PARALLEL (Can do in any order)            │
├───────────────────────────────────────────┤
│ Phase 5.5: Networking                     │
│ Phase 5.6: Power Management               │
└───────────────────────────────────────────┘
    ↓
Phase 6: Self-Assembling Kernel
    ↓
Phase 7: Reality Branching (COW)
    ↓
Phase 8: Advanced Features
```

---

## 🎯 PRIORITY LEVELS

### P0 - CRITICAL (Can't use OS without these)
- ✅ Phase 0-5 (Done!)
- 🚧 Phase 4.5: VM Isolation
- 🚧 Phase 4.6: ELF Loader
- 🚧 Phase 4.7: VFS + TTY
- 🚧 Phase 4.8: Filesystem

### P1 - HIGH (Needed for practical use)
- Phase 4.9: Drivers
- Phase 5.6: Power Management (shutdown/reboot)

### P2 - MEDIUM (Nice to have)
- Phase 5.5: Networking
- Phase 6: Module system
- Phase 7: COW/Fork

### P3 - LOW (Future enhancements)  
- Phase 8: Graphics, Audio, Advanced features

---

## ⚠️ COMMON MISTAKES TO AVOID

1. **Don't skip Phase 4.5** - Everything depends on it
2. **Don't write userspace apps before Phase 4.6** - No loader!
3. **Don't implement network before filesystem** - Need disk for logs/config
4. **Don't forget ACPI shutdown** - Users hate pulling the power plug
5. **Test each phase before moving on** - Bugs compound quickly

---

## ✅ DEFINITION OF "DONE" FOR EACH PHASE

### Phase 4.5 Done =
- [ ] Can execute `iretq` to Ring 3
- [ ] Process has own CR3
- [ ] Page fault doesn't panic
- [ ] User can't access kernel memory

### Phase 4.6 Done =
- [ ] Can load `hello_world.elf`
- [ ] Program runs and prints "Hello"
- [ ] Program exits cleanly
- [ ] Can load multiple different programs

### Phase 4.7 Done =
- [ ] Can `cat /dev/null`
- [ ] Shell reads from `/dev/tty`
- [ ] Can send message between processes

### Phase 4.8 Done =
- [ ] Can write file to disk
- [ ] File persists after reboot
- [ ] Can `ls`, `cat`, `echo > file`

### Phase 4.9 Done =
- [ ] Keyboard works with all keys
- [ ] Mouse moves cursor
- [ ] Can read from USB drive
- [ ] RTC shows correct time

---

## 📊 ESTIMATED TIMELINE

| Phase | Estimated Time | Cumulative |
|-------|---------------|------------|
| 4.5 VM Isolation | 10-12 hours | 10-12 hours |
| 4.6 ELF Loader | 8-10 hours | 18-22 hours |
| 4.7 System Plumbing | 12-15 hours | 30-37 hours |
| 4.8 Filesystem | 10-12 hours | 40-49 hours |
| 4.9 Drivers | 15-20 hours | 55-69 hours |
| 5.5 Networking | 20-25 hours | 75-94 hours |
| 5.6 Power Mgmt | 5-8 hours | 80-102 hours |

**Total to working OS**: ~80-100 hours (2-3 weeks of focused work)

---

## 🚀 RAPID DEVELOPMENT TIPS

Since you mentioned rapid development speed:

1. **Follow the order strictly** - Don't jump around
2. **Test incrementally** - Small changes, frequent testing
3. **Use QEMU snapshots** - Save state, quick rollback
4. **Keep serial logging** - Debugging without graphics
5. **Write tests as you go** - Catch regressions early
6. **Document as you build** - Comments in code
7. **Commit often** - Git commits after each feature

---

**This roadmap is now COMPLETE - all critical sections included!** 🎯

Start at Phase 4.5 and work through sequentially. Everything is dependency-ordered and estimated.

```
Phase 0-3 (Boot, Memory, Interrupts) ✅
    ↓
Phase 4 (Scheduler, Syscalls) ✅
    ↓
Phase 5 (Fractal Memory) ✅
    ↓
┌───────────────────────────────────┐
│ CRITICAL PATH TO USERMODE         │
├───────────────────────────────────┤
│ Phase 4.5: VM Isolation           │ ← YOU ARE HERE
│   ↓                               │
│ Phase 4.6: ELF Loader + Apps      │
│   ↓                               │
│ Phase 4.7: VFS, TTY, IPC, PCIe    │
└───────────────────────────────────┘
    ↓
Phase 6: Self-Assembling Kernel (future)
    ↓
Phase 7: Reality Branching (future)
```

---

## 🎯 RECOMMENDED ORDER

**Start here → Finish there:**

1. **GDT User Segments + TSS** (Phase 4.5)
   - Required for Ring 3 transition
   - ~2-3 hours

2. **Per-Process Page Tables** (Phase 4.5)
   - Required for memory isolation
   - ~3-4 hours

3. **Page Fault Handler** (Phase 4.5)
   - Required for safety
   - ~2-3 hours

4. **SYSRET/IRETQ** (Phase 4.5)
   - Required for syscalls from userspace
   - ~2 hours

5. **ELF Loader** (Phase 4.6)
   - Can now load programs!
   - ~3-4 hours

6. **Simple Userspace App** (Phase 4.6)
   - Test everything works
   - ~1-2 hours

7. **VFS + DevFS** (Phase 4.7)
   - Proper file abstraction
   - ~4-5 hours

8. **TTY Subsystem** (Phase 4.7)
   - Proper terminal
   - ~3-4 hours

**Total estimated time**: 20-30 hours for complete usermode support

---

## 🔍 WHAT'S ACTUALLY MISSING (Detailed)

### From Current Code:

#### GDT (src/arch/x86_64/gdt.rs)
- ✅ Has kernel code/data segments
- ❌ Missing user code segment (DPL=3)
- ❌ Missing user data segment (DPL=3)
- ❌ Missing TSS segment

#### Process (src/process/mod.rs)
- ✅ Has basic TCB structure
- ❌ page_table is Option<u64> but never actually used!
- ❌ No memory regions tracking
- ❌ No file descriptor table

#### Syscalls (src/syscall.rs)
- ✅ Has basic interface
- ❌ All stubs - no real functionality
- ❌ No userspace validation
- ❌ No sysret/iretq implementation

#### Memory (src/memory/)
- ✅ Has buddy allocator
- ✅ Has basic paging module
- ❌ No per-process page table management
- ❌ No page fault handler
- ❌ No demand paging or COW

---

## 💡 KEY INSIGHTS

### Why This Order?

**Phase 4.5 MUST come first** because:
- Can't load ELF safely without page tables
- Can't run apps safely without memory protection
- Can't use syscalls without SYSRET/IRETQ

**Phase 4.6 comes next** because:
- Need to load programs to test Phase 4.5
- Applications validate that isolation works

**Phase 4.7 comes last** because:
- VFS needs running apps to be useful
- TTY needs apps that can write to it
- IPC needs multiple apps

### Common Pitfalls to Avoid:

1. ❌ **Don't** try to write apps before Phase 4.5
   - They'll crash without proper page tables

2. ❌ **Don't** skip TSS
   - Needed for interrupt stack switching from Ring 3

3. ❌ **Don't** forget to update Context in switch.rs
   - Need to save/restore user segment registers

4. ❌ **Don't** map kernel RW in user page tables
   - Security hole - kernel should be inaccessible

---

## 📝 NEXT STEPS

**Immediate action:**

1. Read Phase 4.5 plan (saved as artifact)
2. Start with GDT user segments
3. Then TSS
4. Then per-process page tables
5. Test with simple Ring 3 transition

**When stuck:**
- Check this TASK.md for dependencies
- Each phase has detailed plan in artifacts/
- Phases MUST be done in order

---

## ✅ SUCCESS CRITERIA

**You'll know Phase 4.5 is done when:**
- Process has own page table (CR3)
- Can switch to Ring 3 and back
- Page faults are handled gracefully
- User can't access kernel memory

**You'll know Phase 4.6 is done when:**
- Can load simple ELF binary
- App runs in Ring 3
- App can do syscalls
- App exits cleanly

**You'll know Phase 4.7 is done when:**
- Can open /dev/null
- Shell can read keyboard via /dev/tty
- Can list files with `ls`
- Processes can send messages to each other

---

**Bottom line**: You have amazing infrastructure, but need the "glue" (4.5, 4.6, 4.7) to make it actually run applications. Start with 4.5 - it's the foundation for everything else.

---

### Phase 9: GUI & Window System 🖼️

**WHY**: Modern OS needs graphical interface

- [ ] **Compositor**
  - [ ] Double buffering
  - [ ] Window manager (tiling/floating)
  - [ ] Z-order management
  - [ ] Damage tracking (only redraw changed areas)

- [ ] **Graphics Drivers**
  - [ ] VESA framebuffer (already have basic)
  - [ ] Intel i915 driver
  - [ ] AMD/NVIDIA basics
  - [ ] Hardware acceleration

- [ ] **GUI Framework**
  - [ ] Event system (mouse, keyboard)
  - [ ] Widget library (buttons, text boxes, windows)
  - [ ] Layout engine
  - [ ] Theme system

- [ ] **Desktop Environment Elements**
  - [ ] Taskbar/dock
  - [ ] Application launcher
  - [ ] File manager
  - [ ] Settings panel

**Deliverable**: Graphical desktop, can run GUI applications

---

### Phase 10: Multi-User & Permissions 👥

**WHY**: Security and multi-user support

- [ ] **User Management**
  - [ ] User accounts (UID, GID)
  - [ ] Password hashing (bcrypt/argon2)
  - [ ] /etc/passwd, /etc/shadow
  - [ ] Login system

- [ ] **Permissions**
  - [ ] File permissions (rwx)
  - [ ] Process ownership
  - [ ] Sudo/privilege escalation
  - [ ] Access control lists (ACLs)

- [ ] **Session Management**
  - [ ] Login shell
  - [ ] Environment variables
  - [ ] Session cleanup on logout

**Deliverable**: Multiple users can log in, files have permissions

---

### Phase 11: Package Management 📦

**WHY**: Easy software installation and updates

- [ ] **Package Format**
  - [ ] .pkg or .tar.xz with metadata
  - [ ] Dependency declarations
  - [ ] Pre/post install scripts

- [ ] **Package Manager**
  - [ ] Install/uninstall packages
  - [ ] Dependency resolution
  - [ ] Package database
  - [ ] Update mechanism

- [ ] **Repository**
  - [ ] Host packages on HTTP server
  - [ ] Package signing/verification
  - [ ] Mirror support

**Deliverable**: Users can install apps with single command

---

### Phase 12: Containers & Virtualization 🐳

**WHY**: Application isolation, cloud compatibility

- [ ] **Namespaces**
  - [ ] PID namespace (isolated process tree)
  - [ ] Network namespace (isolated network stack)
  - [ ] Mount namespace (isolated filesystem view)
  - [ ] User namespace

- [ ] **Cgroups (Control Groups)**
  - [ ] CPU limits
  - [ ] Memory limits
  - [ ] I/O limits
  - [ ] Resource accounting

- [ ] **Container Runtime**
  - [ ] Create/start/stop containers
  - [ ] Image management
  - [ ] Network bridge for containers
  - [ ] OCI compatibility (Docker/Podman images)

**Deliverable**: Can run containerized applications

---

### Phase 13: Distributed Computing 🌍

**WHY**: Clustering, high availability

- [ ] **Cluster Management**
  - [ ] Node discovery
  - [ ] Leader election (Raft/Paxos)
  - [ ] Distributed consensus

- [ ] **Distributed Storage**
  - [ ] Replicated filesystem
  - [ ] Distributed block device
  - [ ] Object storage

- [ ] **Load Balancing**
  - [ ] Process migration
  - [ ] Network load balancing
  - [ ] Distributed scheduling

**Deliverable**: Multiple machines work as one system

---

### Phase 14: AI & Machine Learning Support 🤖

**WHY**: Modern workloads, edge computing

- [ ] **Hardware Acceleration**
  - [ ] NPU/TPU drivers
  - [ ] GPU compute (CUDA/OpenCL)
  - [ ] ARM SVE/NEON support

- [ ] **ML Framework Support**
  - [ ] TensorFlow compatibility
  - [ ] PyTorch support
  - [ ] ONNX runtime

- [ ] **Kernel-level Optimizations**
  - [ ] NUMA-aware scheduling
  - [ ] Huge pages for ML workloads
  - [ ] Priority boosting for inference

**Deliverable**: Can run ML models efficiently

---

### Phase 15: Embedded & IoT 📡

**WHY**: Embedded systems, edge devices

- [ ] **ARM Support**
  - [ ] ARMv8 (Cortex-A)
  - [ ] RISC-V port
  - [ ] Device tree parsing

- [ ] **Embedded Features**
  - [ ] Real-time scheduling (PREEMPT_RT)
  - [ ] Low memory footprint mode
  - [ ] Watchdog support
  - [ ] Power profiling

- [ ] **IoT Protocols**
  - [ ] MQTT client
  - [ ] CoAP support
  - [ ] BLE (Bluetooth Low Energy)

**Deliverable**: Runs on embedded ARM/RISC-V devices

---

## 🗺️ COMPLETE 15-PHASE ROADMAP

**FOUNDATION** ✅
- Phase 0-5: Boot → Fractal Memory

**USERMODE** 🚧 (Start here!)
- Phase 4.5-4.9: VM Isolation → Drivers

**SYSTEM SERVICES**
- Phase 5.5-5.6: Network → Power Management

**ADVANCED KERNEL**
- Phase 6-8: Modules → Testing → Security

**USER EXPERIENCE**
- Phase 9-11: GUI → Multi-User → Package Manager

**CLOUD-NATIVE**
- Phase 12-13: Containers → Distributed Computing

**SPECIALIZED**
- Phase 14-15: AI/ML → Embedded/IoT

---

**Timeline**: 260-350 hours total (6-8 weeks focused work)

**This is the COMPLETE roadmap - bare metal to production OS!** 🚀
