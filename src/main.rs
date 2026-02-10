mod cpu;
mod debug;
mod loader;
mod loaderblock;
mod acpi;
mod pe;
mod vm;
mod forwarder;

use std::env;
use std::fs;
use vm::{Vm, MEM_SIZE};
use loaderblock::{LoaderParameterBlock, Kpcr};
use loader::KernelLoader;

// --- Physical Memory Layout ---
pub const KRNL_PBASE: u64  = 0x200000;   
pub const HAL_PBASE: u64   = 0x2000000;  
pub const SYSTEM_BASE: u64 = 0x8000000;  
pub const LPB_PBASE: u64   = 0x4000000;  
pub const KPCR_PBASE: u64  = LPB_PBASE + 0x10000; 
pub const ACPI_PBASE: u64  = 0x5000000;
pub const KUSER_PBASE: u64 = 0x9000000; 
pub const STACK_PBASE: u64 = 0x0A000000;

// --- Virtual Memory Layout (High-Half) ---
const K_VIRT_ANY: u64  = 0xFFFFF80000000000;
const LPB_VBASE: u64   = K_VIRT_ANY + LPB_PBASE; 
const KPCR_VBASE: u64  = LPB_VBASE + 0x10000; 
const ACPI_VBASE: u64  = K_VIRT_ANY + ACPI_PBASE;
const STACK_VIRT_BASE: u64 = 0xFFFFFE8000000000; 
const STACK_VBASE: u64 = STACK_VIRT_BASE;

