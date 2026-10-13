// src/forwarder.rs

use crate::loader::LoadedModule;
use std::path::Path;

/// 포워더 문자열을 해석하여 실제 가상 주소(Virtual Address)를 반환하는 처리기
pub struct ForwarderResolver<'a> {
    modules: &'a [LoadedModule],
}

impl<'a> ForwarderResolver<'a> {
    pub fn new(modules: &'a [LoadedModule]) -> Self {
        Self { modules }
    }

    /// 심볼을 찾아서 주소를 반환합니다. (포워더 처리 포함)
    /// dll_name: Import하는 DLL 이름 (예: "HAL.dll")
    /// func_name: Import하는 함수 이름 (예: "KeGetCurrentIrql")
    pub fn resolve(&self, dll_name: &str, func_name: &str) -> Option<u64> {
        self.resolve_recursive(dll_name, func_name, 0)
    }

    fn resolve_recursive(&self, dll_name: &str, func_name: &str, depth: usize) -> Option<u64> {
        if depth > 10 {
            // println!("[FWD] Too deep recursion for {}!{}", dll_name, func_name);
            return None;
        }

        // 1. 대상 모듈 찾기 (확장자 제거 및 대소문자 무시 비교)
        let target_stem = Path::new(dll_name).file_stem()?.to_str()?.to_uppercase();
        
        let module = self.modules.iter().find(|m| {
            let m_stem = Path::new(&m.name).file_stem().unwrap().to_str().unwrap().to_uppercase();
            m_stem == target_stem
        })?;

        // 2. 모듈의 Export Table에서 함수 검색
        if let Ok(exports) = module.pe.get_exports() {
            // [FIX] 이름 또는 Ordinal(#123 형식)로 심볼 찾기
            let found_exp = if func_name.starts_with('#') {
                if let Ok(ord) = func_name[1..].parse::<u16>() {
                    exports.iter().find(|e| e.ordinal == ord)
                } else {
                    None
                }
            } else {
                exports.iter().find(|e| e.name == func_name)
            };

            if let Some(exp) = found_exp {
                // 3. 포워더인지 확인
                if let Some(fwd_str) = &exp.forwarder {
                    // 포워더 형식: "DLLName.FunctionName" 또는 "DLLName.#Ordinal"
                    if let Some((fwd_dll, fwd_func)) = fwd_str.split_once('.') {
                        return self.resolve_recursive(fwd_dll, fwd_func, depth + 1);
                    }
                } else {
                    // 4. 진짜 함수 주소 반환 (Module Base + RVA)
                    return Some(module.v_base + exp.rva as u64);
                }
            }
        }

        None
    }
}
