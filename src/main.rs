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
pub const KPCR_PBASE: u64  = LPB_PBASE + 0xae80; 
pub const ACPI_PBASE: u64  = 0x5000000;
pub const KUSER_PBASE: u64 = 0x9000000; 
pub const STACK_PBASE: u64 = SYSTEM_BASE + 0x100000; 

// --- Virtual Memory Layout (High-Half) ---
const K_VIRT_ANY: u64  = 0xFFFFF80000000000;
const LPB_VBASE: u64   = K_VIRT_ANY + LPB_PBASE; 
const KPCR_VBASE: u64  = LPB_VBASE + 0xae80; 
const ACPI_VBASE: u64  = K_VIRT_ANY + ACPI_PBASE;
const STACK_VBASE: u64 = K_VIRT_ANY + STACK_PBASE;

fn main() {
    let args: Vec<String> = env::args().collect();
    let sys32_path = if args.len() > 1 { &args[1] } else { "./KrnlFile" };

    println!("-----Initializing VWFL Hypervisor-----");
    let mut vm = Vm::new().expect("Failed VM");

    let krnl_buf = fs::read(format!("{}/ntoskrnl.exe", sys32_path)).expect("Read krnl");
    let krnl_pe = pe::parse(&krnl_buf).expect("Parse krnl");
    let hal_buf = fs::read(format!("{}/hal.dll", sys32_path)).expect("Read hal");
    let hal_pe = pe::parse(&hal_buf).expect("Parse hal");

    setup_kernel_paging(&mut vm, krnl_pe.image_base, hal_pe.image_base).expect("Paging failed");

    loader::load_sections(&mut vm, &krnl_pe, KRNL_PBASE).expect("Load krnl");
    loader::load_sections(&mut vm, &hal_pe, HAL_PBASE).expect("Load hal");

    let krnl_entry_v = krnl_pe.image_base + pe_entry_rva(&krnl_pe);
    let stack_v: u64 = STACK_VBASE + 0x10000; 

    LoaderParameterBlock::setup(&mut vm, LPB_VBASE, LPB_PBASE, KPCR_VBASE + 0x180, stack_v).expect("LPB Init");
    Kpcr::setup(&mut vm, KPCR_VBASE, KPCR_PBASE).expect("KPCR Init");

    let rsdp_v = acpi::setup(&mut vm, ACPI_PBASE, ACPI_VBASE).expect("ACPI failed");
    LoaderParameterBlock::set_acpi(&mut vm, LPB_PBASE, rsdp_v).ok();

    let gdt_v: u64 = K_VIRT_ANY + SYSTEM_BASE;
    let idt_v: u64 = K_VIRT_ANY; 
    let tss_v: u64 = gdt_v + 0x1000;
    LoaderParameterBlock::set_hardware_tables(&mut vm, LPB_PBASE, gdt_v, idt_v, tss_v).ok();

    // 모듈 리스트 연결
    let m1_v = LPB_VBASE + 0x5000; let m1_p = LPB_PBASE + 0x5000;
    let m2_v = LPB_VBASE + 0x6000; let m2_p = LPB_PBASE + 0x6000;
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, m1_v, m1_p, krnl_pe.image_base, krnl_entry_v, 0x2000000, "ntoskrnl.exe").ok();
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, m2_v, m2_p, 0x180000000u64, 0x180000000u64, 0x800000, "hal.dll").ok();
    
    vm.write_memory(LPB_PBASE as usize, &m1_v.to_le_bytes()).ok();
    vm.write_memory(LPB_PBASE as usize + 8, &m2_v.to_le_bytes()).ok();
    vm.write_memory(m1_p as usize, &m2_v.to_le_bytes()).ok();
    vm.write_memory(m1_p as usize + 8, &LPB_VBASE.to_le_bytes()).ok();
    vm.write_memory(m2_p as usize, &LPB_VBASE.to_le_bytes()).ok();
    vm.write_memory(m2_p as usize + 8, &m1_v.to_le_bytes()).ok();

    // 5분할 MDL 구성
    let md_v: [u64; 5] = [LPB_VBASE + 0x7000, LPB_VBASE + 0x7100, LPB_VBASE + 0x7200, LPB_VBASE + 0x7300, LPB_VBASE + 0x7400];
    let md_p: [u64; 5] = [LPB_PBASE + 0x7000, LPB_PBASE + 0x7100, LPB_PBASE + 0x7200, LPB_PBASE + 0x7300, LPB_PBASE + 0x7400];
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[0], md_p[0], 0x0, 0x1000, 0).ok(); 
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[1], md_p[1], 0x1000, 0x4000000 - 0x1000, 8).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[2], md_p[2], 0x4000000, 0x2000000, 12).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[3], md_p[3], 0x8100000, 0x100000, 11).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[4], md_p[4], 0xA000000, MEM_SIZE as u64 - 0xA000000, 1).ok();

    vm.write_memory(LPB_PBASE as usize + 0x10, &md_v[0].to_le_bytes()).ok();
    vm.write_memory(LPB_PBASE as usize + 0x18, &md_v[4].to_le_bytes()).ok();
    for i in 0..5 {
        let next_v = if i == 4 { LPB_VBASE + 0x10 } else { md_v[i+1] };
        let prev_v = if i == 0 { LPB_VBASE + 0x10 } else { md_v[i-1] };
        vm.write_memory(md_p[i] as usize, &next_v.to_le_bytes()).ok();
        vm.write_memory(md_p[i] as usize + 8, &prev_v.to_le_bytes()).ok();
    }

    debug::setup_diagnostic_idt(&mut vm).expect("IDT failed");

    if let Err(e) = cpu::run(&mut vm, krnl_entry_v, stack_v, LPB_VBASE) {
        eprintln!("Error: {}", e);
    }
}

