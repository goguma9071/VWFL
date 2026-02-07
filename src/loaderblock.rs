// src/loaderblock.rs

use crate::vm::Vm;

pub struct Kpcr;

impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64) -> Result<(), &'static str> {
        let prcb_v = vaddr + 0x180;
        vm.write_memory(paddr as usize, &[0u8; 32768])?;
        
        // [x64 KPCR Layout]
        vm.write_memory(paddr as usize + 0x18, &vaddr.to_le_bytes())?; // SelfPcr @ 0x18
        vm.write_memory(paddr as usize + 0x30, &vaddr.to_le_bytes())?; // NT_TIB.Self @ 0x30
        vm.write_memory(paddr as usize + 0x20, &prcb_v.to_le_bytes())?; // Prcb @ 0x20
        Ok(())
    }
}

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64, registry_v: u64, registry_size: u32) -> Result<(), &'static str> {
        // 64KB 전체 초기화
        vm.write_memory(lpb_p as usize, &[0u8; 65536])?;
        
        // [FIX] Standard Windows x64 List Heads (Starting at 0x00)
        // 0x00: LoadOrderListHead
        // 0x10: MemoryDescriptorListHead
        // 0x20: BootDriverListHead
        // 0x30: EarlyLaunchTimeDriverListHead
        for offset in [0x00, 0x10, 0x20, 0x30] {
            let addr = lpb_v + offset as u64;
            vm.write_memory(lpb_p as usize + offset, &addr.to_le_bytes())?;
            vm.write_memory(lpb_p as usize + offset + 8, &addr.to_le_bytes())?;
        }
        
        // [FIX] Registry Information (Offset 0x80, 0x88 for x64 Win10)
        vm.write_memory(lpb_p as usize + 0x80, &registry_v.to_le_bytes())?;
        vm.write_memory(lpb_p as usize + 0x88, &registry_size.to_le_bytes())?;

        // Modern Offsets (Win 10/11 19041+)
        vm.write_memory(lpb_p as usize + 0x44, &0x01u32.to_le_bytes())?; // Flags
        vm.write_memory(lpb_p as usize + 0xA0, &stack_v.to_le_bytes())?; // KernelStack
        vm.write_memory(lpb_p as usize + 0xA8, &prcb_v.to_le_bytes())?;  // Prcb
        vm.write_memory(lpb_p as usize + 0xB0, &prcb_v.to_le_bytes())?;  // Process
        vm.write_memory(lpb_p as usize + 0xB8, &prcb_v.to_le_bytes())?;  // Thread

        // Boot Path & Config
        let config_root_v = lpb_v + 0x9000;
        vm.write_memory(lpb_p as usize + 0xD0, &config_root_v.to_le_bytes())?; 

        let arc_name_v = lpb_v + 0xA000;
        let nt_path_v = lpb_v + 0xB000;
        vm.write_memory(lpb_p as usize + 0xD8, &arc_name_v.to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0xE0, &arc_name_v.to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0xE8, &nt_path_v.to_le_bytes())?;  
        vm.write_memory(lpb_p as usize + 0xF0, &nt_path_v.to_le_bytes())?;  

        // LoadOptions @ 0xF8
        let options_v = lpb_v + 0xC000;
        let options_str = "/DEBUG /DEBUGPORT=COM1 /BAUDRATE=115200";
        let mut options_bytes = Vec::new();
        for c in options_str.encode_utf16() { 
            options_bytes.extend_from_slice(&c.to_le_bytes()); 
        }
        options_bytes.extend_from_slice(&[0, 0]); 
        vm.write_memory((lpb_p + 0xC000) as usize, &options_bytes)?;
        vm.write_memory(lpb_p as usize + 0xF8, &options_v.to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0x108, &options_v.to_le_bytes())?; 

        // Extension @ 0x110
        let ext_v = lpb_v + 0x8000;
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(lpb_p as usize + 0x110, &ext_v.to_le_bytes())?; 
        vm.write_memory(ext_p as usize, &0x158u32.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 4, &5u32.to_le_bytes())?;   

        // KPCR/PRCB Backlink
        vm.write_memory(lpb_p as usize + 0x134, &1u32.to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0x1F8, &(prcb_v - 0x180).to_le_bytes())?; 

        Ok(())
    }

    pub fn set_acpi(vm: &mut Vm, lpb_p: u64, rsdp_v: u64) -> Result<(), &'static str> {
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(ext_p as usize + 0x10, &rsdp_v.to_le_bytes())?;
        vm.write_memory(ext_p as usize + 0x18, &rsdp_v.to_le_bytes())?;
        Ok(())
    }

    pub fn set_hardware_tables(vm: &mut Vm, lpb_p: u64, gdt_v: u64, idt_v: u64, tss_v: u64) -> Result<(), &'static str> {
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(ext_p as usize + 0x40, &gdt_v.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 0x48, &0xffu32.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 0x50, &idt_v.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 0x58, &0xfffu32.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 0x60, &tss_v.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 0x68, &0x67u32.to_le_bytes())?; 
        Ok(())
    }

    pub fn add_module(vm: &mut Vm, _lpb_v: u64, _lpb_p: u64, _entry_v: u64, entry_p: u64, 
                      img_base: u64, entry_point: u64, size: u32, name: &str) -> Result<(), &'static str> {
        let mut data = [0u8; 256];
        data[0x30..0x38].copy_from_slice(&img_base.to_le_bytes()); 
        data[0x38..0x40].copy_from_slice(&entry_point.to_le_bytes()); 
        data[0x40..0x44].copy_from_slice(&size.to_le_bytes()); 
        
        data[0x68..0x6C].copy_from_slice(&1u32.to_le_bytes());
        data[0x6C..0x70].copy_from_slice(&0x40004u32.to_le_bytes()); 

        let name_v_addr = _entry_v + 256;
        let name_p_addr = entry_p + 256;
        let mut utf16_name: Vec<u8> = Vec::new();
        for c in name.encode_utf16() { utf16_name.extend_from_slice(&c.to_le_bytes()); }
        let name_len = utf16_name.len() as u16;
        utf16_name.extend_from_slice(&[0, 0]); 

        // FullDllName (Offset 0x48)
        data[0x48..0x4a].copy_from_slice(&name_len.to_le_bytes());
        data[0x4a..0x4c].copy_from_slice(&(name_len + 2).to_le_bytes());
        data[0x50..0x58].copy_from_slice(&name_v_addr.to_le_bytes());
        
        // BaseDllName (Offset 0x58)
        data[0x58..0x5a].copy_from_slice(&name_len.to_le_bytes());
        data[0x5a..0x5c].copy_from_slice(&(name_len + 2).to_le_bytes());
        data[0x60..0x68].copy_from_slice(&name_v_addr.to_le_bytes());
        
        vm.write_memory(entry_p as usize, &data)?;
        vm.write_memory(name_p_addr as usize, &utf16_name)?;
        Ok(())
    }

    pub fn add_memory(vm: &mut Vm, _lpb_v: u64, lpb_p: u64, entry_v: u64, entry_p: u64, 
                      base_addr: u64, size: u64, mem_type: u32) -> Result<(), &'static str> {
        let mut data = [0u8; 64];
        data[0x10..0x14].copy_from_slice(&mem_type.to_le_bytes());
        data[0x18..0x20].copy_from_slice(&(base_addr >> 12).to_le_bytes());
        data[0x20..0x28].copy_from_slice(&(size >> 12).to_le_bytes());
        vm.write_memory(entry_p as usize, &data)?;
        Ok(())
    }
}