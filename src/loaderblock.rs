// src/loaderblock.rs
use crate::vm::Vm;

pub struct Kpcr;

impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64) -> Result<(), &'static str> {
        let prcb_v = vaddr + 0x180;
        // KPCR/PRCB 영역을 32KB로 확장하여 안정성 확보
        vm.write_memory(paddr as usize, &[0u8; 32768])?;
        vm.write_memory(paddr as usize + 0x18, &vaddr.to_le_bytes())?;
        vm.write_memory(paddr as usize + 0x20, &prcb_v.to_le_bytes())?;
        vm.write_memory(paddr as usize + 0x180, &prcb_v.to_le_bytes())?;
        Ok(())
    }
}

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64) -> Result<(), &'static str> {
        // 1. 64KB 전체 초기화
        vm.write_memory(lpb_p as usize, &[0u8; 65536])?;
        
        // 2. [x64 Standard] List Heads (0x10, 0x20, 0x30)
        // 0x00 is Version/Size, DO NOT OVERWRITE with pointers.
        vm.write_memory(lpb_p as usize + 0x08, &0x200u32.to_le_bytes())?; // Size

        for offset in [0x10, 0x20, 0x30] {
            let addr = lpb_v + offset as u64;
            vm.write_memory(lpb_p as usize + offset, &addr.to_le_bytes())?;
            vm.write_memory(lpb_p as usize + offset + 8, &addr.to_le_bytes())?;
        }

        // 3. [FIX] x64 Modern (Win10/11) Offsets
        vm.write_memory(lpb_p as usize + 0xA0, &stack_v.to_le_bytes())?; // KernelStack
        vm.write_memory(lpb_p as usize + 0xA8, &prcb_v.to_le_bytes())?;  // Prcb
        vm.write_memory(lpb_p as usize + 0xB0, &prcb_v.to_le_bytes())?;  // Process
        vm.write_memory(lpb_p as usize + 0xB8, &prcb_v.to_le_bytes())?;  // Thread

        // 4. 하드웨어 구성 트리 및 부팅 경로 (Modern x64 Offsets)
        let config_root_v = lpb_v + 0x9000;
        vm.write_memory(lpb_p as usize + 0xD0, &config_root_v.to_le_bytes())?; // ConfigurationRoot @ 0xD0

        let arc_name_v = lpb_v + 0xA000;
        let nt_path_v = lpb_v + 0xB000;
        vm.write_memory(lpb_p as usize + 0xD8, &arc_name_v.to_le_bytes())?; // ArcBootDeviceName @ 0xD8
        vm.write_memory(lpb_p as usize + 0xE0, &arc_name_v.to_le_bytes())?; // ArcHalDeviceName @ 0xE0
        vm.write_memory(lpb_p as usize + 0xE8, &nt_path_v.to_le_bytes())?;  // NtBootPathName @ 0xE8
        vm.write_memory(lpb_p as usize + 0xF0, &nt_path_v.to_le_bytes())?;  // NtHalPathName @ 0xF0

        // 6. [FIX] Extension 연결 - x64 표준 오프셋 0x110
        let ext_v = lpb_v + 0x8000;
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(lpb_p as usize + 0x110, &ext_v.to_le_bytes())?; 
        
        vm.write_memory(ext_p as usize, &0x158u32.to_le_bytes())?; // Size
        vm.write_memory(ext_p as usize + 4, &5u32.to_le_bytes())?;   // Version

        // 7. CommonDataArea (KPCR)
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

        let name_v_addr = _entry_v + 256;
        let name_p_addr = entry_p + 256;
        let mut utf16_name: Vec<u8> = Vec::new();
        for c in name.encode_utf16() { utf16_name.extend_from_slice(&c.to_le_bytes()); }
        utf16_name.extend_from_slice(&[0, 0]); 

        data[0x48..0x4a].copy_from_slice(&(utf16_name.len() as u16 - 2).to_le_bytes());
        data[0x4a..0x4c].copy_from_slice(&(utf16_name.len() as u16).to_le_bytes());
        data[0x50..0x58].copy_from_slice(&name_v_addr.to_le_bytes());
        
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
