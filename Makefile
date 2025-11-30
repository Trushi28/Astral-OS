# ============================================================================
# ASTRAL OS - Limine Bootloader Makefile (Fixed)
# ============================================================================

.PHONY: all clean run debug kernel iso help limine font

KERNEL_ELF := kernel/target/x86_64-astral/release/astral-kernel

# Default target
all: iso

help:
	@echo "Astral OS Build System"
	@echo "======================"
	@echo "  make all/iso  - Build bootable ISO"
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

# Build kernel
kernel: font
	@echo "==> Building kernel..."
	cd kernel && cargo +nightly build --release
	@echo "==> Kernel built: $(KERNEL_ELF)"
	@file $(KERNEL_ELF)

# Generate font if missing or empty
font:
	@if [ ! -f kernel/SpaceMono-Regular.ttf ] || [ ! -s kernel/SpaceMono-Regular.ttf ]; then \
		echo "==> ERROR: kernel/SpaceMono-Regular.ttf missing or empty!"; \
		exit 1; \
	fi
	@echo "==> Font: kernel/SpaceMono-Regular.ttf ($(shell wc -c < kernel/SpaceMono-Regular.ttf) bytes)"

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
		-serial stdio \
		-boot d

# Run with persistent VirtIO disk
run-disk: iso disk-img
	@echo "==> Starting QEMU with VirtIO disk..."
	@echo "    Press Ctrl-A X to exit"
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-drive file=astral-disk.img,format=raw,if=none,id=disk0 \
		-device virtio-blk-pci,drive=disk0 \
		-m 512M \
		-serial stdio \
		-boot d

# Create disk image if it doesn't exist
disk-img:
	@if [ ! -f astral-disk.img ]; then \
		echo "==> Creating 64MB disk image..."; \
		qemu-img create -f raw astral-disk.img 64M; \
	fi

# Run with UEFI (if you have OVMF)
run-uefi: iso
	@echo "==> Starting QEMU (UEFI mode)..."
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-m 512M \
		-serial stdio \
		-bios /usr/share/edk2/x64/OVMF.4m.fd

# Debug mode
debug: iso
	@echo "==> Starting QEMU with GDB server on :1234..."
	qemu-system-x86_64 \
		-cdrom astral.iso \
		-m 512M \
		-serial stdio \
		-no-reboot \
		-s -S

# Clean
clean:
	@echo "==> Cleaning..."
	cd kernel && cargo clean
	rm -rf iso_root astral.iso qemu.log astral-disk.img
	@echo "==> Done"

distclean: clean
	rm -rf limine
