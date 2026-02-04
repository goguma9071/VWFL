mod cpu;
mod debug;
mod loader;
mod loaderblock;
mod acpi;
mod pe;
mod vm;

use std::env;
use std::fs;
use vm::{Vm, MEM_SIZE};
use loaderblock::{LoaderParameterBlock, Kpcr};

// --- Physical Memory Layout ---
pub const KRNL_PBASE: u64  = 0x200000;   
pub const HAL_PBASE: u64   = 0x2000000;  
pub const SYSTEM_BASE: u64 = 0x8000000;  
pub const LPB_PBASE: u64   = 0x4000000;  
pub const KPCR_PBASE: u64  = LPB_PBASE + 0x10000; // [FIX] Page aligned (0x10000)
pub const ACPI_PBASE: u64  = 0x5000000;
pub const KUSER_PBASE: u64 = 0x9000000; 
pub const STACK_PBASE: u64 = SYSTEM_BASE + 0x100000; 

// --- Virtual Memory Layout (High-Half) ---
const K_VIRT_ANY: u64  = 0xFFFFF80000000000;
const LPB_VBASE: u64   = K_VIRT_ANY + LPB_PBASE; 
const KPCR_VBASE: u64  = LPB_VBASE + 0x10000; 
const ACPI_VBASE: u64  = K_VIRT_ANY + ACPI_PBASE;
const STACK_VIRT_BASE: u64 = 0xFFFFFE8000000000; // [FIX] PML4 Index 509
const STACK_VBASE: u64 = STACK_VIRT_BASE;