fn pe_entry_rva(pe: &pe::PeFile) -> u64 {
    if pe.entry_point >= pe.image_base { pe.entry_point - pe.image_base } else { pe.entry_point }
}

fn setup_kernel_paging(vm: &mut Vm, krnl_base: u64, _hal_base: u64) -> Result<(), &'static str> {
    let pml4_p        = SYSTEM_BASE + 0x2000;
    let pdpt_high_p   = SYSTEM_BASE + 0x3000; 
    let pdpt_stack_p  = SYSTEM_BASE + 0x4000; 
    let pdpt_user_p   = SYSTEM_BASE + 0x5000; 
    let pdpt_ib_p     = SYSTEM_BASE + 0x6000; 
    
    let pd_mirror_0_p = SYSTEM_BASE + 0x7000; 
    let pd_mirror_1_p = SYSTEM_BASE + 0x8000; 
    let pd_mirror_2_p = SYSTEM_BASE + 0x9000; 
    let pd_mirror_3_p = SYSTEM_BASE + 0xA000; 
    
    let pd_stack_p    = SYSTEM_BASE + 0xB000;
    let pd_user_p     = SYSTEM_BASE + 0xC000;
    let pd_ib_p       = SYSTEM_BASE + 0xD000;
    
    // [FIX] HAL/APIC Mapping Structures (High Virtual Address)
    let pdpt_hal_p    = SYSTEM_BASE + 0xE000;
    let pd_hal_p      = SYSTEM_BASE + 0xF000;

    // [FIX] Additional Identity Mapping PDs (4GB-8GB)
    let pd_mirror_4_p = SYSTEM_BASE + 0x10000;
    let pd_mirror_5_p = SYSTEM_BASE + 0x11000;
    let pd_mirror_6_p = SYSTEM_BASE + 0x12000;
    let pd_mirror_7_p = SYSTEM_BASE + 0x13000;

    vm.write_memory(SYSTEM_BASE as usize, &[0u8; 65536])?; 

    // 1. PML4 연결
    vm.write_memory(pml4_p as usize, &((pdpt_high_p | 0x3).to_le_bytes()))?; // Index 0 (Mirror)
    vm.write_memory((pml4_p + 496*8) as usize, &((pdpt_high_p | 0x3).to_le_bytes()))?; // Index 496 (Kernel)
    vm.write_memory((pml4_p + 509*8) as usize, &((pdpt_stack_p | 0x3).to_le_bytes()))?; // Index 509 (Stack)
    vm.write_memory((pml4_p + 495*8) as usize, &((pdpt_user_p | 0x3).to_le_bytes()))?; // Index 495 (KUSER)
    
    // [FIX] Self-Reference (Index 510 & 493)
    // Windows expects Self-Ref at Index 493 (0x1ED) for Memory Manager calls.
    vm.write_memory((pml4_p + 510*8) as usize, &((pml4_p | 0x3).to_le_bytes()))?; // Index 510 (Standard)
    vm.write_memory((pml4_p + 493*8) as usize, &((pml4_p | 0x3).to_le_bytes()))?; // Index 493 (Windows Default)

    vm.write_memory((pml4_p + 511*8) as usize, &((pdpt_hal_p | 0x3).to_le_bytes()))?; // Index 511 (HAL/APIC)

    let ib_pml4_idx = (krnl_base >> 39) & 0x1ff;
    if ib_pml4_idx != 0 && ib_pml4_idx != 496 {
        vm.write_memory((pml4_p + ib_pml4_idx*8) as usize, &((pdpt_ib_p | 0x3).to_le_bytes()))?;
    }

    // 2. PDPT 연결 (Expand to 8GB)
    let pd_mirrors = [
        pd_mirror_0_p, pd_mirror_1_p, pd_mirror_2_p, pd_mirror_3_p,
        pd_mirror_4_p, pd_mirror_5_p, pd_mirror_6_p, pd_mirror_7_p
    ];
    for i in 0..8 {
        vm.write_memory((pdpt_high_p + (i as u64)*8) as usize, &((pd_mirrors[i as usize] | 0x3).to_le_bytes()))?;
    }
    vm.write_memory(pdpt_stack_p as usize, &((pd_stack_p | 0x3).to_le_bytes()))?;
    vm.write_memory(pdpt_user_p as usize, &((pd_user_p | 0x3).to_le_bytes()))?;
    // [FIX] HAL PDPT Link
    vm.write_memory((pdpt_hal_p + 511*8) as usize, &((pd_hal_p | 0x3).to_le_bytes()))?;

    if ib_pml4_idx != 0 && ib_pml4_idx != 496 {
        vm.write_memory(pdpt_ib_p as usize, &((pd_ib_p | 0x3).to_le_bytes()))?;
    }

    // 3. PD: 8GB Identity Mirror
    for i in 0..8 {
        let base_pd = pd_mirrors[i as usize];
        for j in 0..512 {
            let phys = (i as u64 * 0x40000000) + (j as u64 * 0x200000);
            vm.write_memory((base_pd + j as u64 * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
        }
    }
    
    // [FIX] HAL APIC Mapping (0xFFFFFFFFFFE00000 -> 0xFEE00000)
    // PWT(WriteThrough) + PCD(CacheDisable) + LargePage(2MB)
    vm.write_memory((pd_hal_p + 511*8) as usize, &((0xFEE00000u64 | 0x93).to_le_bytes()))?;

    // 4. PD: 커널 이미지 전용 매핑 (0x140000000 -> 0x200000)
    let ib_pdpt_idx = (krnl_base >> 30) & 0x1ff;
    let target_pdpt = if ib_pml4_idx == 496 { pdpt_high_p } else if ib_pml4_idx == 0 { pdpt_high_p } else { pdpt_ib_p };
    vm.write_memory((target_pdpt + ib_pdpt_idx*8) as usize, &((pd_ib_p | 0x3).to_le_bytes()))?;

    for j in 0..64 {
        let phys = KRNL_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_ib_p + j as u64 * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    // 5. PD: Stack 전용 매핑
    for j in 0..512 {
        let phys = STACK_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_stack_p + j as u64 * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    // 6. PD: KUSER 전용 매핑
    for j in 0..512 {
        let phys = KUSER_PBASE + (j as u64 * 0x200000);
        vm.write_memory((pd_user_p + j as u64 * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    Ok(())
}

fn verify_mapping(_vm: &Vm, _v: u64) {}