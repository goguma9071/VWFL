// src/loader.rs
 
use crate::pe::PeFile;
use crate::vm::Vm;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// 로드된 모듈의 메타데이터
pub struct LoadedModule {
    pub name: String,
    pub v_base: u64,
    pub p_base: u64,
    pub entry: u64,
    pub size: u32,
    pub pe: PeFile,
}

/// 커널 모듈 관리자
pub struct KernelLoader {
    pub modules: Vec<LoadedModule>,
}

impl KernelLoader {
    pub fn new() -> Self {
        KernelLoader {
            modules: Vec::new(),
        }
    }

    pub fn load_directory(&mut self, vm: &mut Vm, dir_path: &str, k_pbase: u64, k_vbase: u64) -> Result<(), Box<dyn std::error::Error>> {
        let mut p_cursor = k_pbase; 
        let mut v_cursor = 0xFFFFF80040000000; 

        let essentials = ["ntoskrnl.exe", "hal.dll"];
        for name in &essentials {
            let fixed_v = if name.contains("ntoskrnl") { Some(k_vbase) } else { None };
            self.load_file(vm, dir_path, name, &mut p_cursor, &mut v_cursor, fixed_v)?;
        }

        let entries = fs::read_dir(dir_path)?;
        let mut paths: Vec<_> = entries.map(|r| r.unwrap().path()).collect();
        paths.sort(); 

        for path in paths {
            if path.is_file() {
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    let lname = fname.to_lowercase();
                    if essentials.contains(&fname) || lname == "config" || (!lname.ends_with(".dll") && !lname.ends_with(".sys") && !lname.ends_with(".exe")) {
                        continue;
                    }
                    self.load_file(vm, dir_path, fname, &mut p_cursor, &mut v_cursor, None)?;
                }
            }
        }
        
        println!("[LOADER] Total {} modules loaded.", self.modules.len());
        Ok(())
    }

    fn load_file(&mut self, vm: &mut Vm, dir: &str, name: &str, p_cursor: &mut u64, v_cursor: &mut u64, fixed_vbase: Option<u64>) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = format!("{}/{}", dir, name);
        let buf = fs::read(&full_path)?;
        let pe = crate::pe::parse(&buf)?;

        let size = pe.sections.iter().map(|s| s.virtual_address + s.virtual_size - pe.image_base).max().unwrap_or(0x10000) as u32;
        let aligned_size = (size + 0x1FFFFF) & !0x1FFFFF; 

        let p_base = *p_cursor;
        *p_cursor += aligned_size as u64;
        if name == "ntoskrnl.exe" { *p_cursor = 0x2000000; } 

        let has_reloc = pe.sections.iter().any(|s| s.name == ".reloc");
        let v_base = if let Some(addr) = fixed_vbase {
            addr
        } else if !has_reloc {
            println!("[LOADER] Warning: {} has no .reloc, using preferred base 0x{:x}", name, pe.image_base);
            pe.image_base
        } else {
            let addr = *v_cursor;
            *v_cursor += aligned_size as u64;
            addr
        };

        let entry = load_sections(vm, &pe, p_base, v_base)?;
        println!("[LOAD] {:<20} | Phys: 0x{:08x} | Virt: 0x{:016x} | Size: 0x{:x}", name, p_base, v_base, size);

        self.modules.push(LoadedModule {
            name: name.to_string(),
            v_base,
            p_base,
            entry,
            size,
            pe,
        });
        
        Ok(())
    }

    /// 모든 모듈에 대해 IAT 바인딩을 수행합니다. (포워더 처리 포함)
    pub fn bind_all(&mut self, vm: &mut Vm) -> Result<(), &'static str> {
        // [FIX] Use ForwarderResolver from forwarder.rs
        let resolver = crate::forwarder::ForwarderResolver::new(&self.modules);

        for m in &self.modules {
            let imports = m.pe.get_imports()?;
            
            for imp in imports {
                for func in imp.functions {
                    if let Some(addr) = resolver.resolve(&imp.dll_name, &func.name) {
                        let iat_paddr = m.p_base + func.iat_rva as u64;
                        vm.write_memory(iat_paddr as usize, &addr.to_le_bytes())?;
                    } else {
                        if !imp.dll_name.to_uppercase().starts_with("HALEXT") {
                            // println!("[BIND] Unresolved: {}!{}", imp.dll_name, func.name);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct SymbolMap {
    symbols: HashMap<String, (u64, Option<String>)>,
}

impl SymbolMap {
    pub fn new() -> Self {
        SymbolMap { symbols: HashMap::new() }
    }

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

    pub fn resolve(&self, dll_name: &str, func_name: &str) -> Option<u64> {
        let dll_stem = Path::new(dll_name).file_stem()?.to_str()?.to_uppercase();
        let key = format!("{}.{}", dll_stem, func_name);
        self.resolve_key(&key, 0)
    }

    fn resolve_key(&self, key: &str, depth: usize) -> Option<u64> {
        if depth > 10 { return None; } 
        if let Some((addr, fwd)) = self.symbols.get(key) {
            if let Some(fwd_str) = fwd {
                return self.resolve_key(fwd_str, depth + 1);
            } else {
                return Some(*addr);
            }
        }
        None
    }
}

pub fn load_sections(vm: &mut Vm, pe_file: &PeFile, load_pbase: u64, load_vbase: u64) -> Result<u64, &'static str> {
    if !pe_file.header_data.is_empty() {
        vm.write_memory(load_pbase as usize, &pe_file.header_data)?;
    }
    for section in &pe_file.sections {
        let rva = section.virtual_address.wrapping_sub(pe_file.image_base);
        let phys_dest = load_pbase + rva;
        vm.write_memory(phys_dest as usize, &section.raw_data)?;
    }
    let _ = pe_file.apply_relocation(vm, load_pbase, load_vbase); 
    
    let entry_rva = pe_file.entry_point.wrapping_sub(pe_file.image_base);
    Ok(load_vbase + entry_rva)
}

pub fn bind_imports_phys(_vm: &mut Vm, _pe: &PeFile, _p_base: u64, _symbol_map: &SymbolMap) -> Result<(), &'static str> {
    Ok(())
}