// src/loaderblock.rs

use crate::vm::Vm;

pub struct Kpcr;

impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64, gdt_v: u64, idt_v: u64, tss_v: u64, stack_v: u64) -> Result<(), &'static str> {
        let prcb_v = vaddr + 0x180;
        let prcb_p = paddr + 0x180;
        
        let dummy_thread_v = vaddr + 0x5000;
        let dummy_thread_p = paddr + 0x5000;
        let dummy_process_v = vaddr + 0x6000;
        let dummy_process_p = paddr + 0x6000;

        // KPCR 및 주변 영역 초기화 (64KB)
        vm.write_memory(paddr as usize, &[0u8; 65536])?;
        
        // 1. _KPCR Layout (Vergilius 19041 x64 표준 완벽 준수)
        vm.write_memory(paddr as usize + 0x00, &gdt_v.to_le_bytes())?;    // GdtBase @ 0x00
        vm.write_memory(paddr as usize + 0x08, &tss_v.to_le_bytes())?;    // TssBase @ 0x08
        vm.write_memory(paddr as usize + 0x18, &vaddr.to_le_bytes())?;    // Self @ 0x18
        vm.write_memory(paddr as usize + 0x20, &prcb_v.to_le_bytes())?;   // CurrentPrcb @ 0x20
        vm.write_memory(paddr as usize + 0x30, &vaddr.to_le_bytes())?;    // Used_Self (NtTib.Self) @ 0x30
        vm.write_memory(paddr as usize + 0x38, &idt_v.to_le_bytes())?;    // IdtBase @ 0x38

        // GS:[0x188] CurrentThread (PRCB 오프셋 상대 주소)
        vm.write_memory(paddr as usize + 0x188, &dummy_thread_v.to_le_bytes())?;
        
        // 2. _KPRCB 정밀 초기화 (0x180 지점 시작)
        vm.write_memory(prcb_p as usize + 0x00, &0x1F80u32.to_le_bytes())?;      // MxCsr @ 0x0
        vm.write_memory(prcb_p as usize + 0x08, &dummy_thread_v.to_le_bytes())?; // CurrentThread @ 0x8
        vm.write_memory(prcb_p as usize + 0x10, &dummy_thread_v.to_le_bytes())?; // NextThread @ 0x10
        vm.write_memory(prcb_p as usize + 0x18, &dummy_thread_v.to_le_bytes())?; // IdleThread @ 0x18
        vm.write_memory(prcb_p as usize + 0x28, &stack_v.to_le_bytes())?;        // RspBase @ 0x28
        vm.write_memory(prcb_p as usize + 0x88, &1u16.to_le_bytes())?;           // MinorVersion @ 0x88
        vm.write_memory(prcb_p as usize + 0x8A, &1u16.to_le_bytes())?;           // MajorVersion @ 0x8A
        
        // CurrentProcess 포인터 @ 0x40 (19041 x64)
        vm.write_memory((prcb_p + 0x40) as usize, &dummy_process_v.to_le_bytes())?;

        // 3. _KPROCESS 기초 설정 (CR3)
        let valid_cr3 = 0x8102000u64; 
        vm.write_memory((dummy_process_p + 0x28) as usize, &valid_cr3.to_le_bytes())?;

        // 4. _KTHREAD.Process @ 0xA8
        vm.write_memory((dummy_thread_p + 0xA8) as usize, &dummy_process_v.to_le_bytes())?;

        Ok(())
    }
}

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64, registry_v: u64, registry_size: u32) -> Result<(), &'static str> {
        let dummy_tree_v = lpb_v + 0x2000;
        let arc_name_v = lpb_v + 0xA000;
        let nt_path_v = lpb_v + 0xB000;
        let options_v = lpb_v + 0xC000;
        let ext_v = lpb_v + 0x8000;
        let ext_p = lpb_p + 0x8000;

        // LPB 전체 초기화 (64KB)
        vm.write_memory(lpb_p as usize, &[0u8; 65536])?;
        
        // [Vergilius _LOADER_PARAMETER_BLOCK x64 준수]
        vm.write_memory(lpb_p as usize + 0x00, &10u32.to_le_bytes())?;
        vm.write_memory(lpb_p as usize + 0x04, &0u32.to_le_bytes())?;
        vm.write_memory(lpb_p as usize + 0x08, &0x160u32.to_le_bytes())?;

        // 1. Standard List Heads (0x10 ~ 0x70)
        for offset in [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70] {
            let addr = lpb_v + offset as u64;
            vm.write_memory(lpb_p as usize + offset, &addr.to_le_bytes())?;
            vm.write_memory(lpb_p as usize + offset + 8, &addr.to_le_bytes())?;
        }
        
        // 2. 핵심 포인터 (사용자 정의 오프셋 규격 준수)
        vm.write_memory(lpb_p as usize + 0x80, &stack_v.to_le_bytes())?;    
        vm.write_memory(lpb_p as usize + 0x88, &prcb_v.to_le_bytes())?;     
        vm.write_memory(lpb_p as usize + 0x90, &(prcb_v + 0x5e80).to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0x98, &(prcb_v + 0x4e80).to_le_bytes())?; 

        // 3. Registry & Configuration (Validated by User & Vergilius)
        vm.write_memory(lpb_p as usize + 0xA4, &registry_size.to_le_bytes())?; // Length @ 0xA4
        vm.write_memory(lpb_p as usize + 0xA8, &registry_v.to_le_bytes())?;    // Base @ 0xA8
        vm.write_memory(lpb_p as usize + 0xB0, &dummy_tree_v.to_le_bytes())?; 

        // 4. Paths & Options
        vm.write_memory(lpb_p as usize + 0xB8, &arc_name_v.to_le_bytes())?; 
        vm.write_memory(lpb_p as usize + 0xC8, &nt_path_v.to_le_bytes())?;  
        
        let options_str = "/DEBUG /DEBUGPORT=COM1 /BAUDRATE=115200 /EMS"; 
        let mut options_bytes = Vec::new();
        for c in options_str.encode_utf16() { options_bytes.extend_from_slice(&c.to_le_bytes()); }
        options_bytes.extend_from_slice(&[0, 0]); 
        vm.write_memory((lpb_p + 0xC000) as usize, &options_bytes)?;
        vm.write_memory(lpb_p as usize + 0xD8, &options_v.to_le_bytes())?;

        // 5. Extension @ 0xF0
        vm.write_memory(lpb_p as usize + 0xF0, &ext_v.to_le_bytes())?;
        vm.write_memory(ext_p as usize, &0x158u32.to_le_bytes())?; 
        vm.write_memory(ext_p as usize + 4, &5u32.to_le_bytes())?;

        Ok(())
    }

    pub fn set_acpi(vm: &mut Vm, lpb_p: u64, rsdp_v: u64) -> Result<(), &'static str> {
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(ext_p as usize + 0x10, &rsdp_v.to_le_bytes())?;
        Ok(())
    }

    pub fn set_hardware_tables(vm: &mut Vm, lpb_p: u64, gdt_v: u64, idt_v: u64, tss_v: u64) -> Result<(), &'static str> {
        let ext_p = lpb_p + 0x8000;
        
        // [Vergilius & User-Identified Layout 준수]
        // 8-byte Base Pointers first (0x00, 0x08, 0x10 간격)
        vm.write_memory(ext_p as usize + 0x40, &gdt_v.to_le_bytes())?;    // GdtBase @ +0x00 (Ext 0x40)
        vm.write_memory(ext_p as usize + 0x48, &tss_v.to_le_bytes())?;    // TssBase @ +0x08 (Ext 0x48)
        vm.write_memory(ext_p as usize + 0x50, &idt_v.to_le_bytes())?;    // IdtBase @ +0x10 (Ext 0x50)
        
        // 4-byte Limits follow
        vm.write_memory(ext_p as usize + 0x58, &0xFFu32.to_le_bytes())?;  // GdtLimit @ +0x18 (Ext 0x58)
        vm.write_memory(ext_p as usize + 0x5C, &0xFFFu32.to_le_bytes())?; // IdtLimit @ +0x1C (Ext 0x5C)
        vm.write_memory(ext_p as usize + 0x60, &0x67u32.to_le_bytes())?;  // TssLimit @ +0x20 (Ext 0x60)

        // HypervisorFlags @ 0x108 
        vm.write_memory(ext_p as usize + 0x108, &1u32.to_le_bytes())?; 

        Ok(())
    }

    pub fn add_module(vm: &mut Vm, _lpb_v: u64, _lpb_p: u64, _entry_v: u64, entry_p: u64, 
                      img_base: u64, entry_point: u64, size: u32, name: &str) -> Result<(), &'static str> {
        let mut data = [0u8; 512]; 
        
        // [_LDR_DATA_TABLE_ENTRY x64 표준 준수]
        data[0x30..0x38].copy_from_slice(&img_base.to_le_bytes());    // DllBase @ 0x30
        data[0x38..0x40].copy_from_slice(&entry_point.to_le_bytes()); // EntryPoint @ 0x38
        data[0x40..0x44].copy_from_slice(&size.to_le_bytes());        // SizeOfImage @ 0x40
        
        let name_v_addr = _entry_v + 0x150; 
        let name_p_addr = entry_p + 0x150;
        let mut utf16_name: Vec<u8> = Vec::new();
        for c in name.encode_utf16() { utf16_name.extend_from_slice(&c.to_le_bytes()); }
        let name_len = utf16_name.len() as u16;
        utf16_name.extend_from_slice(&[0, 0]); 

        // FullDllName (UNICODE_STRING) @ 0x48
        data[0x48..0x4a].copy_from_slice(&name_len.to_le_bytes());
        data[0x4a..0x4c].copy_from_slice(&(name_len + 2).to_le_bytes());
        data[0x50..0x58].copy_from_slice(&name_v_addr.to_le_bytes());
        
        // BaseDllName (UNICODE_STRING) @ 0x58
        data[0x58..0x5a].copy_from_slice(&name_len.to_le_bytes());
        data[0x5a..0x5c].copy_from_slice(&(name_len + 2).to_le_bytes());
        data[0x60..0x68].copy_from_slice(&name_v_addr.to_le_bytes());
        
        // Flags @ 0x68
        data[0x68..0x6C].copy_from_slice(&0x40004u32.to_le_bytes()); 
        // USHORT ObsoleteLoadCount @ 0x6C
        data[0x6C..0x6E].copy_from_slice(&1u16.to_le_bytes());

        vm.write_memory(entry_p as usize, &data)?;
        vm.write_memory(name_p_addr as usize, &utf16_name)?;
        Ok(())
    }

    pub fn add_memory(vm: &mut Vm, _lpb_v: u64, lpb_p: u64, _entry_v: u64, entry_p: u64, 
                      base_addr: u64, size: u64, mem_type: u32) -> Result<(), &'static str> {
        let mut data = [0u8; 64];
        data[0x10..0x14].copy_from_slice(&mem_type.to_le_bytes());
        data[0x18..0x20].copy_from_slice(&(base_addr >> 12).to_le_bytes());
        data[0x20..0x28].copy_from_slice(&(size >> 12).to_le_bytes());
        vm.write_memory(entry_p as usize, &data)?;
        Ok(())
    }
}
