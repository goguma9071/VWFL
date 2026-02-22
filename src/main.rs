mod cpu;
mod debug;
mod loader;
mod loaderblock;
mod acpi;
mod pe;
mod vm;
mod forwarder;
mod nt_types;

use std::env;
use std::fs;
use vm::{Vm, MEM_SIZE};
use loaderblock::{LoaderParameterBlock, LoaderParameterExtension, LdrDataTableEntry, Kpcr};
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
    
    // [OFFSET CHECK] 구조체 정렬 밀림 검사
    unsafe {
        use loaderblock::{KPCR, KPRCB};
        let kpcr_ptr = std::mem::zeroed::<KPCR>();
        let prcb_ptr = std::mem::zeroed::<KPRCB>();
        let kpcr_base = &kpcr_ptr as *const _ as usize;
        let prcb_base = &prcb_ptr as *const _ as usize;
        
        println!("[VERIFY] KPCR.KdVersionBlock Offset: 0x{:x} (Target: 0x108)", (&kpcr_ptr.KdVersionBlock as *const _ as usize) - kpcr_base);
        println!("[VERIFY] KPRCB.MinorVersion Offset: 0x{:x} (Target: 0x88)", (&prcb_ptr.MinorVersion as *const _ as usize) - prcb_base);
        println!("[VERIFY] KPRCB.TscFrequency Offset: 0x{:x} (Target: 0x90)", (&prcb_ptr.TscFrequency as *const _ as usize) - prcb_base);
        println!("[VERIFY] KPRCB.ProcessorState Offset: 0x{:x} (Target: 0x100)", (&prcb_ptr.ProcessorState as *const _ as usize) - prcb_base);
    }

    let mut vm = Vm::new().expect("Failed VM");

    // Initialize KUSER_SHARED_DATA
    let mut kuser_data = [0u8; 4096];
    kuser_data[0x26c..0x270].copy_from_slice(&19041u32.to_le_bytes()); 
    kuser_data[0x270..0x274].copy_from_slice(&10u32.to_le_bytes());    
    vm.write_memory(KUSER_PBASE as usize, &kuser_data).ok();

    // 1. 커널 로더 초기화 및 모든 모듈 로드
    let mut kloader = KernelLoader::new();
    let krnl_vbase = 0xFFFFF80000200000;
    kloader.load_directory(&mut vm, sys32_path, KRNL_PBASE, krnl_vbase).expect("Failed to load modules");

    let krnl_mod = &kloader.modules[0]; 
    let hal_mod = &kloader.modules[1];
    let krnl_entry_v = krnl_mod.entry;

    // 2. 페이지 테이블 구축
    setup_kernel_paging(&mut vm, krnl_mod.v_base, hal_mod.v_base).expect("Paging failed");

    // 3. SYSTEM 하이브 로드
    let sys_hive = fs::read(format!("{}/config/SYSTEM", sys32_path)).expect("Read SYSTEM Hive");
    let hive_size = sys_hive.len() as u32;
    let hive_p = 0x4200000; // [FIX] 안전한 높은 주소로 이동
    let hive_v = 0xFFFFF80045000000; 
    vm.write_memory(hive_p, &sys_hive).expect("Write Hive");
    println!("[DEBUG] SYSTEM Hive size: 0x{:x} at Phys: 0x{:x}", hive_size, hive_p);

    // 9. MDL 리스트 연결 (7개 항목으로 확장: Hive 영역 추가 보호)
    let mem_head_v = LPB_VBASE + 0x20;
    let md_v: [u64; 7] = [
        LPB_VBASE + 0x20000, LPB_VBASE + 0x21000, LPB_VBASE + 0x22000, 
        LPB_VBASE + 0x23000, LPB_VBASE + 0x24000, LPB_VBASE + 0x25000,
        LPB_VBASE + 0x26000
    ];
    let md_p: [u64; 7] = [
        LPB_PBASE + 0x20000, LPB_PBASE + 0x21000, LPB_PBASE + 0x22000, 
        LPB_PBASE + 0x23000, LPB_PBASE + 0x24000, LPB_PBASE + 0x25000,
        LPB_PBASE + 0x26000
    ];
    
    let base_map: [u64; 7] = [0x0, 0x1000, 0x200000, 0x2000000, 0x4000000, 0x4200000, 0x4200000 + ((hive_size as u64 + 0xFFF) & !0xFFF)];
    let size_map: [u64; 7] = [
        0x1000, 0x1FF000, 0x1E00000, 0x2000000, 
        0x200000, // LPB + NLS + ApiSet (2MB)
        hive_size as u64, // [FIX] SYSTEM Hive 영역 보호
        MEM_SIZE as u64 - (0x4200000 + ((hive_size as u64 + 0xFFF) & !0xFFF))
    ];
    let type_map: [u32; 7] = [1, 0, 7, 8, 15, 15, 0]; // 15 = LoaderMemoryData

    for i in 0..7 {
        LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[i], md_p[i], base_map[i], size_map[i], type_map[i]).ok();
        
        let n_v = if i == 6 { mem_head_v } else { md_v[i+1] };
        let p_v = if i == 0 { mem_head_v } else { md_v[i-1] };
        vm.write_memory(md_p[i] as usize, &n_v.to_le_bytes()).ok();
        vm.write_memory((md_p[i] + 8) as usize, &p_v.to_le_bytes()).ok();
    }
    // LPB Head 연결
    vm.write_memory(LPB_PBASE as usize + 0x20, &md_v[0].to_le_bytes()).ok(); 
    vm.write_memory(LPB_PBASE as usize + 0x28, &md_v[6].to_le_bytes()).ok(); 
    
    // Hive Mapping
    let paging_pbase  = SYSTEM_BASE + 0x100000; 
    let pd_hal_p = paging_pbase + 0x14000;
    let hive_pd_idx = (hive_v >> 21) & 0x1ff;   

    // [RESTORE] Safety mapping before hive
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

    // 4. KPCR 및 LPB 기초 초기화
    let gdt_v: u64 = K_VIRT_ANY + SYSTEM_BASE;
    let idt_v: u64 = gdt_v + 0x20000; 
    let tss_v: u64 = gdt_v + 0x1000;
    
    // [FIX] 실제 NLS 데이터 로드 및 블록 초기화
    let nls_p_base = LPB_PBASE + 0x30000;
    let nls_v_base = LPB_VBASE + 0x30000;

    let ansi_data = fs::read(format!("{}/C_1252.NLS", sys32_path)).expect("Read ANSI NLS");
    let oem_data  = fs::read(format!("{}/C_437.NLS", sys32_path)).expect("Read OEM NLS");
    
    // [CORE FIX] 128KB 정밀 UpCase Table 생성 (a-z -> A-Z 매핑)
    let mut upcase_table = vec![0u16; 65536];
    for i in 0..65536 {
        let c = i as u16;
        if c >= 0x61 && c <= 0x7A { // 'a' - 'z'
            upcase_table[i] = c - 0x20;
        } else {
            upcase_table[i] = c;
        }
    }
    let mut upcase_bytes = Vec::with_capacity(131072);
    for val in upcase_table { upcase_bytes.extend_from_slice(&val.to_le_bytes()); }

    let ansi_v = nls_v_base + 0x1000 + 0x20;  // skip header
    let oem_v  = nls_v_base + 0x11000 + 0x20;  // also skip header
    let case_v = nls_v_base + 0x21000; // [FIX] 생성된 128KB 테이블 주소

    vm.write_memory((nls_p_base + 0x1000) as usize, &ansi_data).ok();
    vm.write_memory((nls_p_base + 0x11000) as usize, &oem_data).ok();
    vm.write_memory((nls_p_base + 0x21000) as usize, &upcase_bytes).ok(); // [FIX] 128KB 데이터 주입

    let nls_block = nt_types::NLS_DATA_BLOCK {
        AnsiCodePageData: ansi_v,
        OemCodePageData: oem_v,
        UnicodeCaseTableData: case_v,
        AppXDefaultRegion: 0,
        DefaultLocale: 0,
    };
    let nls_bytes = unsafe { std::slice::from_raw_parts(&nls_block as *const _ as *const u8, std::mem::size_of::<nt_types::NLS_DATA_BLOCK>()) };
    vm.write_memory(nls_p_base as usize, nls_bytes).ok();
    
    Kpcr::setup(&mut vm, KPCR_VBASE, KPCR_PBASE, gdt_v, idt_v, tss_v, stack_v).expect("KPCR Init");
    LoaderParameterBlock::setup(&mut vm, LPB_VBASE, LPB_PBASE, KPCR_VBASE + 0x180, stack_v, 0x10000, hive_v, hive_size, nls_v_base).expect("LPB Init");
    
    // 5. ACPI 및 Extension 초기화
    let rsdp_v = acpi::setup(&mut vm, ACPI_PBASE, ACPI_VBASE).expect("ACPI failed");
    let ext_p = LPB_PBASE + LoaderParameterExtension::OFFSET_IN_LPB;
    let ext_v = LPB_VBASE + LoaderParameterExtension::OFFSET_IN_LPB;
    LoaderParameterExtension::setup(&mut vm, ext_p, ext_v).expect("Extension Init");
    LoaderParameterExtension::set_acpi(&mut vm, ext_p, rsdp_v).ok();

    // [FIX] ApiSetSchema 로드 및 연결 (정밀 매핑: .apiset 섹션 가리킴)
    let apiset_path = format!("{}/apisetschema.dll", sys32_path);
    if let Ok(apiset_buf) = fs::read(&apiset_path) {
        let apiset_p = LPB_PBASE + 0x60000;
        let apiset_v = LPB_VBASE + 0x60000;
        vm.write_memory(apiset_p as usize, &apiset_buf).ok();
        
        // [CORE FIX] 커널은 PE 헤더가 아닌 .apiset 데이터의 시작점(RVA 0x2000)을 기대함
        let actual_schema_v = apiset_v + 0x2000; 
        let actual_schema_size = apiset_buf.len() as u32 - 0x2000;
        
        LoaderParameterExtension::set_apiset(&mut vm, ext_p, actual_schema_v, actual_schema_size).ok();
        println!("[LOADER] ApiSetSchema mapped to .apiset section (Addr: 0x{:x})", actual_schema_v);
    }

    // 6. LPB 모듈 리스트 등록 및 연결
    println!("\n----- KERNEL MODULE MEMORY MAP -----");
    let mut nodes = Vec::new();
    for (i, m) in kloader.modules.iter().enumerate() {
        let offset = 0x40000 + (i as u64 * 0x1000); 
        let node_v = LPB_VBASE + offset;
        let node_p = LPB_PBASE + offset;
        
        println!("[MAP] {:<20} | 0x{:016x} - 0x{:016x} | Node: 0x{:x}", m.name, m.v_base, m.v_base + m.size as u64, node_v);
        
        LdrDataTableEntry::add_module(&mut vm, node_v, node_p, m.v_base, m.entry, m.size, &m.name).ok();
        nodes.push((node_v, node_p));
    }
    println!("------------------------------------\n");

    // 순환 리스트 연결
    let head_v1 = LPB_VBASE + 0x10; 
    let head_v3 = LPB_VBASE + 0x30; 

    vm.write_memory(LPB_PBASE as usize + 0x10, &nodes[0].0.to_le_bytes()).ok(); 
    vm.write_memory(LPB_PBASE as usize + 0x18, &nodes.last().unwrap().0.to_le_bytes()).ok(); 
    
    if nodes.len() > 1 {
        vm.write_memory(LPB_PBASE as usize + 0x30, &(nodes[1].0 + 0x20).to_le_bytes()).ok(); 
        vm.write_memory(LPB_PBASE as usize + 0x38, &(nodes.last().unwrap().0 + 0x20).to_le_bytes()).ok(); 
    }

    for i in 0..nodes.len() {
        let next_v1 = if i == nodes.len() - 1 { head_v1 } else { nodes[i+1].0 };
        let prev_v1 = if i == 0 { head_v1 } else { nodes[i-1].0 };
        vm.write_memory(nodes[i].1 as usize, &next_v1.to_le_bytes()).ok();
        vm.write_memory((nodes[i].1 + 8) as usize, &prev_v1.to_le_bytes()).ok();

        if i > 0 {
            let next_v3 = if i == nodes.len() - 1 { head_v3 } else { nodes[i+1].0 + 0x20 };
            let prev_v3 = if i == 1 { head_v3 } else { nodes[i-1].0 + 0x20 };
            vm.write_memory((nodes[i].1 + 0x20) as usize, &next_v3.to_le_bytes()).ok();
            vm.write_memory((nodes[i].1 + 0x28) as usize, &prev_v3.to_le_bytes()).ok();
        }

        let next_v2 = if i == nodes.len() - 1 { nodes[0].0 + 0x10 } else { nodes[i+1].0 + 0x10 };
        let prev_v2 = if i == 0 { nodes.last().unwrap().0 + 0x10 } else { nodes[i-1].0 + 0x10 };
        vm.write_memory((nodes[i].1 + 0x10) as usize, &next_v2.to_le_bytes()).ok();
        vm.write_memory((nodes[i].1 + 0x18) as usize, &prev_v2.to_le_bytes()).ok();
    }

    // 7. IAT 바인딩
    println!("[LOADER] Binding modules (IAT Patching with Forwarder support)...");
    kloader.bind_all(&mut vm).expect("Binding failed");

    // 8. GDT/TSS 설정
    let tss_p = SYSTEM_BASE + 0x1000;
    vm.write_memory(tss_p as usize, &[0u8; 104]).expect("Write TSS");
    let mut gdt_entries = vec![0u64; 32];
    gdt_entries[2] = 0x00AF9A000000FFFF;
    gdt_entries[3] = 0x00CF92000000FFFF;
    // [RESTORE] User mode GDT entries
    gdt_entries[4] = 0x00AFFA000000FFFF;
    gdt_entries[5] = 0x00CFF2000000FFFF;
    gdt_entries[6] = 0x00AFFA000000FFFF;

    let tss_low = (0x00 << 56) | (0x00 << 52) | (0x89 << 40) | ((tss_p & 0xFFFFFF) << 16) | (0x67);
    let tss_high = tss_p >> 32;
    gdt_entries[8] = tss_low; gdt_entries[9] = tss_high;
    for (i, entry) in gdt_entries.iter().enumerate() {
        vm.write_memory((SYSTEM_BASE + i as u64 * 8) as usize, &entry.to_le_bytes()).ok();
    }

    // 9. MDL 리스트 연결 (7개 항목으로 확장 및 정밀 보호)
    let mem_head_v = LPB_VBASE + 0x20;
    let md_v: [u64; 7] = [
        LPB_VBASE + 0x20000, LPB_VBASE + 0x21000, LPB_VBASE + 0x22000, 
        LPB_VBASE + 0x23000, LPB_VBASE + 0x24000, LPB_VBASE + 0x25000,
        LPB_VBASE + 0x26000
    ];
    let md_p: [u64; 7] = [
        LPB_PBASE + 0x20000, LPB_PBASE + 0x21000, LPB_PBASE + 0x22000, 
        LPB_PBASE + 0x23000, LPB_PBASE + 0x24000, LPB_PBASE + 0x25000,
        LPB_PBASE + 0x26000
    ];
    
    let base_map: [u64; 7] = [0x0, 0x1000, 0x200000, 0x2000000, 0x2C00000, 0x4000000, 0x4100000];
    let size_map: [u64; 7] = [
        0x1000, 0x1FF000, 0x1E00000, 0xC00000, // HalCode (12MB)
        0x1400000, // [FIX] SYSTEM Hive (20MB) 보호 - LoaderMemoryData
        0x100000,  // LPB + NLS (1MB) 보호 - LoaderMemoryData
        MEM_SIZE as u64 - 0x4100000
    ];
    let type_map: [u32; 7] = [1, 0, 7, 8, 15, 15, 0]; // 15 = LoaderMemoryData

    for i in 0..7 {
        LoaderParameterBlock::add_memory(&mut vm, LPB_VBASE, LPB_PBASE, md_v[i], md_p[i], base_map[i], size_map[i], type_map[i]).ok();
        
        let n_v = if i == 6 { mem_head_v } else { md_v[i+1] };
        let p_v = if i == 0 { mem_head_v } else { md_v[i-1] };
        vm.write_memory(md_p[i] as usize, &n_v.to_le_bytes()).ok();
        vm.write_memory((md_p[i] + 8) as usize, &p_v.to_le_bytes()).ok();
    }
    // LPB Head 연결
    vm.write_memory(LPB_PBASE as usize + 0x20, &md_v[0].to_le_bytes()).ok(); 
    vm.write_memory(LPB_PBASE as usize + 0x28, &md_v[6].to_le_bytes()).ok(); 

    debug::setup_diagnostic_idt(&mut vm).expect("IDT failed");

    let mut verify_code = [0u8; 16];
    vm.read_memory(0xb92010, &mut verify_code).ok();
    println!("[CHECK] Code at Entry (Phys 0xB92010): {:02X?}", verify_code);

    if let Err(e) = cpu::run(&mut vm, krnl_entry_v, stack_v, LPB_VBASE) {
        eprintln!("Error: {}", e);
    }
}

// [RESTORE] PE Helper
fn pe_entry_rva(pe: &pe::PeFile) -> u64 {
    if pe.entry_point >= pe.image_base { pe.entry_point - pe.image_base } else { pe.entry_point }
}

fn setup_kernel_paging(vm: &mut Vm, _krnl_base: u64, hal_base: u64) -> Result<(), &'static str> {
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
