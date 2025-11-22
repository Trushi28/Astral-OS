// ============================================================================
// ASTRAL OS - ZIG SPATIAL MEMORY MANAGER
// Enhanced with Reality-Aware allocation and Subspace support
// ============================================================================

// ============================================================================
// SPATIAL COORDINATE - 3D addressing for fractal memory
// ============================================================================

pub const SpatialCoordinate = struct {
    x: u64,
    y: u64,
    z: u64,

    pub fn to_linear(self: SpatialCoordinate) u64 {
        return mortonEncode3(self.x, self.y, self.z);
    }

    pub fn distance(self: SpatialCoordinate, other: SpatialCoordinate) u64 {
        const dx = if (self.x > other.x) self.x - other.x else other.x - self.x;
        const dy = if (self.y > other.y) self.y - other.y else other.y - self.y;
        const dz = if (self.z > other.z) self.z - other.z else other.z - self.z;
        return dx + dy + dz;
    }

    pub fn contains(self: SpatialCoordinate, other: SpatialCoordinate, radius: u64) bool {
        return self.distance(other) <= radius;
    }
};

fn mortonEncode3(x: u64, y: u64, z: u64) u64 {
    return expandBits(x) | (expandBits(y) << 1) | (expandBits(z) << 2);
}

fn expandBits(v: u64) u64 {
    var x = v & 0x1fffff;
    x = (x | (x << 32)) & 0x1f00000000ffff;
    x = (x | (x << 16)) & 0x1f0000ff0000ff;
    x = (x | (x << 8)) & 0x100f00f00f00f00f;
    x = (x | (x << 4)) & 0x10c30c30c30c30c3;
    x = (x | (x << 2)) & 0x1249249249249249;
    return x;
}

// ============================================================================
// FRACTAL MEMORY BLOCK - matches Rust #[repr(C)]
// ============================================================================

pub const FractalMemoryBlock = extern struct {
    x: u64,
    y: u64,
    z: u64,
    physical_addr: usize,
    size: usize,
    in_use: bool,
    _padding: [7]u8,
    next: ?*FractalMemoryBlock,
};

// ============================================================================
// FRACTAL MEMORY ALLOCATOR - matches Rust #[repr(C)]
// ============================================================================

pub const FractalMemoryAllocator = extern struct {
    heap_start: usize,
    heap_size: usize,
    heap_used: usize,
    first_block: ?*FractalMemoryBlock,

    pub fn init(heap_start: usize, heap_size: usize) FractalMemoryAllocator {
        return FractalMemoryAllocator{
            .heap_start = heap_start,
            .heap_size = heap_size,
            .heap_used = 0,
            .first_block = null,
        };
    }

    fn alignUp(addr: usize, alignment: usize) usize {
        if (alignment == 0) return addr;
        const mask = alignment - 1;
        return (addr + mask) & ~mask;
    }

    fn isInHeap(self: *const FractalMemoryAllocator, addr: usize) bool {
        return addr >= self.heap_start and addr < self.heap_start + self.heap_size;
    }

    fn safeAdd(a: usize, b: usize) ?usize {
        const result = @addWithOverflow(a, b);
        if (result[1] != 0) return null;
        return result[0];
    }

    pub fn allocateSpatial(
        self: *FractalMemoryAllocator,
        coord: SpatialCoordinate,
        size: usize,
    ) ?*FractalMemoryBlock {
        if (size == 0 or size > self.heap_size) return null;

        // Look for freed block with spatial locality preference
        var best_block: ?*FractalMemoryBlock = null;
        var best_distance: u64 = ~@as(u64, 0);

        var current = self.first_block;
        while (current) |block| {
            if (!block.in_use and block.size >= size) {
                const block_coord = SpatialCoordinate{ .x = block.x, .y = block.y, .z = block.z };
                const dist = coord.distance(block_coord);
                if (dist < best_distance) {
                    best_distance = dist;
                    best_block = block;
                }
            }
            current = block.next;
        }

        // Reuse best matching freed block
        if (best_block) |block| {
            block.in_use = true;
            block.x = coord.x;
            block.y = coord.y;
            block.z = coord.z;
            const data_ptr: [*]u8 = @ptrFromInt(block.physical_addr);
            @memset(data_ptr[0..block.size], 0);
            return block;
        }

        // Allocate new block
        const metadata_size = @sizeOf(FractalMemoryBlock);
        const alignment: usize = 16;

        const current_pos = safeAdd(self.heap_start, self.heap_used) orelse return null;
        const metadata_aligned = alignUp(current_pos, alignment);
        const metadata_padding = metadata_aligned - current_pos;

        const data_pos = safeAdd(metadata_aligned, metadata_size) orelse return null;
        const data_aligned = alignUp(data_pos, alignment);
        const data_padding = data_aligned - data_pos;

        const padding_plus_meta = safeAdd(metadata_padding, metadata_size) orelse return null;
        const data_plus_size = safeAdd(data_padding, size) orelse return null;
        const total_needed = safeAdd(padding_plus_meta, data_plus_size) orelse return null;

        const new_used = safeAdd(self.heap_used, total_needed) orelse return null;
        if (new_used > self.heap_size) return null;

        self.heap_used += metadata_padding;
        const block_addr = self.heap_start + self.heap_used;
        if (!self.isInHeap(block_addr)) return null;

        const block: *FractalMemoryBlock = @ptrFromInt(block_addr);
        self.heap_used += metadata_size + data_padding;

        const data_addr = self.heap_start + self.heap_used;
        if (!self.isInHeap(data_addr)) return null;

        const end_addr = safeAdd(data_addr, size - 1) orelse return null;
        if (!self.isInHeap(end_addr)) return null;

        self.heap_used += size;

        block.* = FractalMemoryBlock{
            .x = coord.x,
            .y = coord.y,
            .z = coord.z,
            .physical_addr = data_addr,
            .size = size,
            .in_use = true,
            ._padding = [_]u8{0} ** 7,
            .next = self.first_block,
        };

        const data_ptr: [*]u8 = @ptrFromInt(data_addr);
        @memset(data_ptr[0..size], 0);

        self.first_block = block;
        return block;
    }

    pub fn deallocateSpatial(self: *FractalMemoryAllocator, coord: SpatialCoordinate) bool {
        var current = self.first_block;
        while (current) |block| {
            if (block.x == coord.x and block.y == coord.y and block.z == coord.z and block.in_use) {
                block.in_use = false;
                const data_ptr: [*]u8 = @ptrFromInt(block.physical_addr);
                @memset(data_ptr[0..block.size], 0);
                return true;
            }
            current = block.next;
        }
        return false;
    }

    pub fn findNearest(self: *FractalMemoryAllocator, coord: SpatialCoordinate, max_distance: u64) ?*FractalMemoryBlock {
        var best: ?*FractalMemoryBlock = null;
        var best_dist: u64 = max_distance + 1;

        var current = self.first_block;
        while (current) |block| {
            if (block.in_use) {
                const block_coord = SpatialCoordinate{ .x = block.x, .y = block.y, .z = block.z };
                const dist = coord.distance(block_coord);
                if (dist < best_dist) {
                    best_dist = dist;
                    best = block;
                }
            }
            current = block.next;
        }
        return best;
    }

    pub fn getStats(self: *const FractalMemoryAllocator) AllocatorStats {
        var stats = AllocatorStats{
            .total_blocks = 0,
            .used_blocks = 0,
            .total_memory = self.heap_size,
            .used_memory = self.heap_used,
            .fragmentation = 0,
        };

        var current = self.first_block;
        var free_size: usize = 0;
        while (current) |block| {
            stats.total_blocks += 1;
            if (block.in_use) {
                stats.used_blocks += 1;
            } else {
                free_size += block.size;
            }
            current = block.next;
        }

        if (self.heap_used > 0) {
            stats.fragmentation = @as(u8, @intCast((free_size * 100) / self.heap_used));
        }
        return stats;
    }
};

