.PHONY: all clean run build iso userspace

KERNEL := target/x86_64-unknown-none/release/astral-kernel
ISO := astral-os.iso
LIMINE_DIR := limine
OVMF_PATH := /usr/share/edk2/x64/OVMF.4m.fd

all: iso

build:
	@echo "🔨 Building Astral OS kernel..."
	cd kernel && cargo build --release

userspace:
	@echo "🔨 Building Userspace..."
	cd userspace && cargo build --release --target x86_64-astral-user.json
	@cp userspace/target/x86_64-astral-user/release/hello kernel/hello.elf

iso: build userspace
	@echo "📀 Creating bootable ISO..."
	@mkdir -p iso_root/boot/limine
	@cp $(KERNEL) iso_root/boot/kernel.elf
	@cp limine.conf iso_root/boot/limine/
	@if [ ! -d "$(LIMINE_DIR)" ]; then \
		echo "📥 Downloading Limine bootloader..."; \
		git clone https://github.com/limine-bootloader/limine.git --branch=v8.x-binary --depth=1; \
	fi
	@cp $(LIMINE_DIR)/limine-bios.sys iso_root/boot/limine/
	@cp $(LIMINE_DIR)/limine-bios-cd.bin iso_root/boot/limine/
	@cp $(LIMINE_DIR)/limine-uefi-cd.bin iso_root/boot/limine/
	@mkdir -p iso_root/EFI/BOOT
	@cp $(LIMINE_DIR)/BOOTX64.EFI iso_root/EFI/BOOT/
	@xorriso -as mkisofs \
		-b boot/limine/limine-bios-cd.bin \
		-no-emul-boot -boot-load-size 4 -boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part --efi-boot-image --protective-msdos-label \
		iso_root -o $(ISO) 2>/dev/null
	@$(LIMINE_DIR)/limine bios-install $(ISO) 2>/dev/null || true
	@echo "✅ ISO created: $(ISO)"

run: iso
	@echo "🚀 Launching Astral OS in QEMU..."
	qemu-system-x86_64 \
		-cdrom $(ISO) \
		-serial stdio \
		-m 512M \
		-enable-kvm \
		-cpu host \
		-smp 2

run-uefi: iso
	@echo "🚀 Launching Astral OS in QEMU (UEFI)..."
	qemu-system-x86_64 \
		-cdrom $(ISO) \
		-bios $(OVMF_PATH) \
		-serial stdio \
		-m 512M \
		-enable-kvm \
		-cpu host \
		-smp 2

run-debug: iso
	@echo "🐛 Launching Astral OS in QEMU (Debug mode)..."
	qemu-system-x86_64 \
		-cdrom $(ISO) \
		-serial stdio \
		-m 512M \
		-s -S \
		-no-reboot \
		-d int,cpu_reset

clean:
	@echo "🧹 Cleaning build artifacts..."
	rm -rf iso_root $(ISO)
	cd kernel && cargo clean

help:
	@echo "Astral OS Build System"
	@echo "======================"
	@echo "make build      - Build the kernel"
	@echo "make iso        - Create bootable ISO"
	@echo "make run        - Run in QEMU (BIOS)"
	@echo "make run-uefi   - Run in QEMU (UEFI)"
	@echo "make run-debug  - Run in QEMU with GDB server"
	@echo "make clean      - Clean build artifacts"
