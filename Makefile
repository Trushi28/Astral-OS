# ============================================================================
# ASTRAL OS - Limine Bootloader Makefile (Fixed)
# ============================================================================

.PHONY: all clean run debug kernel iso help limine userland

KERNEL_ELF := target/x86_64-astral/release/astral-kernel

# Default target
all: iso

help:
	@echo "Astral OS Build System"
	@echo "======================"
	@echo "  make all/iso  - Build bootable ISO"
	@echo "  make userland - Build Ring 3 binaries"
	@echo "  make kernel   - Build kernel only"
	@echo "  make run      - Build and run in QEMU"
	@echo "  make debug    - Run with GDB server"
	@echo "  make clean    - Clean build artifacts"

# Download Limine if needed
limine:
	@if [ ! -d limine ]; then \
		echo "==> Downloading Limine..."; \
		git clone https://github.com/limine-bootloader/limine.git --branch=v8.x-binary --depth=1; \
	fi
	@if [ ! -f limine/limine ]; then \
		echo "==> Building Limine utility..."; \
		$(MAKE) -C limine; \
	fi

# Build userland (Ring 3 binaries as ELF)
userland:
	@echo "==> Building userland ELF binaries..."
	cd userland && cargo +nightly build --release
	@echo "==> Userland ELF built:"
	@ls -lh target/x86_64-userland/release/userland
	@file target/x86_64-userland/release/userland

# Build kernel (depends on userland for include_bytes!)
kernel: userland
	@echo "==> Building kernel..."
	cd kernel && cargo +nightly build --release
	@echo "==> Kernel built: $(KERNEL_ELF)"
	@file $(KERNEL_ELF)


# Create ISO
iso: limine kernel
	@echo "==> Creating ISO..."
	
	# Clean and create directory structure
	rm -rf iso_root
	mkdir -p iso_root/boot
	mkdir -p iso_root/boot/limine
	mkdir -p iso_root/EFI/BOOT
	
	# Copy kernel
	cp $(KERNEL_ELF) iso_root/boot/astral-kernel
	
	# Copy Limine files
	cp limine.conf iso_root/boot/limine/
	cp limine/limine-bios.sys iso_root/boot/limine/
	cp limine/limine-bios-cd.bin iso_root/boot/limine/
	cp limine/limine-uefi-cd.bin iso_root/boot/limine/
	cp limine/BOOTX64.EFI iso_root/EFI/BOOT/
	
	# Also copy to root for fallback
	cp $(KERNEL_ELF) iso_root/astral-kernel
	cp limine.conf iso_root/
	
	# Create ISO with xorriso
	xorriso -as mkisofs \
		-b boot/limine/limine-bios-cd.bin \
		-no-emul-boot \
		-boot-load-size 4 \
		-boot-info-table \
		--efi-boot boot/limine/limine-uefi-cd.bin \
		-efi-boot-part \
		--efi-boot-image \
		--protective-msdos-label \
		iso_root \
		-o astral.iso
	
	# Install Limine
	./limine/limine bios-install astral.iso
	
	@echo "==> ISO created: astral.iso"
	@ls -lh astral.iso

# Run in QEMU
run: iso
	@echo "==> Starting QEMU..."
	@echo "    Press Ctrl-A X to exit"
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-m 512M \
		-smp 4 \
		-serial stdio \
		-boot d

# Run with persistent VirtIO disk
run-disk: iso disk-img
	@echo "==> Starting QEMU with VirtIO disk..."
	@echo "    Press Ctrl-A X to exit"
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-drive file=astral-disk.img,format=raw,if=none,id=disk0 \
		-device virtio-blk-pci,drive=disk0,disable-legacy=on\
		-m 512M \
		-smp 4 \
		-serial stdio \
		-boot d

# Create disk image if it doesn't exist (512MB for userland binaries)
disk-img:
	@if [ ! -f astral-disk.img ]; then \
		echo "==> Creating 512MB disk image..."; \
		qemu-img create -f raw astral-disk.img 512M; \
	fi

# Copy userland binaries to disk (for filesystem loading)
copy-userland: userland disk-img
	@echo "==> Copying userland binaries to disk..."
	@# The shell binary will be loaded from PsychicFS at runtime
	@# For now, we embed it in the kernel - filesystem loading requires
	@# PsychicFS to be mounted first and the binary written to it
	@echo "==> Userland binaries ready for filesystem integration"

# Run with UEFI (if you have OVMF)
run-uefi: iso
	@echo "==> Starting QEMU (UEFI mode)..."
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-drive file=astral-disk.img,format=raw,if=none,id=disk0 \
		-device virtio-blk-pci,drive=disk0,disable-legacy=on\
		-m 512M \
		-smp 4 \
		-serial stdio \
		-bios /usr/share/edk2/x64/OVMF.4m.fd

# Debug mode
debug: iso
	@echo "==> Starting QEMU with GDB server on :1234..."
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-m 512M \
		-smp 4 \
		-serial stdio \
		-no-reboot \
		-s -S

# Clean
clean:
	@echo "==> Cleaning..."
	cargo clean
	rm -rf iso_root astral.iso qemu.log
	@echo "==> Done"

distclean: clean
	rm -rf limine astral-disk.img
