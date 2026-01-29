mod cpu;
mod debug;
mod loader;
mod loaderblock;
mod acpi;
mod pe;
mod vm;

use std::env;
use std::fs;
use std::process;
use vm::{Vm, MEM_SIZE};
use loaderblock::{LoaderParameterBlock, Kpcr};

pub const KRNL_PBASE: u64  = 0x200000;   
pub const HAL_PBASE: u64   = 0x2000000;  
pub const SYSTEM_BASE: u64 = 0x8000000;  
pub const LPB_PBASE: u64   = 0x4000000;  
pub const KPCR_PBASE: u64  = 0x6000000;  
pub const ACPI_PBASE: u64  = 0x5000000;
pub const KUSER_PBASE: u64 = 0x9000000; 

const K_VIRT_ANY: u64  = 0xFFFFF80000000000;
const LPB_VBASE: u64   = K_VIRT_ANY + LPB_PBASE; 
const KPCR_VBASE: u64  = K_VIRT_ANY + KPCR_PBASE;

fn main() {
    let args: Vec<String> = env::args().collect();
    let sys32_path = if args.len() > 1 { &args[1] } else { "./System32" };

    println!("Initializing VWFL Hypervisor (Exec Fix Mode)...");
    let mut vm = Vm::new().expect("Failed VM");

    let krnl_buf = fs::read(format!("{}/ntoskrnl.exe", sys32_path)).expect("Read krnl");
    let krnl_pe = pe::parse(&krnl_buf).expect("Parse krnl");
    let hal_buf = fs::read(format!("{}/hal.dll", sys32_path)).expect("Read hal");
    let hal_pe = pe::parse(&hal_buf).expect("Parse hal");

    setup_kernel_paging(&mut vm, krnl_pe.image_base, hal_pe.image_base).expect("Paging failed");

    loader::load_sections(&mut vm, &krnl_pe, KRNL_PBASE).expect("Load krnl");
    loader::load_sections(&mut vm, &hal_pe, HAL_PBASE).expect("Load hal");

    let krnl_entry_v = krnl_pe.image_base + pe_entry_rva(&krnl_pe);
    let hal_alias_v = 0x180000000u64;
    let hal_entry_v = hal_alias_v + pe_entry_rva(&hal_pe);

    let prcb_v: u64 = KPCR_VBASE + 0x180;
    let stack_v: u64 = K_VIRT_ANY + 0x90000;
    LoaderParameterBlock::setup(&mut vm, LPB_VBASE, LPB_PBASE, prcb_v, stack_v).expect("LPB Init");
    Kpcr::setup(&mut vm, KPCR_VBASE, KPCR_PBASE).expect("KPCR Init");

    // MachineType = 1
    vm.write_memory(LPB_PBASE as usize + 0x24, &1u32.to_le_bytes()).ok();

    let rsdp_v = acpi::setup(&mut vm, ACPI_PBASE, ACPI_PBASE).expect("ACPI failed");
    LoaderParameterBlock::set_acpi(&mut vm, LPB_PBASE, rsdp_v).ok();

    // 모듈 체인 연결
    let m1_v = LPB_VBASE + 0x5000; let m1_p = LPB_PBASE + 0x5000;
    let m2_v = LPB_VBASE + 0x6000; let m2_p = LPB_PBASE + 0x6000;
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, m1_v, m1_p, krnl_pe.image_base, krnl_entry_v, 0x2000000).ok();
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, m2_v, m2_p, hal_alias_v, hal_entry_v, 0x800000).ok();
    
    vm.write_memory(LPB_PBASE as usize, &m1_v.to_le_bytes()).ok();
    vm.write_memory(LPB_PBASE as usize + 8, &m2_v.to_le_bytes()).ok();
    vm.write_memory(m1_p as usize, &m2_v.to_le_bytes()).ok();
    vm.write_memory(m1_p as usize + 8, &LPB_VBASE.to_le_bytes()).ok();
    vm.write_memory(m2_p as usize, &LPB_VBASE.to_le_bytes()).ok();
    vm.write_memory(m2_p as usize + 8, &m1_v.to_le_bytes()).ok();

    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, LPB_VBASE + 0x7000, LPB_PBASE + 0x7000, 0, MEM_SIZE as u64, 1).ok();

    debug::setup_diagnostic_idt(&mut vm).expect("IDT failed");

    let mut regs = vm.vcpu_fd.get_regs().expect("Regs");
    regs.rax = 0; regs.rbx = 0; regs.rcx = LPB_VBASE; regs.rdx = 0; regs.r8 = 0; regs.r9 = LPB_VBASE;
    regs.rip = krnl_entry_v;
    regs.rsp = stack_v - 0x100;
    regs.rflags = 0x2;
    vm.vcpu_fd.set_regs(&regs).expect("Set Regs");

    println!("VM Ready. Kernel ImageBase (0x{:x}) Executable.", krnl_pe.image_base);

    if let Err(e) = cpu::run(&mut vm) {
        eprintln!("Error: {}", e);
    }
}

fn pe_entry_rva(pe: &pe::PeFile) -> u64 {
    if pe.entry_point >= pe.image_base { pe.entry_point - pe.image_base } else { pe.entry_point }
}

