// src/loaderblock.rs
use crate::vm::Vm;

#[repr(C, packed)]
pub struct ListEntry {
    pub flink: u64,
    pub blink: u64,
}

pub struct Kpcr;

impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64) -> Result<(), &'static str> {
        let prcb_v = vaddr + 0x180;
        vm.write_memory(paddr as usize, &[0u8; 4096])?;
        
        vm.write_memory(paddr as usize + 0x18, &vaddr.to_le_bytes())?;   // Self
        vm.write_memory(paddr as usize + 0x20, &prcb_v.to_le_bytes())?;  // Prcb
        vm.write_memory(paddr as usize + 0x180, &prcb_v.to_le_bytes())?; // Prcb.Self
        vm.write_memory(paddr as usize + 0x180 + 0x24, &[0u8])?;         // Number = 0
        Ok(())
    }
}

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64) -> Result<(), &'static str> {
        // 1. 64KB 전체 초기화
        vm.write_memory(lpb_p as usize, &[0u8; 65536])?; 
        
        // 2. Prcb 포인터 융단 폭격
        for offset in (0x38..0x500).step_by(8) {
            vm.write_memory(lpb_p as usize + offset, &prcb_v.to_le_bytes())?;
        }

        // 3. 리스트 헤더 초기화
        for offset in [0x00, 0x10, 0x20] {
            let addr = lpb_v + offset as u64;
            vm.write_memory(lpb_p as usize + offset, &addr.to_le_bytes())?;
            vm.write_memory(lpb_p as usize + offset + 8, &addr.to_le_bytes())?;
        }

        // 4. [CRITICAL] 하드웨어 구성 요소 주입
        // ConfigurationRoot (Offset 0x140 - Win10 x64 기준)
        let config_root_v = lpb_v + 0x9000;
        let config_root_p = lpb_p + 0x9000;
        vm.write_memory(lpb_p as usize + 0x140, &config_root_v.to_le_bytes())?;
        
        // Configuration Component (텅 빈 루트 노드)
        // Class=System(0), Type=Maximum(7), Flags=0
        vm.write_memory(config_root_p as usize, &[0u8; 64])?; 

        // ArcBootDeviceName (Offset 0x120) - "multi(0)disk(0)rdisk(0)partition(1)"
        let arc_name_v = lpb_v + 0xA000;
        let arc_name_p = lpb_p + 0xA000;
        vm.write_memory(lpb_p as usize + 0x120, &arc_name_v.to_le_bytes())?;
        
        // "multi(0)disk(0)rdisk(0)partition(1)" (Unicode)
        let arc_str = "m\0u\0l\0t\0i\0(\00\0)\0d\0i\0s\0k\0(\00\0)\0r\0d\0i\0s\0k\0(\00\0)\0p\0a\0r\0t\0i\0t\0i\0o\0n\0(\01\0)\0\0\0";
        vm.write_memory(arc_name_p as usize, arc_str.as_bytes())?;

        // 5. 정적 필드 고정
        vm.write_memory(lpb_p as usize + 0x2c, &1u32.to_le_bytes())?; // NumberOfProcessors
        vm.write_memory(lpb_p as usize + 0x30, &stack_v.to_le_bytes())?; // KernelStack
        vm.write_memory(lpb_p as usize + 0x1F8, &(prcb_v - 0x180).to_le_bytes())?; // CommonDataArea -> KPCR

        // 6. Extension (0x110)
        let ext_v = lpb_v + 0x8000;
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(lpb_p as usize + 0x110, &ext_v.to_le_bytes())?;
        vm.write_memory(ext_p as usize, &0x400u32.to_le_bytes())?; // Size
        vm.write_memory(ext_p as usize + 4, &10u32.to_le_bytes())?; // Version 10

        Ok(())
    }

    pub fn set_acpi(vm: &mut Vm, lpb_p: u64, rsdp_v: u64) -> Result<(), &'static str> {
        let ext_p = lpb_p + 0x8000;
        vm.write_memory(ext_p as usize + 0x10, &rsdp_v.to_le_bytes())?;
        Ok(())
    }

    pub fn add_module(vm: &mut Vm, _lpb_v: u64, _lpb_p: u64, _entry_v: u64, entry_p: u64, 
                      img_base: u64, entry_point: u64, size: u32) -> Result<(), &'static str> {
        let mut data = [0u8; 256];
        data[0x48..0x50].copy_from_slice(&img_base.to_le_bytes());
        data[0x50..0x58].copy_from_slice(&entry_point.to_le_bytes());
        data[0x58..0x5c].copy_from_slice(&size.to_le_bytes());
        vm.write_memory(entry_p as usize, &data)?;
        Ok(())
    }

    pub fn add_memory(vm: &mut Vm, _lpb_v: u64, lpb_p: u64, entry_v: u64, entry_p: u64, 
                      base_addr: u64, size: u64, mem_type: u32) -> Result<(), &'static str> {
        let mut data = [0u8; 64];
        data[0x10..0x14].copy_from_slice(&mem_type.to_le_bytes());
        data[0x18..0x20].copy_from_slice(&(base_addr >> 12).to_le_bytes());
        data[0x20..0x28].copy_from_slice(&(size >> 12).to_le_bytes());
        vm.write_memory(entry_p as usize, &data)?;
        vm.write_memory(lpb_p as usize + 0x10, &entry_v.to_le_bytes())?; // Head.Flink
        Ok(())
    }
}