fn main() {
    let args: Vec<String> = env::args().collect();
    let sys32_path = if args.len() > 1 { &args[1] } else { "./KrnlFile" };

    println!("-----Initializing VWFL Hypervisor-----");
    let mut vm = Vm::new().expect("Failed VM");

    let krnl_buf = fs::read(format!("{}/ntoskrnl.exe", sys32_path)).expect("Read krnl");
    let krnl_pe = pe::parse(&krnl_buf).expect("Parse krnl");
    let hal_buf = fs::read(format!("{}/hal.dll", sys32_path)).expect("Read hal");
    let hal_pe = pe::parse(&hal_buf).expect("Parse hal");

    // [FIX] Initialize KUSER_SHARED_DATA (0xFFFFF78000000000)
    let mut kuser_data = [0u8; 4096];
    kuser_data[0x26c..0x270].copy_from_slice(&19041u32.to_le_bytes()); // Build 19041
    kuser_data[0x270..0x274].copy_from_slice(&10u32.to_le_bytes());    // Major 10
    vm.write_memory(KUSER_PBASE as usize, &kuser_data).ok();

    let krnl_vbase = 0xFFFFF80000200000; 
    let krnl_entry_v = loader::load_sections(&mut vm, &krnl_pe, KRNL_PBASE, krnl_vbase).expect("Load krnl");

    let hal_size = hal_pe.sections.iter().map(|s| s.virtual_address + s.virtual_size - hal_pe.image_base).max().unwrap_or(0x800000) as u32; // 실패 시 기본 8MB

    // 2. ntoskrnl size 계산.
    let krnl_size = krnl_pe.sections.iter().map(|s| s.virtual_address + s.virtual_size - krnl_pe.image_base).max().unwrap_or(0x2000000) as u32; // 실패 시 기본 32MB

    // 2. HAL의 경우, .reloc이 없다면 파일의 원래 ImageBase를 사용해야 합니다.
    let hal_vbase = hal_pe.image_base;
    println!("[DEBUG] HAL preferred base: 0x{:x}", hal_vbase);

    let hal_entry_v = loader::load_sections(&mut vm, &hal_pe, HAL_PBASE, hal_vbase).expect("Load hal");

     setup_kernel_paging(&mut vm, krnl_vbase, hal_vbase).expect("Paging failed");

    // 3. 확인용 로그
    println!("[LOADER] Kernel Entry: 0x{:016x}", krnl_entry_v);
    println!("[LOADER] HAL Entry:    0x{:016x}", hal_entry_v);
    let stack_v: u64 = STACK_VBASE + 0x10000; 

    // [STRICT] LPB Initialization with canonical addresses
    LoaderParameterBlock::setup(&mut vm, LPB_VBASE, LPB_PBASE, KPCR_VBASE + 0x180, stack_v).expect("LPB Init");
    Kpcr::setup(&mut vm, KPCR_VBASE, KPCR_PBASE).expect("KPCR Init");

    let rsdp_v = acpi::setup(&mut vm, ACPI_PBASE, ACPI_VBASE).expect("ACPI failed");
    LoaderParameterBlock::set_acpi(&mut vm, LPB_PBASE, rsdp_v).ok();

    let gdt_v: u64 = K_VIRT_ANY + SYSTEM_BASE;
    let idt_v: u64 = gdt_v + 0x20000; 
    let tss_v: u64 = gdt_v + 0x1000;
    LoaderParameterBlock::set_hardware_tables(&mut vm, LPB_PBASE, gdt_v, idt_v, tss_v).ok();

    // [FIX] Module Definitions and STRICT Canonical Linking
    let m1_v = LPB_VBASE + 0x5000; let m1_p = LPB_PBASE + 0x5000;
    let m2_v = LPB_VBASE + 0x6000; let m2_p = LPB_PBASE + 0x6000;
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, m1_v, m1_p, krnl_vbase, krnl_entry_v, krnl_size, "ntoskrnl.exe").ok();
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, m2_v, m2_p, hal_vbase, hal_entry_v, hal_size, "hal.dll").ok();

    // Standard Windows x64 LPB List Linking @ 0x10
    vm.write_memory(LPB_PBASE as usize + 0x10, &m1_v.to_le_bytes()).ok(); // Flink -> m1
    vm.write_memory(LPB_PBASE as usize + 0x18, &m2_v.to_le_bytes()).ok(); // Blink -> m2
    
    vm.write_memory(m1_p as usize, &m2_v.to_le_bytes()).ok(); // m1.Next -> m2
    vm.write_memory(m1_p as usize + 8, &(LPB_VBASE + 0x10).to_le_bytes()).ok(); // m1.Prev -> Head
    vm.write_memory(m2_p as usize, &(LPB_VBASE + 0x10).to_le_bytes()).ok(); // m2.Next -> Head
    vm.write_memory(m2_p as usize + 8, &m1_v.to_le_bytes()).ok(); // m2.Prev -> m1

    // MDL (Memory Descriptor List) Linking @ 0x20
    let md_v: [u64; 5] = [LPB_VBASE + 0x7000, LPB_VBASE + 0x7100, LPB_VBASE + 0x7200, LPB_VBASE + 0x7300, LPB_VBASE + 0x7400];
    let md_p: [u64; 5] = [LPB_PBASE + 0x7000, LPB_PBASE + 0x7100, LPB_PBASE + 0x7200, LPB_PBASE + 0x7300, LPB_PBASE + 0x7400];
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[0], md_p[0], 0x0, 0x1000, 0).ok(); 
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[1], md_p[1], 0x1000, 0x4000000 - 0x1000, 8).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[2], md_p[2], 0x4000000, 0x2000000, 12).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[3], md_p[3], 0x8100000, 0x100000, 11).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[4], md_p[4], 0xA000000, MEM_SIZE as u64 - 0xA000000, 1).ok();

    vm.write_memory(LPB_PBASE as usize + 0x20, &md_v[0].to_le_bytes()).ok(); // Memory Flink
    vm.write_memory(LPB_PBASE as usize + 0x28, &md_v[4].to_le_bytes()).ok(); // Memory Blink
    for i in 0..5 {
        let next_v = if i == 4 { LPB_VBASE + 0x20 } else { md_v[i+1] };
        let prev_v = if i == 0 { LPB_VBASE + 0x20 } else { md_v[i-1] };
        vm.write_memory(md_p[i] as usize, &next_v.to_le_bytes()).ok();
        vm.write_memory(md_p[i] as usize + 8, &prev_v.to_le_bytes()).ok();
    }

      // InMemoryOrder (0x10), InInitializationOrder (0x20) 추가 연결
    for offset in [0x10, 0x20] {
        let off = offset as u64;
        vm.write_memory(m1_p as usize + offset, &(m2_v + off).to_le_bytes()).ok();
        vm.write_memory(m1_p as usize + offset + 8, &(LPB_VBASE + 0x10 + off).to_le_bytes()).ok();
        vm.write_memory(m2_p as usize + offset, &(LPB_VBASE + 0x10 + off).to_le_bytes()).ok();
        vm.write_memory(m2_p as usize + offset + 8, &(m1_v + off).to_le_bytes()).ok();
    }

    debug::setup_diagnostic_idt(&mut vm).expect("IDT failed");

    if let Err(e) = cpu::run(&mut vm, krnl_entry_v, stack_v, LPB_VBASE) {
        eprintln!("Error: {}", e);
    }
}

fn pe_entry_rva(pe: &pe::PeFile) -> u64 {
    if pe.entry_point >= pe.image_base { pe.entry_point - pe.image_base } else { pe.entry_point }
}

