// src/acpi.rs
use crate::vm::Vm;

pub fn setup(vm: &mut Vm, base_paddr: u64, base_vaddr: u64) -> Result<u64, &'static str> {
    let rsdp_p = base_paddr;
    let xsdt_p = base_paddr + 0x100;
    let madt_p = base_paddr + 0x200;
    let fadt_p = base_paddr + 0x300;
    let dsdt_p = base_paddr + 0x500;

    let rsdp_v = base_vaddr;
    let xsdt_v = base_vaddr + 0x100;
    let madt_v = base_vaddr + 0x200;
    let fadt_v = base_vaddr + 0x300;
    let dsdt_v = base_vaddr + 0x500;

    // 1. DSDT (Minimal)
    let mut dsdt = vec![0u8; 36];
    write_header(&mut dsdt, b"DSDT", 36, 1);
    update_checksum(&mut dsdt);
    vm.write_memory(dsdt_p as usize, &dsdt)?;

    // 2. MADT (Multiple APIC Description Table)
    let mut madt = vec![0u8; 44];
    // Total Length: Header(44) + LAPIC(8) + IOAPIC(12) + ISO(10) = 74
    write_header(&mut madt, b"APIC", 74, 2); 
    madt[36..40].copy_from_slice(&0xFEE00000u32.to_le_bytes()); // Local APIC Phys Base
    madt[40..44].copy_from_slice(&1u32.to_le_bytes()); // PC-AT Compatible
    
    // [FIX] Type 0: Processor Local APIC (ID 0)
    let lapic_entry: [u8; 8] = [0, 8, 0, 0, 1, 0, 0, 0]; // Type, Len, ProcID, APICID(0), Flags(Enabled)
    madt.extend_from_slice(&lapic_entry);
    
    // [FIX] Type 1: I/O APIC (ID 1, Addr 0xFEC00000)
    let ioapic_entry: [u8; 12] = [1, 12, 1, 0, 0, 0, 0, 0, 0x00, 0xC0, 0x00, 0xFE]; 
    madt.extend_from_slice(&ioapic_entry);

    // [CORE FIX] Type 2: Interrupt Source Override (ISA IRQ 0 -> GSI 2)
    // 윈도우 타이머 인터럽트가 IOAPIC 2번 핀으로 들어옴을 명시
    let iso_entry: [u8; 10] = [2, 10, 0, 0, 2, 0, 0, 0, 0, 0]; // Type, Len, Bus(0), Source(0), GSI(2), Flags(0)
    madt.extend_from_slice(&iso_entry);
    
    update_checksum(&mut madt);
    vm.write_memory(madt_p as usize, &madt)?;

    // 3. FADT
    let mut fadt = vec![0u8; 244];
    write_header(&mut fadt, b"FACP", 244, 3);
    fadt[109] = 0x3; 
    fadt[112..116].copy_from_slice(&0x00000401u32.to_le_bytes()); 
    fadt[140..148].copy_from_slice(&dsdt_v.to_le_bytes()); 
    update_checksum(&mut fadt);
    vm.write_memory(fadt_p as usize, &fadt)?;

    // 4. XSDT
    let mut xsdt = vec![0u8; 36 + 16];
    write_header(&mut xsdt, b"XSDT", 36 + 16, 1);
    xsdt[36..44].copy_from_slice(&fadt_v.to_le_bytes()); 
    xsdt[44..52].copy_from_slice(&madt_v.to_le_bytes()); 
    update_checksum(&mut xsdt);
    vm.write_memory(xsdt_p as usize, &xsdt)?;

    // 5. RSDP
    let mut rsdp = [0u8; 36];
    rsdp[0..8].copy_from_slice(b"RSD PTR ");
    rsdp[10..16].copy_from_slice(b"VWFL  "); 
    rsdp[15] = 2; 
    rsdp[20..24].copy_from_slice(&36u32.to_le_bytes()); 
    rsdp[24..32].copy_from_slice(&xsdt_v.to_le_bytes()); 

    let sum1 = rsdp[0..20].iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    rsdp[8] = (0u8).wrapping_sub(sum1);
    let sum2 = rsdp.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    rsdp[32] = (0u8).wrapping_sub(sum2);
    
    vm.write_memory(rsdp_p as usize, &rsdp)?;
    Ok(rsdp_v)
}

fn write_header(data: &mut [u8], sig: &[u8; 4], len: u32, rev: u8) {
    data[0..4].copy_from_slice(sig);
    data[4..8].copy_from_slice(&len.to_le_bytes());
    data[8] = rev;
    data[10..16].copy_from_slice(b"VWFL  ");
    data[16..24].copy_from_slice(b"HYPERVIS");
}

fn update_checksum(data: &mut [u8]) {
    data[9] = 0;
    let sum = data.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    data[9] = (0u8).wrapping_sub(sum);
}
