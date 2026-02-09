// src/loaderblock.rs

use crate::vm::Vm;

pub struct Kpcr;

impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64) -> Result<(), &'static str> {
        let prcb_v = vaddr + 0x180;
        vm.write_memory(paddr as usize, &[0u8; 32768])?;
        vm.write_memory(paddr as usize + 0x18, &vaddr.to_le_bytes())?; // SelfPcr
        vm.write_memory(paddr as usize + 0x30, &vaddr.to_le_bytes())?; // NT_TIB.Self
        vm.write_memory(paddr as usize + 0x20, &prcb_v.to_le_bytes())?; // Prcb
        Ok(())
    }
}

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64, registry_v: u64, registry_size: u32) -> Result<(), &'static str> {
        vm.write_memory(lpb_p as usize, &[0u8; 65536])?;
        
        // List Heads (Standard x64: 0x00, 0x10, 0x20, 0x30)
        for offset in [0x00, 0x10, 0x20, 0x30] {
            let addr = lpb_v + offset as u64;
            vm.write_memory(lpb_p as usize + offset, &addr.to_le_bytes())?;
            vm.write_memory(lpb_p as usize + offset + 8, &addr.to_le_bytes())?;
        }
        
        // [FIX] Registry Information Spraying (For maximum compatibility)
        // Covers Win7(0x70), Win10(0xA0), and variants
        // Base(8bytes) + Length(4bytes + padding)
        for offset in [0x70, 0x78, 0x80, 0x88, 0x90, 0xA0, 0xA8, 0xB0, 0xC0] {
            vm.write_memory(lpb_p as usize + offset, &registry_v.to_le_bytes())?;
            vm.write_memory(lpb_p as usize + offset + 8, &registry_size.to_le_bytes())?;
        }

        // LoadOptions (String Pointer)
        let options_v = lpb_v + 0xC000;
        let options_str = "/DEBUG /DEBUGPORT=COM1 /BAUDRATE=115200";
        let mut options_bytes = Vec::new();
        for c in options_str.encode_utf16() { options_bytes.extend_from_slice(&c.to_le_bytes()); }
        options_bytes.extend_from_slice(&[0, 0]); 
        vm.write_memory((lpb_p + 0xC000) as usize, &options_bytes)?;
        
        // Spray LoadOptions pointer too (0x48, 0x98, 0xF8 etc)
        for offset in [0x48, 0x98, 0xF8, 0x108] {
            vm.write_memory(lpb_p as usize + offset, &options_v.to_le_bytes())?;
        }

        // Modern Offsets (Likely locations for other fields)
        vm.write_memory(lpb_p as usize + 0xA0, &stack_v.to_le_bytes())?; // KernelStack might be here too?
        
        // Standard Modern Offsets
        vm.write_memory(lpb_p as usize + 0xA8, &prcb_v.to_le_bytes())?;  // Prcb
        vm.write_memory(lpb_p as usize + 0xB0, &prcb_v.to_le_bytes())?;  // Process
        vm.write_memory(lpb_p as usize + 0xB8, &prcb_v.to_le_bytes())?;  // Thread

        // Boot Path
        let arc_name_v = lpb_v + 0xA000;
        let nt_path_v = lpb_v + 0xB000;
        vm.write_memory(lpb_p as usize + 0xD8, &arc_name_v.to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0xE8, &nt_path_v.to_le_bytes())?;  

        // Extension @ 0x110
        let ext_v = lpb_v + 0x8000;
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(lpb_p as usize + 0x110, &ext_v.to_le_bytes())?; 
        vm.write_memory(ext_p as usize, &0x158u32.to_le_bytes())?; // Size
        vm.write_memory(ext_p as usize + 4, &5u32.to_le_bytes())?; // Version

        Ok(())
    }

    pub fn set_acpi(vm: &mut Vm, lpb_p: u64, rsdp_v: u64) -> Result<(), &'static str> {
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(ext_p as usize + 0x10, &rsdp_v.to_le_bytes())?;
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
        
        // 0x44: FullDllName (UNICODE_STRING) - 16 bytes
        // 0x54: BaseDllName (UNICODE_STRING) - 16 bytes (Assuming)
        // Adjust for alignment if necessary
        
        let name_v_addr = _entry_v + 256;
        let name_p_addr = entry_p + 256;
        let mut utf16_name: Vec<u8> = Vec::new();
        for c in name.encode_utf16() { utf16_name.extend_from_slice(&c.to_le_bytes()); }
        let name_len = utf16_name.len() as u16;
        utf16_name.extend_from_slice(&[0, 0]); 

        // FullDllName @ 0x48 (Standard)
        data[0x48..0x4a].copy_from_slice(&name_len.to_le_bytes());
        data[0x4a..0x4c].copy_from_slice(&(name_len + 2).to_le_bytes());
        data[0x50..0x58].copy_from_slice(&name_v_addr.to_le_bytes());
        
        // BaseDllName @ 0x58 (Standard)
        data[0x58..0x5a].copy_from_slice(&name_len.to_le_bytes());
        data[0x5a..0x5c].copy_from_slice(&(name_len + 2).to_le_bytes());
        data[0x60..0x68].copy_from_slice(&name_v_addr.to_le_bytes());
        
        data[0x68..0x6C].copy_from_slice(&1u32.to_le_bytes()); // Flags?
        data[0x6C..0x70].copy_from_slice(&0x40004u32.to_le_bytes()); // LoadCount?

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