// old setup_kernel_paging function
/* 
fn setup_kernel_paging(vm: &mut Vm, krnl_base: u64, hal_base: u64) -> Result<(), &'static str> {
    let pml4_p        = SYSTEM_BASE + 0x2000;
    
    // [핵심] High Kernel Area(PML4 511)를 담당할 단 하나의 PDPT
    let pdpt_high_p   = SYSTEM_BASE + 0x3000; 
    
    let pdpt_stack_p  = SYSTEM_BASE + 0x4000; 
    let pdpt_user_p   = SYSTEM_BASE + 0x5000; 
    let pd_ib_p       = SYSTEM_BASE + 0xD000;   // 커널용 PD
    let pd_hal_dll_p  = SYSTEM_BASE + 0x14000;  // HAL용 PD
    
    let pd_stack_p    = SYSTEM_BASE + 0xB000;
    let pd_user_p     = SYSTEM_BASE + 0xC000;
    let bridge_p      = SYSTEM_BASE + 0x50000;

    let nx: u64 = 1 << 63; // [NEW] No-Execute bit

    // GDT/IDT 영역을 kernel PD에 huge page로 매핑 (SYSTEM_BASE phys)
    let gdt_virt: u64 = 0xfffff80008000000;
    let gdt_pd_idx = ((gdt_virt >> 21) & 0x1ff) as usize;
    // PD entry에 SYSTEM_BASE phys + huge page flag
    vm.write_memory((pd_ib_p + gdt_pd_idx as u64 * 8) as usize, 
    &((SYSTEM_BASE | 0x83 | nx).to_le_bytes()))?;  // [FIX] NX 적용 (GDT는 데이터임)

    // 1. 초기화
    vm.write_memory(SYSTEM_BASE as usize, &[0u8; 524288])?; 
    vm.write_memory(bridge_p as usize, &[0x0F, 0x01, 0xC1]).ok(); 

    // 2. PML4 설정 (권한 및 NX 정규화)
    vm.write_memory((pml4_p + 511*8) as usize, &((pdpt_high_p | 0x3).to_le_bytes()))?;
    vm.write_memory((pml4_p + 509*8) as usize, &((pdpt_stack_p | 0x3 | nx).to_le_bytes()))?; // [FIX] NX
    vm.write_memory((pml4_p + 495*8) as usize, &((pdpt_user_p | 0x7 | nx).to_le_bytes()))?; // [FIX] User(0x4) + NX
    vm.write_memory((pml4_p + 510*8) as usize, &((pml4_p | 0x3 | nx).to_le_bytes()))?; // [FIX] NX
    vm.write_memory((pml4_p + 493*8) as usize, &((pml4_p | 0x3 | nx).to_le_bytes()))?; 

    // 3. PDPT 설정
    let ib_pdpt_idx = (krnl_base >> 30) & 0x1ff; 
    vm.write_memory((pdpt_high_p + ib_pdpt_idx*8) as usize, &((pd_ib_p | 0x3).to_le_bytes()))?;
    let hal_pdpt_idx = (hal_base >> 30) & 0x1ff; 
    vm.write_memory((pdpt_high_p + hal_pdpt_idx*8) as usize, &((pd_hal_dll_p | 0x3).to_le_bytes()))?;

    // 4. PD 설정
    // 커널 코드는 NX를 적용하지 않음 (실행 가능)
    for j in 0..64 {
        let phys = KRNL_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_ib_p + j as u64 * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    let hal_pd_start_idx = ((hal_base >> 21) & 0x1ff) as u64;
    for j in 0..32 {
        let phys = HAL_PBASE + (j as u64 * 0x200000);
        let entry_paddr = pd_hal_dll_p + (hal_pd_start_idx + j as u64) * 8;
        vm.write_memory(entry_paddr as usize, &((phys | 0x83).to_le_bytes()))?;
    }
    vm.write_memory((pd_hal_dll_p + 511*8) as usize, &((0xFEE00000u64 | 0x93 | nx).to_le_bytes()))?; // [FIX] NX

    // 스택 매핑 (NX 적용)
    vm.write_memory(pdpt_stack_p as usize, &((pd_stack_p | 0x3 | nx).to_le_bytes()))?;
    for j in 0..512 {
        let phys = STACK_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_stack_p + j as u64 * 8) as usize, &((phys | 0x83 | nx).to_le_bytes()))?;
    }

    // KUSER 매핑 (User 권한 + NX 적용)
    vm.write_memory(pdpt_user_p as usize, &((pd_user_p | 0x7 | nx).to_le_bytes()))?;
    for j in 0..512 {
        let phys = KUSER_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_user_p + j as u64 * 8) as usize, &((phys | 0x87 | nx).to_le_bytes()))?;
    }

    Ok(())
} 

*/

