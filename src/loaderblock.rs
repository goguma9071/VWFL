// src/loaderblock.rs
use crate::vm::Vm;

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct ListEntry {
    pub flink: u64,
    pub blink: u64,
}

pub struct Kpcr;

impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64) -> Result<(), &'static str> {
        // GS:[0x18] -> KPCR Self Pointer
        vm.write_memory(paddr as usize + 0x18, &vaddr.to_le_bytes())?;
        // GS:[0x20] -> PRCB Pointer (KPCR + 0x180)
        vm.write_memory(paddr as usize + 0x20, &(vaddr + 0x180).to_le_bytes())?;
        Ok(())
    }
}

/// 윈도우 메모리 지도 엔트리 (MDL)
#[repr(C, packed)]
pub struct MemoryAllocationDescriptor {
    pub list_entry: ListEntry,
    pub memory_type: u32,
    pub base_page: u64,
    pub page_count: u64,
}

/// 로드된 모듈 정보 (DataTableEntry)
#[repr(C, packed)]
pub struct KldrDataTableEntry {
    pub in_load_order_links: ListEntry,
    pub in_memory_order_links: ListEntry,
    pub in_initialization_order_links: ListEntry,
    pub image_base: u64,
    pub entry_point: u64,
    pub size_of_image: u32,
    pub full_module_name: [u16; 32],
}

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64) -> Result<(), &'static str> {
        vm.write_memory(lpb_p as usize, &[0u8; 4096])?;
        
        // 1. LoadOrderListHead (0x00)
        vm.write_memory(lpb_p as usize, &lpb_v.to_le_bytes())?;
        vm.write_memory(lpb_p as usize + 8, &lpb_v.to_le_bytes())?;
        
        // 2. MemoryDescriptorListHead (0x10)
        let mem_list_v = lpb_v + 0x10;
        vm.write_memory(lpb_p as usize + 0x10, &mem_list_v.to_le_bytes())?;
        vm.write_memory(lpb_p as usize + 0x18, &mem_list_v.to_le_bytes())?;
        
        Ok(())
    }

    /// 모듈(ntoskrnl, hal) 추가
    pub fn add_module(vm: &mut Vm, lpb_v: u64, lpb_p: u64, entry_v: u64, entry_p: u64, 
                      img_base: u64, entry_point: u64, size: u32) -> Result<(), &'static str> {
        let mut entry = KldrDataTableEntry {
            in_load_order_links: ListEntry { flink: lpb_v, blink: lpb_v },
            in_memory_order_links: ListEntry { flink: 0, blink: 0 },
            in_initialization_order_links: ListEntry { flink: 0, blink: 0 },
            image_base: img_base,
            entry_point: entry_point,
            size_of_image: size,
            full_module_name: [0; 32],
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(&entry as *const _ as *const u8, std::mem::size_of::<KldrDataTableEntry>())
        };
        vm.write_memory(entry_p as usize, bytes)?;
        vm.write_memory(lpb_p as usize, &entry_v.to_le_bytes())?; // Flink
        vm.write_memory(lpb_p as usize + 8, &entry_v.to_le_bytes())?; // Blink
        Ok(())
    }

    /// 메모리 지도 추가
    pub fn add_memory(vm: &mut Vm, lpb_v: u64, lpb_p: u64, entry_v: u64, entry_p: u64, 
                      base_addr: u64, size: u64, mem_type: u32) -> Result<(), &'static str> {
        let entry = MemoryAllocationDescriptor {
            list_entry: ListEntry { flink: lpb_v + 0x10, blink: lpb_v + 0x10 },
            memory_type: mem_type,
            base_page: base_addr >> 12,
            page_count: size >> 12,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(&entry as *const _ as *const u8, std::mem::size_of::<MemoryAllocationDescriptor>())
        };
        vm.write_memory(entry_p as usize, bytes)?;
        vm.write_memory(lpb_p as usize + 0x10, &entry_v.to_le_bytes())?;
        vm.write_memory(lpb_p as usize + 0x18, &entry_v.to_le_bytes())?;
        Ok(())
    }
}