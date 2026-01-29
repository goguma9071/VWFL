// src/acpi.rs
use crate::vm::Vm;

#[repr(C, packed)]
struct AcpiHeader {
    signature: [u8; 4],
    length: u32,
    revision: u8,
    checksum: u8,
    oem_id: [u8; 6],
    oem_table_id: [u8; 8],
    oem_revision: u32,
    creator_id: [u32; 1],
    creator_revision: u32,
}

/// 최소한의 ACPI 테이블 세트를 구축합니다.
pub fn setup(vm: &mut Vm, base_paddr: u64, base_vaddr: u64) -> Result<u64, &'static str> {
    // 테이블별 오프셋 설정
    let rsdp_p = base_paddr;
    let xsdt_p = base_paddr + 0x100;
    let madt_p = base_paddr + 0x200;
    let fadt_p = base_paddr + 0x300;

    let rsdp_v = base_vaddr;
    let xsdt_v = base_vaddr + 0x100;
    let madt_v = base_vaddr + 0x200;
    let fadt_v = base_vaddr + 0x300;

    // 1. MADT (Multiple APIC Description Table) - CPU 정보
    let mut madt = vec![0u8; 44]; // Header(36) + Local APIC(8)
    write_header(&mut madt, b"APIC", 44, 1);
    // Local APIC 주소 (0xFEE00000)
    madt[36..40].copy_from_slice(&0xFEE00000u32.to_le_bytes());
    // Flags (PC-AT Dual PIC)
    madt[40..44].copy_from_slice(&1u32.to_le_bytes());
    // Entry: Processor Local APIC (Type 0, Length 8)
    let lapic_entry: [u8; 8] = [0, 8, 0, 0, 1, 0, 0, 0]; // Type 0, Len 8, ACPI ID 0, APIC ID 0, Enabled 1
    madt.extend_from_slice(&lapic_entry);
    update_checksum(&mut madt);
    vm.write_memory(madt_p as usize, &madt)?;

    // 2. FADT (Fixed ACPI Description Table) - 필수 항목
    let mut fadt = vec![0u8; 244];
    write_header(&mut fadt, b"FACP", 244, 4);
    update_checksum(&mut fadt);
    vm.write_memory(fadt_p as usize, &fadt)?;

    // 3. XSDT (Extended System Description Table) - 테이블 목록
    let mut xsdt = vec![0u8; 36 + 16]; // Header(36) + 2 entries(16)
    write_header(&mut xsdt, b"XSDT", 36 + 16, 1);
    xsdt[36..44].copy_from_slice(&madt_v.to_le_bytes());
    xsdt[44..52].copy_from_slice(&fadt_v.to_le_bytes());
    update_checksum(&mut xsdt);
    vm.write_memory(xsdt_p as usize, &xsdt)?;

    // 4. RSDP (Root System Description Pointer) - 입구
    let mut rsdp = [0u8; 36];
    rsdp[0..8].copy_from_slice(b"RSD PTR ");
    rsdp[15] = 0; // Checksum (First 20 bytes)
    rsdp[16..22].copy_from_slice(b"BOCHS ");
    rsdp[22] = 2; // Revision 2 (ACPI 2.0+)
    rsdp[24..32].copy_from_slice(&xsdt_v.to_le_bytes());
    rsdp[32] = 36; // Length
    
    // RSDP 체크섬 계산 (첫 20바이트)
    let sum1 = rsdp[0..20].iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    rsdp[15] = (0u8).wrapping_sub(sum1);
    // 전체 체크섬 계산
    let sum2 = rsdp.iter().fold(0u8, |acc, &x| acc.wrapping_add(x));
    rsdp[33] = (0u8).wrapping_sub(sum2);
    
    vm.write_memory(rsdp_p as usize, &rsdp)?;

    println!("ACPI Tables initialized at Physical: 0x{:x}", base_paddr);
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
