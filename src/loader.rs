// src/loader.rs
/* use crate::pe::PeFile;
use crate::vm::Vm;

pub fn load_sections(vm: &mut Vm, pe_file: &PeFile, load_base: u64) -> Result<u64, &'static str> {
    // [FIX] Load PE Headers
    // Windows kernels need the PE header at the base of the image to find imports/exports.
    if !pe_file.header_data.is_empty() {
        vm.write_memory(load_base as usize, &pe_file.header_data)?;
    }

    for section in &pe_file.sections {
        let rva = if section.virtual_address >= pe_file.image_base {
            section.virtual_address - pe_file.image_base
        } else {
            section.virtual_address
        };

        let phys_addr = load_base + rva;
        vm.write_memory(phys_addr as usize, &section.raw_data)?;
    }

    let entry_rva = if pe_file.entry_point >= pe_file.image_base {
        pe_file.entry_point - pe_file.image_base
    } else {
        pe_file.entry_point
    };

    Ok(load_base + entry_rva)
}
*/
use crate::pe::PeFile;
use crate::vm::Vm;



pub fn load_sections(
    vm: &mut Vm, 
    pe_file: &PeFile, 
    load_pbase: u64, // 물리 메모리 로드 시작 주소 (예: KRNL_PBASE)
    load_vbase: u64  // 매핑될 가상 메모리 시작 주소 (예: 0xFFFFF80000400000)
) -> Result<u64, &'static str> {
    
    println!("[LOADER] Loading image to Phys: 0x{:x}, Virt: 0x{:x}", load_pbase, load_vbase);

    // 1. PE 헤더 로드
    // Windows 커널은 실행 중 자신의 ImageBase(헤더)를 참조하므로 반드시 로드해야 합니다.
    if !pe_file.header_data.is_empty() {
        vm.write_memory(load_pbase as usize, &pe_file.header_data)?;
    }

    // 2. 각 섹션 로드 (물리 메모리 배치)
    for section in &pe_file.sections {
        // RVA(Relative Virtual Address) 계산
        let rva = section.virtual_address.wrapping_sub(pe_file.image_base);
        let phys_dest = load_pbase + rva;

        // 실제 데이터를 물리 메모리의 계산된 위치에 복사
        vm.write_memory(phys_dest as usize, &section.raw_data)?;
        
        // [참고] 필요하다면 VirtualSize와 RawData 차이만큼 0으로 채우는 로직을 넣을 수 있습니다.
    }

    // 3. 베이스 재배치(Base Relocation) 적용
    // 섹션 데이터가 메모리에 복사된 상태에서, .reloc 섹션의 정보를 바탕으로
    // 메모리 내부의 절대 주소들을 load_vbase(새 가상 주소) 기준으로 패치합니다.
    pe_file.apply_relocation(vm, load_pbase, load_vbase)?;

    // 4. 가상 엔트리 포인트 계산 (RIP 설정용)
    // 원래의 EntryPoint에서 원래의 ImageBase를 빼고 새 가상 주소를 더합니다.
    let entry_rva = pe_file.entry_point.wrapping_sub(pe_file.image_base);
    let virtual_entry_point = load_vbase + entry_rva;

    println!("[LOADER] Done. Virtual Entry Point: 0x{:016x}", virtual_entry_point);

    Ok(virtual_entry_point)
}