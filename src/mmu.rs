union PageTableEntry {
    section: SectionDescriptor,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct SectionDescriptor(u32);

impl SectionDescriptor {
    const fn new(
        base_address: SectionBaseAddress,
        access_permissions: AccessPermissions,
        MemoryAttributes {
            execute,
            global,
            memory_type,
        }: MemoryAttributes,
    ) -> Self {
        let mut address = match base_address {
            SectionBaseAddress::Section(addr) => (addr as u32) << 20,
            SectionBaseAddress::SuperSection(addr) => ((addr as u32) << 24) & (1 << 18),
        };
        address |= ((!global) as u32) << 17;
        let shareable = match memory_type {
            MemoryType::Normal { shareable, .. } => shareable,
            MemoryType::Device { shareable } => shareable,
            MemoryType::StronglyOrdered => true,
        };
        address |= (shareable as u32) << 16;
        // See ARMv6 Architecture Reference Manual, section B4.3.1.
        let (apx_bit, ap_bits) = match access_permissions {
            AccessPermissions::NoAccess => (0, 0b00),
            AccessPermissions::ReadOnlyUserNone => (1, 0b01),
            AccessPermissions::ReadOnly => (1, 0b10),
            AccessPermissions::ReadWriteUserNone => (0, 0b01),
            AccessPermissions::ReadWriteUserReadOnly => (0, 0b10),
            AccessPermissions::ReadWrite => (0, 0b11),
        };
        address |= apx_bit << 15;
        // See ARMv6 Architecture Reference Manual, section B4.4.1.
        let (tex_bits, c_bit, b_bit) = match memory_type {
            MemoryType::Normal { inner, outer, .. } => {
                let tex = match outer {
                    CachePolicy::NonCacheable => 0b100,
                    CachePolicy::WriteThrough => 0b110,
                    CachePolicy::WriteBack => 0b111,
                    CachePolicy::WriteAllocate => 0b101,
                };
                let (c, b) = match inner {
                    CachePolicy::NonCacheable => (0, 0),
                    CachePolicy::WriteThrough => (1, 0),
                    CachePolicy::WriteBack => (1, 1),
                    CachePolicy::WriteAllocate => (0, 1),
                };
                (tex, c, b)
            }
            MemoryType::Device { shareable: true } => (0b000, 0, 1),
            MemoryType::Device { shareable: false } => (0b010, 0, 0),
            MemoryType::StronglyOrdered => (0b000, 0, 0),
        };
        address |= tex_bits << 12;
        address |= ap_bits << 10;
        address |= ((!execute) as u32) << 4;
        address |= c_bit << 3;
        address |= b_bit << 2;
        address |= 0b10;
        SectionDescriptor(address)
    }
}

/// The base address of a memory region.
enum SectionBaseAddress {
    /// The section's base address.
    ///
    /// It should reside in the first 12 bits of the address.
    // TODO: Support Domains?
    Section(u16),
    /// The super sections' base address.
    SuperSection(u8),
}

/// Access permissions for a memory region.
enum AccessPermissions {
    /// No access is allowed.
    NoAccess,
    /// Read only access is allowed in priviledged mode, no access in user mode.
    ReadOnlyUserNone,
    /// Read only access is allowed in priviledged and user mode.
    ReadOnly,
    /// Read write access is allowed in priviledged mode, no access in user mode.
    ReadWriteUserNone,
    /// Read write access is allowed in priviledged mode, read only access in user mode.
    ReadWriteUserReadOnly,
    /// Read write access is allowed in priviledged and user mode.
    ReadWrite,
}

struct MemoryAttributes {
    /// Whether memory accesses can be an instruction fetch.
    execute: bool,
    /// Whether the memory is globally accessible.
    global: bool,
    /// The memory type.
    memory_type: MemoryType,
}

/// The type of memory.
///
/// Used in the `MemoryAttributes` struct to describe the type of memory.
enum MemoryType {
    /// Normal memory.
    Normal {
        /// Inner cache policy.
        inner: CachePolicy,
        /// Outer cache policy.
        outer: CachePolicy,
        /// Whether the memory is shareable.
        shareable: bool,
    },
    /// Device memory.
    Device {
        /// Whether the memory is shareable.
        shareable: bool,
    },
    /// Strongly ordered memory.
    StronglyOrdered,
}

/// The cache policy for a memory region.
enum CachePolicy {
    /// No caching is allowed.
    NonCacheable,
    /// Write through caching, no write allocate.
    WriteThrough,
    /// Write back caching, no write allocate.
    WriteBack,
    /// write back caching, write allocate.
    WriteAllocate,
}
