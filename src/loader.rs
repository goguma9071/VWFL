// src/loader.rs
use crate::pe::PeFile;
use crate::vm::Vm;

pub fn load_sections(vm: &mut Vm, pe_file: &PeFile, load_base: u64) -> Result<u64, &'static str> {
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