pub const AllocatorStats = extern struct {
    total_blocks: usize,
    used_blocks: usize,
    total_memory: usize,
    used_memory: usize,
    fragmentation: u8,
};

// ============================================================================
// C ABI EXPORTS
// ============================================================================

export fn astral_init_spatial_allocator(heap_start: usize, heap_size: usize) ?*FractalMemoryAllocator {
    if (heap_size < @sizeOf(FractalMemoryAllocator) + 4096) return null;
    if (heap_start == 0) return null;

    const allocator_ptr: *FractalMemoryAllocator = @ptrFromInt(heap_start);
    const actual_start = heap_start + @sizeOf(FractalMemoryAllocator);
    const actual_size = heap_size - @sizeOf(FractalMemoryAllocator);
    allocator_ptr.* = FractalMemoryAllocator.init(actual_start, actual_size);
    return allocator_ptr;
}

export fn astral_allocate_spatial(alloc: *FractalMemoryAllocator, x: u64, y: u64, z: u64, size: usize) ?*FractalMemoryBlock {
    const coord = SpatialCoordinate{ .x = x, .y = y, .z = z };
    return alloc.allocateSpatial(coord, size);
}

export fn astral_deallocate_spatial(alloc: *FractalMemoryAllocator, x: u64, y: u64, z: u64) bool {
    const coord = SpatialCoordinate{ .x = x, .y = y, .z = z };
    return alloc.deallocateSpatial(coord);
}

export fn astral_get_block_data(block: *FractalMemoryBlock) usize {
    return block.physical_addr;
}

export fn astral_get_block_size(block: *FractalMemoryBlock) usize {
    return block.size;
}

export fn astral_find_nearest(alloc: *FractalMemoryAllocator, x: u64, y: u64, z: u64, max_dist: u64) ?*FractalMemoryBlock {
    const coord = SpatialCoordinate{ .x = x, .y = y, .z = z };
    return alloc.findNearest(coord, max_dist);
}

export fn astral_get_allocator_stats(alloc: *FractalMemoryAllocator, stats_out: *AllocatorStats) void {
    stats_out.* = alloc.getStats();
}

export fn astral_compute_spatial_hash(x: u64, y: u64, z: u64) u64 {
    const coord = SpatialCoordinate{ .x = x, .y = y, .z = z };
    return coord.to_linear();
}

export fn astral_validate_allocator(alloc: *FractalMemoryAllocator) bool {
    if (alloc.heap_size == 0 or alloc.heap_start == 0) return false;
    if (alloc.heap_used > alloc.heap_size) return false;

    var seen: usize = 0;
    var current = alloc.first_block;
    while (current) |block| {
        seen += 1;
        if (seen > 100000) return false;
        if (block.physical_addr < alloc.heap_start) return false;
        if (block.physical_addr >= alloc.heap_start + alloc.heap_size) return false;
        if (block.size == 0 or block.size > alloc.heap_size) return false;
        current = block.next;
    }
    return true;
}

// Memory operations
export fn astral_memset(ptr: [*]u8, value: u8, size: usize) void {
    @memset(ptr[0..size], value);
}

export fn astral_memcpy(dest: [*]u8, src: [*]const u8, size: usize) void {
    @memcpy(dest[0..size], src[0..size]);
}

export fn astral_memzero(ptr: [*]u8, size: usize) void {
    @memset(ptr[0..size], 0);
}