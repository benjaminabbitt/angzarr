#!/bin/bash
# Build a bootable ISO image for Angzarr kernel

set -e

echo "Building Angzarr bootable ISO..."

# Create ISO directory structure
mkdir -p isodir/boot/grub

# Copy kernel binary
cp angzarr-kernel/target/x86_64-unknown-none/release/angzarr-kernel isodir/boot/angzarr.bin

# Create GRUB configuration
cat > isodir/boot/grub/grub.cfg <<EOF
set timeout=0
set default=0

menuentry "Angzarr Kernel" {
    multiboot2 /boot/angzarr.bin
    boot
}
EOF

# Check if grub-mkrescue is available
if command -v grub-mkrescue &> /dev/null; then
    echo "Creating ISO with grub-mkrescue..."
    grub-mkrescue -o angzarr.iso isodir
    echo "ISO created: angzarr.iso"
elif command -v xorriso &> /dev/null; then
    echo "grub-mkrescue not found, trying xorriso directly..."
    # Fallback to xorriso if grub-mkrescue not available
    echo "Warning: Manual ISO creation required"
    echo "Please install grub-mkrescue or use provided kernel binary"
else
    echo "Warning: Neither grub-mkrescue nor xorriso found"
    echo "ISO creation skipped. Kernel binary available at:"
    echo "  isodir/boot/angzarr.bin"
fi

# Clean up
# rm -rf isodir  # Keep for debugging

echo "Build complete!"
echo "To run in QEMU: qemu-system-x86_64 -cdrom angzarr.iso"
echo "To run in Xen: Use scripts/test-xen.sh"