fn setup_kernel_paging(vm: &mut Vm, krnl_base: u64, hal_base: u64) -> Result<(), &'static str> {
    let pml4_p        = SYSTEM_BASE + 0x2000;
    let pdpt_high_p   = SYSTEM_BASE + 0x3000; 
    let pdpt_low_p    = SYSTEM_BASE + 0x6000;   // [추가] 낮은 주소(HAL)용 PDPT
    let pd_kernel_p   = SYSTEM_BASE + 0xD000;   // 커널/LPB/GDT 통합 PD
    let pd_hal_p      = SYSTEM_BASE + 0x14000; 
    
    let pdpt_stack_p  = SYSTEM_BASE + 0x4000; 
    let pd_stack_p    = SYSTEM_BASE + 0xB000;
    let pdpt_user_p   = SYSTEM_BASE + 0x5000; 
    let pd_user_p     = SYSTEM_BASE + 0xC000;
    let bridge_p      = SYSTEM_BASE + 0x50000;

    let nx: u64 = 1 << 63;

    // 1. [순서 중요] 먼저 메모리를 0으로 초기화합니다.
    vm.write_memory(SYSTEM_BASE as usize, &[0u8; 524288])?; 
    vm.write_memory(bridge_p as usize, &[0x0F, 0x01, 0xC1]).ok(); 

    // 2. PML4 설정
    vm.write_memory((pml4_p + 511*8) as usize, &((pdpt_high_p | 0x3).to_le_bytes()))?;
    vm.write_memory(pml4_p as usize, &((pdpt_low_p | 0x3).to_le_bytes()))?; // Index 0 (HAL용)
    vm.write_memory((pml4_p + 509*8) as usize, &((pdpt_stack_p | 0x3 | nx).to_le_bytes()))?;
    vm.write_memory((pml4_p + 495*8) as usize, &((pdpt_user_p | 0x7 | nx).to_le_bytes()))?;
    vm.write_memory((pml4_p + 510*8) as usize, &((pml4_p | 0x3 | nx).to_le_bytes()))?;

    // 3. PDPT 설정
    // High Area (0xFFFFF800_00000000 ~)
    vm.write_memory(pdpt_high_p as usize, &((pd_kernel_p | 0x3).to_le_bytes()))?;
    
    // Low Area (HAL: 0x1c0000000 -> Index 7)
    let hal_pdpt_idx = (hal_base >> 30) & 0x1ff;
    vm.write_memory((pdpt_low_p + hal_pdpt_idx * 8) as usize, &((pd_hal_p | 0x3).to_le_bytes()))?;

    // 4. PD 설정 (물리 주소 0 ~ 1GB를 커널 가상 영역에 통째로 매핑)
    // 이렇게 하면 KRNL_PBASE, LPB_PBASE, SYSTEM_BASE(GDT)가 자동으로 제자리에 매핑됩니다.
    for j in 0..512 {
        let phys = j as u64 * 0x200000;
        vm.write_memory((pd_kernel_p + j * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    // 5. HAL 전용 PD 설정 (물리 주소 HAL_PBASE를 가상 주소 hal_base에 매핑)
    let hal_pd_idx = (hal_base >> 21) & 0x1ff;
    for j in 0..32 {
        let phys = HAL_PBASE + (j as u64 * 0x200000);
        let entry_addr = pd_hal_p + (hal_pd_idx + j as u64) * 8;
        vm.write_memory(entry_addr as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    // APIC 매핑 (기존 유지)
    vm.write_memory((pd_hal_p + 511*8) as usize, &((0xFEE00000u64 | 0x93 | nx).to_le_bytes()))?;

    // 6. 스택 및 KUSER 매핑 (기존 로직 유지)
    vm.write_memory(pdpt_stack_p as usize, &((pd_stack_p | 0x3 | nx).to_le_bytes()))?;
    for j in 0..512 {
        let phys = STACK_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_stack_p + j as u64 * 8) as usize, &((phys | 0x83 | nx).to_le_bytes()))?;
    }
    vm.write_memory(pdpt_user_p as usize, &((pd_user_p | 0x7 | nx).to_le_bytes()))?;
    for j in 0..512 {
        let phys = KUSER_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_user_p + j as u64 * 8) as usize, &((phys | 0x87 | nx).to_le_bytes()))?;
    }

    Ok(())
}