fn setup_kernel_paging(vm: &mut Vm, krnl_base: u64, hal_base: u64) -> Result<(), &'static str> {
    let pml4        = SYSTEM_BASE + 0x1000;
    let pdpt_low    = SYSTEM_BASE + 0x2000; 
    let pdpt_kuser  = SYSTEM_BASE + 0x3000; 
    let pdpt_knrl   = SYSTEM_BASE + 0x4000; 
    let pdpt_stack  = SYSTEM_BASE + 0x5000; 
    
    // [FIX] 커널 이미지를 위한 전용 PDPT 및 PD
    let pdpt_ib     = SYSTEM_BASE + 0x6000; 
    let pd_ib       = SYSTEM_BASE + 0x7000; 

    let pd_low      = SYSTEM_BASE + 0x8000; 
    let pd_knrl     = SYSTEM_BASE + 0x9000; 
    let pd_hal      = SYSTEM_BASE + 0xA000; 

    vm.write_memory(SYSTEM_BASE as usize, &[0u8; 65536])?; 

    // --- PML4 ---
    vm.write_memory(pml4 as usize, &((pdpt_low | 0x3).to_le_bytes()))?;
    vm.write_memory((pml4 + 493*8) as usize, &((pml4 | 0x3).to_le_bytes()))?; 
    vm.write_memory((pml4 + 495*8) as usize, &((pdpt_kuser | 0x3).to_le_bytes()))?;
    vm.write_memory((pml4 + 496*8) as usize, &((pdpt_knrl | 0x3).to_le_bytes()))?;
    vm.write_memory((pml4 + 511*8) as usize, &((pdpt_stack | 0x3).to_le_bytes()))?;

    // [FIX] ImageBase가 PML4[0] 영역에 걸치는지 확인하고 매핑
    // 보통 0x140000000은 PML4[0] -> PDPT[1] 이므로 pdpt_low를 재사용하되, 해당 인덱스를 확실히 채워야 함
    // 만약 ImageBase가 아주 높다면 pdpt_ib 사용
    let ib_pml4_idx = (krnl_base >> 39) & 0x1ff;
    if ib_pml4_idx == 0 {
        // PDPT[1] (0x140000000 / 1GB = 5, but PDPT index covers 1GB * 512? No. 
        // 0x140000000 = 5GB. PDPT entry covers 1GB. Index = 5.
        // Wait, PDPT entry maps 1GB (512 * 2MB).
        let ib_pdpt_idx = (krnl_base >> 30) & 0x1ff; 
        vm.write_memory((pdpt_low + ib_pdpt_idx*8) as usize, &((pd_ib | 0x3).to_le_bytes()))?;
    } else {
        vm.write_memory((pml4 + ib_pml4_idx*8) as usize, &((pdpt_ib | 0x3).to_le_bytes()))?;
        let ib_pdpt_idx = (krnl_base >> 30) & 0x1ff; 
        vm.write_memory((pdpt_ib + ib_pdpt_idx*8) as usize, &((pd_ib | 0x3).to_le_bytes()))?;
    }

    // --- PDPT 채우기 ---
    // Low/Identity (0~4GB) - 인덱스 0~3 사용
    for i in 0..4 {
        let pd_paddr = pd_low + (i as u64 * 0x1000); 
        vm.write_memory((pdpt_low + i as u64 * 8) as usize, &((pd_paddr | 0x3).to_le_bytes()))?;
        // PD 채우기
        for j in 0..512 {
            let phys = (i as u64 * 0x40000000) + (j as u64 * 0x200000);
            vm.write_memory((pd_paddr + j as u64 * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
        }
    }
    
    // KUSER, Kernel, Stack용 PDPT 채우기 (모두 pd_low 재사용하여 물리메모리 접근 허용)
    for i in 0..512 {
        let pd_paddr = pd_low + ((i % 4) as u64 * 0x1000);
        vm.write_memory((pdpt_kuser + i*8) as usize, &((pd_paddr | 0x3).to_le_bytes()))?;
        vm.write_memory((pdpt_knrl + i*8) as usize, &((pd_paddr | 0x3).to_le_bytes()))?;
        vm.write_memory((pdpt_stack + i*8) as usize, &((pd_paddr | 0x3).to_le_bytes()))?;
    }

    // [FIX] 커널 이미지 전용 PD (pd_ib) 채우기
    // 0x140000000 가상 주소를 KRNL_PBASE (0x200000) 물리 주소로 매핑
    // ImageBase의 오프셋에 맞춰 PD 엔트리 설정
    let ib_pd_idx_start = (krnl_base >> 21) & 0x1ff;
    for i in 0..64 { // 넉넉하게 128MB 매핑
        let phys = KRNL_PBASE + (i as u64 * 0x200000);
        vm.write_memory((pd_ib + (ib_pd_idx_start + i)*8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    // High-Half에도 커널 매핑 보정 (PDPT_KERNEL 내)
    let high_pdpt_idx = (0xFFFFF80000200000u64 >> 30) & 0x1ff; // KRNL_PBASE in High-Half? 
    // 사실 KRNL_PBASE 물리 주소는 이미 전수 매핑으로 커버되지만, 명시적으로 확실히 함
    let pd_high_k = SYSTEM_BASE + 0xB000;
    vm.write_memory((pdpt_knrl + 0*8) as usize, &((pd_high_k | 0x3).to_le_bytes()))?; // Base 0
    for i in 0..512 {
        let phys = i as u64 * 0x200000;
        vm.write_memory((pd_high_k + i*8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    Ok(())
}

fn verify_mapping(_vm: &Vm, _v: u64) {}