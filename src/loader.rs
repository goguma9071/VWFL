// src/loader.rs
 
use crate::pe::PeFile;
use crate::vm::Vm;
use std::fs;

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

        // [CORE FIX] hal.dll은 .reloc이 없는 경우가 많으므로 고정 주소 0xFFFFF80002000000 필사용
        let essentials = [
            ("ntoskrnl.exe", k_vbase), 
            ("hal.dll", 0xFFFFF80002000000),
            ("kd.dll", 0xFFFFF80000600000), // ntoskrnl 근처
            ("kdcom.dll", 0xFFFFF80000800000)
        ];
        
        for (name, fixed_v) in &essentials {
            self.load_file_fixed(vm, dir_path, name, &mut p_cursor, *fixed_v)?;
        }

        // 나머지 드라이버들 로드 (주소는 0xFFFFF80040000000 대역부터)
        let mut v_cursor_drivers = 0xFFFFF80040000000;
        let entries = fs::read_dir(dir_path)?;
        let mut paths: Vec<_> = entries.map(|r| r.unwrap().path()).collect();
        paths.sort(); 

        for path in paths {
            if path.is_file() {
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    let lname = fname.to_lowercase();
                    if essentials.iter().any(|&(e, _)| e == lname) || lname == "config" || 
                       (!lname.ends_with(".dll") && !lname.ends_with(".sys") && !lname.ends_with(".exe")) {
                        continue;
                    }
                    self.load_file_dynamic(vm, dir_path, fname, &mut p_cursor, &mut v_cursor_drivers)?;
                }
            }
        }
        
        println!("[LOADER] Total {} modules loaded.", self.modules.len());
        Ok(())
    }

    fn load_file_fixed(&mut self, vm: &mut Vm, dir: &str, name: &str, p_cursor: &mut u64, v_base: u64) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = format!("{}/{}", dir, name);
        let buf = fs::read(&full_path)?;
        let pe = crate::pe::parse(&buf)?;

        let size = pe.sections.iter().map(|s| s.virtual_address + s.virtual_size - pe.image_base).max().unwrap_or(0x10000) as u32;
        let aligned_size = (size as u64 + 0x1FFFFF) & !0x1FFFFF; 

        let p_base = *p_cursor;
        *p_cursor += aligned_size;
        
        // [FIX] ntoskrnl.exe 로드 후 hal.dll 물리 주소 간격 조정 (기존 로직 유지)
        if name == "ntoskrnl.exe" { *p_cursor = 0x2000000; } 

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

    fn load_file_dynamic(&mut self, vm: &mut Vm, dir: &str, name: &str, p_cursor: &mut u64, v_cursor: &mut u64) -> Result<(), Box<dyn std::error::Error>> {
        let full_path = format!("{}/{}", dir, name);
        let buf = fs::read(&full_path)?;
        let pe = crate::pe::parse(&buf)?;

        let size = pe.sections.iter().map(|s| s.virtual_address + s.virtual_size - pe.image_base).max().unwrap_or(0x10000) as u32;
        let aligned_size = (size as u64 + 0x1FFFFF) & !0x1FFFFF; 

        let p_base = *p_cursor;
        let v_base = *v_cursor;

        *p_cursor += aligned_size;
        *v_cursor += aligned_size;

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

    pub fn bind_all(&mut self, vm: &mut Vm) -> Result<(), &'static str> {
        let resolver = crate::forwarder::ForwarderResolver::new(&self.modules);

        for m in &self.modules {
            let imports = m.pe.get_imports()?;
            for imp in imports {
                for func in imp.functions {
                    if let Some(addr) = resolver.resolve(&imp.dll_name, &func.name) {
                        let iat_paddr = m.p_base + func.iat_rva as u64;
                        vm.write_memory(iat_paddr as usize, &addr.to_le_bytes())?;
                    }
                }
            }
        }
        Ok(())
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
