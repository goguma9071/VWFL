mod cpu;
mod loader;
mod loaderblock;
mod pe;
mod vm;

use std::env;
use std::fs;
use std::process;
use vm::{Vm, MEM_SIZE};
use loaderblock::{LoaderParameterBlock, Kpcr};

// --- 절대 겹치지 않는 안전한 메모리 맵 ---
pub const SYSTEM_BASE: u64 = 0x8000000; // 128MB: 시스템 구조체 전용
pub const KRNL_PBASE: u64  = 0x200000;  // 2MB: ntoskrnl
pub const HAL_PBASE: u64   = 0x2000000; // 32MB: hal.dll
pub const LPB_PBASE: u64   = 0x4000000; // 64MB: LPB
pub const KPCR_PBASE: u64  = 0x6000000; // 96MB: KPCR

const LPB_VBASE: u64   = 0xFFFFF80004000000; // 커널 영역 고위 주소
const KPCR_VBASE: u64  = 0xFFFFF80006000000;

fn main() {
    let args: Vec<String> = env::args().collect();
    let sys32_path = if args.len() > 1 { &args[1] } else { "./System32" };

    println!("Initializing VWFL KVM Hypervisor (Safe Layout)...");
    let mut vm = Vm::new().expect("Failed to create VM");

    let krnl_buf = fs::read(format!("{}/ntoskrnl.exe", sys32_path)).expect("Read kernel");
    let krnl_pe = pe::parse(&krnl_buf).expect("Parse Kernel");
    let hal_buf = fs::read(format!("{}/hal.dll", sys32_path)).expect("Read hal");
    let hal_pe = pe::parse(&hal_buf).expect("Parse HAL");

    // 1. 페이지 테이블 및 구조체 영역 초기화
    setup_kernel_paging(&mut vm, krnl_pe.image_base).expect("Paging failed");

    // 2. 파일 로드 (서로 멀리 떨어뜨림)
    let krnl_phys = loader::load_sections(&mut vm, &krnl_pe, KRNL_PBASE).expect("Load kernel");
    let hal_phys = loader::load_sections(&mut vm, &hal_pe, HAL_PBASE).expect("Load hal");

    let krnl_entry_v = krnl_pe.image_base + (krnl_phys - KRNL_PBASE);
    let hal_entry_v = hal_pe.image_base + (hal_phys - HAL_PBASE);

    // 3. 윈도우 구조체 설정
    LoaderParameterBlock::setup(&mut vm, LPB_VBASE, LPB_PBASE).expect("LPB Setup");
    Kpcr::setup(&mut vm, KPCR_VBASE, KPCR_PBASE).expect("KPCR Setup");

    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, LPB_VBASE + 0x1000, LPB_PBASE + 0x1000,
                                     krnl_pe.image_base, krnl_entry_v, 0x2000000).ok();
    LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, LPB_VBASE + 0x2000, LPB_PBASE + 0x2000,
                                     hal_pe.image_base, hal_entry_v, 0x800000).ok();
    LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, LPB_VBASE + 0x3000, LPB_PBASE + 0x3000, 0, MEM_SIZE as u64, 1).ok();

    setup_diagnostic_idt(&mut vm).expect("IDT Setup failed");

    let mut regs = vm.vcpu_fd.get_regs().expect("Regs");
    regs.rcx = LPB_VBASE; 
    regs.rip = krnl_entry_v;
    regs.rsp = 0x90000; 
    regs.rflags = 0x2;
    vm.vcpu_fd.set_regs(&regs).expect("Set Regs");

    println!("--- Final Mapping Check (128MB Base) ---");
    verify_mapping(&vm, krnl_entry_v);
    verify_mapping(&vm, KPCR_VBASE);

    println!("VM Ready. Kernel Entry: 0x{:x}", regs.rip);

    if let Err(e) = cpu::run(&mut vm) {
        eprintln!("Error: {}", e);
    }
}

fn setup_diagnostic_idt(vm: &mut Vm) -> Result<(), &'static str> {
    let stub_base = SYSTEM_BASE + 0x10000; 
    for i in 0..256 {
        let stub: [u8; 5] = [0xB0, i as u8, 0xE6, 0xF9, 0xF4];
        vm.write_memory((stub_base + i as u64 * 8) as usize, &stub)?;
        let mut entry = [0u8; 16];
        let h = stub_base + i as u64 * 8;
        entry[0..2].copy_from_slice(&(h as u16).to_le_bytes());
        entry[2..4].copy_from_slice(&0x10u16.to_le_bytes()); 
        entry[5] = 0x8E;
        entry[6..8].copy_from_slice(&((h >> 16) as u16).to_le_bytes());
        entry[8..12].copy_from_slice(&((h >> 32) as u32).to_le_bytes());
        vm.write_memory(i * 16, &entry)?;
    }
    Ok(())
}

fn setup_kernel_paging(vm: &mut Vm, image_base: u64) -> Result<(), &'static str> {
    let pml4 = (SYSTEM_BASE + 0x1000) as u64;
    let pdpt = (SYSTEM_BASE + 0x2000) as u64;
    let pd_ident = (SYSTEM_BASE + 0x3000) as u64;
    let pd_kernel = (SYSTEM_BASE + 0x4000) as u64;

    vm.write_memory(SYSTEM_BASE as usize, &[0u8; 4096 * 8])?;

    let pdpt_e = pdpt | 0x3;
    vm.write_memory(pml4 as usize, &pdpt_e.to_le_bytes())?; // Index 0
    vm.write_memory((pml4 + 493*8) as usize, &((pml4 | 0x3).to_le_bytes()))?; // Recursive
    vm.write_memory((pml4 + 495*8) as usize, &pdpt_e.to_le_bytes())?; // KUSER
    vm.write_memory((pml4 + 496*8) as usize, &pdpt_e.to_le_bytes())?; // Kernel/LPB
    vm.write_memory((pml4 + 511*8) as usize, &pdpt_e.to_le_bytes())?; // Stack

    let pml4_idx_img = (image_base >> 39) & 0x1ff;
    vm.write_memory((pml4 + pml4_idx_img * 8) as usize, &pdpt_e.to_le_bytes())?;

    let pd_ident_e = pd_ident | 0x3;
    let pd_kernel_e = pd_kernel | 0x3;
    vm.write_memory(pdpt as usize, &pd_ident_e.to_le_bytes())?; 
    vm.write_memory((pdpt + 511*8) as usize, &pd_ident_e.to_le_bytes())?;

    let pdpt_idx_img = (image_base >> 30) & 0x1ff;
    vm.write_memory((pdpt + pdpt_idx_img * 8) as usize, &pd_kernel_e.to_le_bytes())?;

    for i in 0..512 {
        vm.write_memory((pd_ident + i*8) as usize, &((i as u64 * 0x200000) | 0x83).to_le_bytes())?;
        vm.write_memory((pd_kernel + i*8) as usize, &(((i as u64 + 1) * 0x200000) | 0x83).to_le_bytes())?;
    }
    Ok(())
}

fn verify_mapping(vm: &Vm, virt_addr: u64) {
    let pml4_base = 0x8001000; // 128MB + 4KB
    let pml4_idx = (virt_addr >> 39) & 0x1FF;
    unsafe {
        let pml4_e = *(vm.mem_ptr.add(pml4_base + (pml4_idx as usize * 8)) as *const u64);
        if pml4_e & 1 == 0 { 
            println!("  PML4[{}] FAILED (Val: 0x{:x})", pml4_idx, pml4_e); 
            return; 
        }
        println!("  0x{:x} -> Mapping OK", virt_addr);
    }
}