fn main() {
    let args: Vec<String> = env::args().collect();
    let sys32_path = if args.len() > 1 { &args[1] } else { "./KrnlFile" };

    println!("-----Initializing VWFL Hypervisor-----");
    let mut vm = Vm::new().expect("Failed VM");

    // Initialize KUSER_SHARED_DATA
    let mut kuser_data = [0u8; 4096];
    kuser_data[0x26c..0x270].copy_from_slice(&19041u32.to_le_bytes()); 
    kuser_data[0x270..0x274].copy_from_slice(&10u32.to_le_bytes());    
    vm.write_memory(KUSER_PBASE as usize, &kuser_data).ok();

    // 1. 커널 로더 초기화 및 모든 모듈 로드 (자동 주소 할당)
    let mut kloader = KernelLoader::new();
    let krnl_vbase = 0xFFFFF80000200000;
    
    kloader.load_directory(&mut vm, sys32_path, KRNL_PBASE, krnl_vbase).expect("Failed to load modules");

    // ntoskrnl과 hal의 정보 추출
    let krnl_mod = &kloader.modules[0]; 
    let hal_mod = &kloader.modules[1];
    
    let krnl_entry_v = krnl_mod.entry;
    let hal_vbase = hal_mod.v_base;

    // 2. 페이지 테이블 구축
    setup_kernel_paging(&mut vm, krnl_mod.v_base, hal_mod.v_base).expect("Paging failed");

    // 3. SYSTEM 하이브 로드
    let sys_hive = fs::read(format!("{}/config/SYSTEM", sys32_path)).expect("Read SYSTEM Hive");
    let hive_size = sys_hive.len() as u32;
    let hive_p = 0x2C00000; 
    let hive_v = 0xFFFFF80045000000; 
    vm.write_memory(hive_p, &sys_hive).expect("Write Hive");
    println!("[DEBUG] SYSTEM Hive size: 0x{:x}", hive_size);
    
    // Hive Mapping
    let paging_pbase  = SYSTEM_BASE + 0x100000; 
    let pd_hal_p = paging_pbase + 0x14000;
    let hive_pd_idx = (hive_v >> 21) & 0x1ff;   

    // [FIX] Map one extra page before the hive for safety (Prevents fault at 44FFFE98)
    if hive_pd_idx > 0 {
        let entry_addr = pd_hal_p + (hive_pd_idx - 1) * 8;
        vm.write_memory(entry_addr as usize, &((hive_p as u64 | 0x83).to_le_bytes())).ok();
    }

    for j in 0..((hive_size as u64 + 0x1FFFFF) / 0x200000) {
        let phys = hive_p + (j * 0x200000) as usize;
        let entry_addr = pd_hal_p + (hive_pd_idx as u64 + j) * 8;
        vm.write_memory(entry_addr as usize, &((phys as u64 | 0x83).to_le_bytes())).ok();
    }

    let stack_v: u64 = STACK_VBASE + 0x10000; 

    // 4. LPB 및 KPCR 초기화
    LoaderParameterBlock::setup(&mut vm, LPB_VBASE, LPB_PBASE, KPCR_VBASE + 0x180, stack_v, hive_v, hive_size).expect("LPB Init");
    Kpcr::setup(&mut vm, KPCR_VBASE, KPCR_PBASE).expect("KPCR Init");

    let rsdp_v = acpi::setup(&mut vm, ACPI_PBASE, ACPI_VBASE).expect("ACPI failed");
    LoaderParameterBlock::set_acpi(&mut vm, LPB_PBASE, rsdp_v).ok();

    // 5. LPB 모듈 리스트 등록
    println!("\n----- KERNEL MODULE MEMORY MAP -----");
    let mut nodes = Vec::new();
    for (i, m) in kloader.modules.iter().enumerate() {
        // [FIX] Move nodes to 0x40000 to avoid collision with KPCR(0x10000) and MDL(0x20000)
        let offset = 0x40000 + (i as u64 * 0x1000); 
        let node_v = LPB_VBASE + offset;
        let node_p = LPB_PBASE + offset;
        
        println!("[MAP] {:<20} | 0x{:016x} - 0x{:016x} | Node: 0x{:x}", m.name, m.v_base, m.v_base + m.size as u64, node_v);
        
        LoaderParameterBlock::add_module(&mut vm, LPB_VBASE, LPB_PBASE, node_v, node_p, m.v_base, m.entry, m.size, &m.name).ok();
        nodes.push((node_v, node_p));
    }
    println!("------------------------------------\n");

    // 6. 순환 리스트 연결
    let head_v = LPB_VBASE + 0x00; 
    vm.write_memory(LPB_PBASE as usize + 0x00, &nodes[0].0.to_le_bytes()).ok(); // Head.Flink
    vm.write_memory(LPB_PBASE as usize + 0x08, &nodes.last().unwrap().0.to_le_bytes()).ok(); // Head.Blink

    for i in 0..nodes.len() {
        let next_v = if i == nodes.len() - 1 { head_v } else { nodes[i+1].0 };
        let prev_v = if i == 0 { head_v } else { nodes[i-1].0 };
        vm.write_memory(nodes[i].1 as usize, &next_v.to_le_bytes()).ok();
        vm.write_memory((nodes[i].1 + 8) as usize, &prev_v.to_le_bytes()).ok();

        // 내부 링크(0x10, 0x20)도 순환 연결
        for off in [0x10, 0x20] {
            let next_idx = if i == nodes.len() - 1 { 0 } else { i + 1 };
            let prev_idx = if i == 0 { nodes.len() - 1 } else { i - 1 };
            
            let next_link = nodes[next_idx].0 + off as u64;
            let prev_link = nodes[prev_idx].0 + off as u64;
            
            vm.write_memory((nodes[i].1 + off) as usize, &next_link.to_le_bytes()).ok();
            vm.write_memory((nodes[i].1 + off + 8) as usize, &prev_link.to_le_bytes()).ok();
        }
    }

    // 7. IAT 바인딩 (ForwarderResolver 사용)
    println!("[LOADER] Binding modules (IAT Patching with Forwarder support)...");
    kloader.bind_all(&mut vm).expect("Binding failed");

    // 8. GDT/TSS 설정
    let tss_p = SYSTEM_BASE + 0x1000;
    vm.write_memory(tss_p as usize, &[0u8; 104]).expect("Write TSS");
    let mut gdt_entries = vec![0u64; 32];
    gdt_entries[2] = 0x00AF9A000000FFFF;
    gdt_entries[3] = 0x00CF92000000FFFF;
    gdt_entries[4] = 0x00AFFA000000FFFF;
    gdt_entries[5] = 0x00CFF2000000FFFF;
    gdt_entries[6] = 0x00AFFA000000FFFF;
    let tss_low = (0x00 << 56) | (0x00 << 52) | (0x89 << 40) | ((tss_p & 0xFFFFFF) << 16) | (0x67);
    let tss_high = tss_p >> 32;
    gdt_entries[8] = tss_low; gdt_entries[9] = tss_high;
    for (i, entry) in gdt_entries.iter().enumerate() {
        vm.write_memory((SYSTEM_BASE + i as u64 * 8) as usize, &entry.to_le_bytes()).ok();
    }

    let gdt_v: u64 = K_VIRT_ANY + SYSTEM_BASE;
    let idt_v: u64 = gdt_v + 0x20000; 
    let tss_v: u64 = gdt_v + 0x1000;
    LoaderParameterBlock::set_hardware_tables(&mut vm, LPB_PBASE, gdt_v, idt_v, tss_v).ok();

    // 9. MDL 리스트 연결
    let mem_head_v = LPB_VBASE + 0x10;
    let md_v: [u64; 5] = [LPB_VBASE + 0x20000, LPB_VBASE + 0x21000, LPB_VBASE + 0x22000, LPB_VBASE + 0x23000, LPB_VBASE + 0x24000];
    let md_p: [u64; 5] = [LPB_PBASE + 0x20000, LPB_PBASE + 0x21000, LPB_PBASE + 0x22000, LPB_PBASE + 0x23000, LPB_PBASE + 0x24000];
    let base_map: [u64; 5] = [0x0, 0x1000, 0x4000000, 0x8100000, 0xA000000];
    let size_map: [u64; 5] = [0x1000, 0x4000000 - 0x1000, 0x2000000, 0x100000, MEM_SIZE as u64 - 0xA000000];
    let type_map: [u32; 5] = [0, 8, 12, 11, 1];
    for i in 0..5 {
        LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[i], md_p[i], base_map[i], size_map[i], type_map[i]).ok();
        let n_v = if i == 4 { mem_head_v } else { md_v[i+1] };
        let p_v = if i == 0 { mem_head_v } else { md_v[i-1] };
        vm.write_memory(md_p[i] as usize, &n_v.to_le_bytes()).ok();
        vm.write_memory((md_p[i] + 8) as usize, &p_v.to_le_bytes()).ok();
    }
    vm.write_memory(LPB_PBASE as usize + 0x10, &md_v[0].to_le_bytes()).ok(); 
    vm.write_memory(LPB_PBASE as usize + 0x18, &md_v[4].to_le_bytes()).ok(); 

    debug::setup_diagnostic_idt(&mut vm).expect("IDT failed");

    let mut verify_code = [0u8; 16];
    vm.read_memory(0xb92010, &mut verify_code).ok();
    println!("[CHECK] Code at Entry (Phys 0xB92010): {:02X?}", verify_code);

    if let Err(e) = cpu::run(&mut vm, krnl_entry_v, stack_v, LPB_VBASE) {
        eprintln!("Error: {}", e);
    }
}

