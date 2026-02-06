// src/loader.rs
 
use crate::pe::PeFile;
use crate::vm::Vm;
use std::collections::HashMap;

/// 전역 심볼 맵: (Key="MODULE.FUNCTION", Value=Address or Forwarder)
pub struct SymbolMap {
    // Value: (VirtualAddress, Option<ForwarderString>)
    symbols: HashMap<String, (u64, Option<String>)>,
}

impl SymbolMap {
    pub fn new() -> Self {
        SymbolMap {
            symbols: HashMap::new(),
        }
    }

    /// 모듈의 수출 정보를 수집하여 맵에 등록합니다.
    /// module_name: "NTOSKRNL", "HAL" 등 (확장자 제외, 대문자 권장)
    pub fn collect_exports(&mut self, module_name: &str, pe: &PeFile, base_addr: u64) -> Result<(), &'static str> {
        let exports = pe.get_exports()?;
        let mod_upper = module_name.to_uppercase();

        for exp in exports {
            let key = format!("{}.{}", mod_upper, exp.name);
            let addr = base_addr + exp.rva as u64;
            self.symbols.insert(key, (addr, exp.forwarder));
        }
        Ok(())
    }

    /// 심볼의 최종 주소를 찾습니다. (포워더 처리 포함)
    pub fn resolve(&self, dll_name: &str, func_name: &str) -> Option<u64> {
        // 1. 기본 검색 키 생성 (확장자 제거 및 대문자화)
        let dll_stem = std::path::Path::new(dll_name)
            .file_stem()?.to_str()?.to_uppercase();
        
        let key = format!("{}.{}", dll_stem, func_name);
        self.resolve_key(&key, 0)
    }

    fn resolve_key(&self, key: &str, depth: usize) -> Option<u64> {
        if depth > 10 { return None; } // 순환 참조 방지

        if let Some((addr, fwd)) = self.symbols.get(key) {
            if let Some(fwd_str) = fwd {
                // Forwarder 발견! (예: "NTOSKRNL.KiService")
                // 포워더 문자열이 곧 Key 형식이므로 바로 재귀 검색
                return self.resolve_key(fwd_str, depth + 1);
            } else {
                return Some(*addr);
            }
        }
        None
    }
}

// ----------------------------------------------------------------------------
// Existing Loader Function
// ----------------------------------------------------------------------------

pub fn load_sections(
    vm: &mut Vm, 
    pe_file: &PeFile, 
    load_pbase: u64, 
    load_vbase: u64
) -> Result<u64, &'static str> {
    
    println!("[LOADER] Loading image to Phys: 0x{:x}, Virt: 0x{:x}", load_pbase, load_vbase);

    // 1. PE 헤더 로드
    if !pe_file.header_data.is_empty() {
        vm.write_memory(load_pbase as usize, &pe_file.header_data)?;
    }

    // 2. 섹션 로드
    for section in &pe_file.sections {
        let rva = section.virtual_address.wrapping_sub(pe_file.image_base);
        let phys_dest = load_pbase + rva;
        vm.write_memory(phys_dest as usize, &section.raw_data)?;
    }

    // 3. 베이스 재배치
    pe_file.apply_relocation(vm, load_pbase, load_vbase)?;

    // 4. 엔트리 포인트 계산
    let entry_rva = pe_file.entry_point.wrapping_sub(pe_file.image_base);
    let virtual_entry_point = load_vbase + entry_rva;

    println!("[LOADER] Done. Virtual Entry Point: 0x{:016x}", virtual_entry_point);

    Ok(virtual_entry_point)
}

/// IAT(Import Address Table)를 채워 넣습니다. (Binding)
pub fn bind_imports(
    vm: &mut Vm, 
    pe: &PeFile, 
    base_addr: u64, 
    symbol_map: &SymbolMap
) -> Result<(), &'static str> {
    let imports = pe.get_imports()?;

    for imp in imports {
        for func in imp.functions {
            // 심볼 주소 찾기
            if let Some(addr) = symbol_map.resolve(&imp.dll_name, &func.name) {
                // IAT 위치 계산 (Base + RVA)
                let iat_vaddr = base_addr + func.iat_rva as u64;
                
                // IAT에 주소 쓰기 (가상 주소에 써야 하지만, 여기선 편의상 물리 주소 변환이 필요)
                // 하지만 현재 하이퍼바이저 구조상, 로더는 '물리 주소'를 알고 있어야 씁니다.
                // bind_imports 함수는 '가상 베이스'를 받지만, 쓰기 위해서는 '물리 주소'가 필요합니다.
                // 따라서 인자로 p_base도 받아야 합니다. (아래 수정)
                
                // [주의] 이 함수 호출 시 p_base를 전달받도록 수정하거나,
                // vm.write_memory가 가상 주소를 지원하지 않는다면 변환해야 합니다.
                // 여기서는 간단히 가상->물리 변환 로직을 내장하거나 인자를 추가합니다.
                
                // 임시: IAT RVA는 섹션 내부에 있으므로, RVA -> 물리 오프셋 변환이 가능합니다.
                // 하지만 load_sections와 동일한 방식으로 계산하면 됩니다.
                let iat_rva_delta = func.iat_rva as u64; //.wrapping_sub(pe.image_base); (RVA는 이미 0 기준임)
                // 단, pe.get_imports()가 반환하는 iat_rva는 순수 RVA입니다.
                
                // IAT가 위치한 섹션을 찾아 물리 주소를 계산하는 것이 가장 안전합니다.
                // 하지만 로더가 'Load Base(Phys)'를 알고 있다면, Phys = P_Base + RVA 입니다.
                // 따라서 이 함수는 P_Base도 인자로 받아야 합니다.
            } else {
                println!("[LOADER] Warning: Unresolved Import {} from {}", func.name, imp.dll_name);
            }
        }
    }
    Ok(())
}

// [수정된 bind_imports] 물리 주소 베이스 추가
pub fn bind_imports_phys(
    vm: &mut Vm, 
    pe: &PeFile, 
    p_base: u64, // 물리 베이스
    symbol_map: &SymbolMap
) -> Result<(), &'static str> {
    let imports = pe.get_imports()?;

    for imp in imports {
        for func in imp.functions {
            if let Some(addr) = symbol_map.resolve(&imp.dll_name, &func.name) {
                // IAT의 물리 주소 = 모듈 물리 베이스 + IAT RVA
                // 주의: PE의 RVA는 ImageBase 기준이 아닌 0 기준 오프셋입니다.
                let iat_paddr = p_base + func.iat_rva as u64;
                
                // 최종 주소(가상 주소)를 IAT에 씁니다.
                vm.write_memory(iat_paddr as usize, &addr.to_le_bytes())?;
            } else {
                // 커널 로딩 시 일부 API는 HAL이나 확장 모듈에 있을 수 있습니다.
                // 심각한 경우 패닉을 일으키거나 로그만 남깁니다.
                println!("[BIND] Unresolved: {}!{}", imp.dll_name, func.name);
            }
        }
    }
    Ok(())
}
