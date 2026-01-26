use crate::pe::PeFile;
use crate::vm::Vm;

/// PE 섹션을 VM 메모리에 로드합니다.
/// 기본적으로 0x100000 (1MB) 위치를 기준(Base)으로 로드합니다.
/// 리턴값: 로드된 엔트리 포인트의 물리 주소 (Guest Physical Address)
pub fn load_sections(vm: &mut Vm, pe_file: &PeFile) -> Result<u64, &'static str> {
    // 커널/프로그램을 로드할 기준 물리 주소 (1MB 지점)
    // 0x0~0x1000은 보통 IDT/GDT나 리얼모드 영역으로 쓰이므로 피함.
    const LOAD_BASE: u64 = 0x100000;

    for section in &pe_file.sections {
        // PE 파일 내의 가상 주소(VA)에서 이미지 베이스를 빼서 RVA(Relative Virtual Address)를 구함
        let rva = if section.virtual_address >= pe_file.image_base {
            section.virtual_address - pe_file.image_base
        } else {
            // 만약 이미 RVA로 되어있거나 이상한 경우, 그대로 사용 (안전장치)
            section.virtual_address
        };

        // 실제 VM 메모리에 쓸 물리 주소 계산
        let phys_addr = LOAD_BASE + rva;
        let data = &section.raw_data;

        // 섹션 데이터 쓰기
        vm.write_memory(phys_addr as usize, data)?;
    }

    // 엔트리 포인트 계산
    let entry_rva = if pe_file.entry_point >= pe_file.image_base {
        pe_file.entry_point - pe_file.image_base
    } else {
        pe_file.entry_point
    };

    let entry_phys = LOAD_BASE + entry_rva;
    Ok(entry_phys)
}