fn pe_entry_rva(pe: &pe::PeFile) -> u64 {
    if pe.entry_point >= pe.image_base { pe.entry_point - pe.image_base } else { pe.entry_point }
}

fn setup_kernel_paging(vm: &mut Vm, krnl_base: u64, hal_base: u64) -> Result<(), &'static str> {
    let paging_pbase  = SYSTEM_BASE + 0x100000; 
    let pml4_p        = paging_pbase + 0x2000;
    let pdpt_high_p   = paging_pbase + 0x3000; 
    let pdpt_low_p    = paging_pbase + 0x6000;   
    let pd_kernel_p   = paging_pbase + 0xD000;   
    let pd_hal_p      = paging_pbase + 0x14000; 
    let pdpt_stack_p  = paging_pbase + 0x4000; 
    let pd_stack_p    = paging_pbase + 0xB000;
    let pdpt_user_p   = paging_pbase + 0x5000; 
    let pd_user_p     = paging_pbase + 0xC000;
    let bridge_p      = paging_pbase + 0x50000;
    let krnl_pml4_idx = 496; 
    let nx: u64 = 1 << 63;

    vm.write_memory(paging_pbase as usize, &[0u8; 524288])?; 
    vm.write_memory(bridge_p as usize, &[0x0F, 0x01, 0xC1]).ok(); 

    vm.write_memory((pml4_p + krnl_pml4_idx * 8) as usize, &((pdpt_high_p | 0x3).to_le_bytes()))?;
    vm.write_memory((pml4_p + 511 * 8) as usize, &((pdpt_high_p | 0x3).to_le_bytes()))?; 
    vm.write_memory(pml4_p as usize, &((pdpt_low_p | 0x3).to_le_bytes()))?; 
    vm.write_memory((pml4_p + 509*8) as usize, &((pdpt_stack_p | 0x3 | nx).to_le_bytes()))?;
    vm.write_memory((pml4_p + 495*8) as usize, &((pdpt_user_p | 0x7 | nx).to_le_bytes()))?;
    vm.write_memory((pml4_p + 510*8) as usize, &((pml4_p | 0x3 | nx).to_le_bytes()))?;
    vm.write_memory((pml4_p + 493*8) as usize, &((pml4_p | 0x3 | nx).to_le_bytes()))?; 

    vm.write_memory(pdpt_high_p as usize, &((pd_kernel_p | 0x3).to_le_bytes()))?;
    vm.write_memory((pdpt_high_p + 8) as usize, &((pd_hal_p | 0x3).to_le_bytes()))?; 

    // [FIX] Map low virtual address range (0x180000000~0x1FFFFFFFF)
    vm.write_memory((pdpt_low_p + 6 * 8) as usize, &((pd_hal_p | 0x3).to_le_bytes()))?;
    vm.write_memory((pdpt_low_p + 7 * 8) as usize, &((pd_hal_p | 0x3).to_le_bytes()))?;

    for j in 0..512 {
        let phys = j as u64 * 0x200000;
        vm.write_memory((pd_kernel_p + j * 8) as usize, &((phys | 0x83).to_le_bytes()))?;
    }

    let hal_pd_idx = (hal_base >> 21) & 0x1ff;
    for j in 0..32 {
        let phys = HAL_PBASE + (j as u64 * 0x200000);
        let entry_addr = pd_hal_p + (hal_pd_idx + j as u64) * 8;
        vm.write_memory(entry_addr as usize, &((phys | 0x83).to_le_bytes()))?;
    }
    vm.write_memory((pd_hal_p + 511*8) as usize, &((0xFEE00000u64 | 0x93 | nx).to_le_bytes()))?;

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