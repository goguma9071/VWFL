use crate::pe::PeFile;
use crate::vm;

pub fn load_sections(vm : &mut super::vm::Vm, pe_file: &super::pe::PeFile) -> Result<(), &'static str> {
    for section in &pe_file.sections {
        // Calculate the rebased destination address
        // The section's virtual_address is absolute; subtract ImageBase to get an offset relative to 0
        let rebased_destination = section.virtual_address.checked_sub(pe_file.image_base)
            .ok_or("Section virtual address is less than ImageBase, cannot rebase.")?;

        let data = &section.raw_data;

        for i in 0..data.len() {
            // Write to the rebased address in the VM's memory
            vm.write_byte(rebased_destination + i as u64, data[i])?;
        }
    }
    Ok(())
}