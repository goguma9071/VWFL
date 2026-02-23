use kvm_ioctls::{Kvm, VmFd, VcpuFd};
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES, kvm_irq_routing_entry, KVM_IRQ_ROUTING_IRQCHIP};
use std::ptr;

pub const MEM_SIZE: usize = 8 * 1024 * 1024 * 1024; // 8GB Memory
pub const SERIAL_PORT_ADDRESS: u64 = 0x10000000; // 가상 시리얼 포트 주소 (MMIO)

pub struct Vm {
    #[allow(dead_code)]
    pub kvm: Kvm,
    #[allow(dead_code)]
    pub vm_fd: VmFd,
    pub vcpu_fd: VcpuFd,
    pub mem_ptr: *mut u8, 
    pub mem_size: usize,
}

impl Drop for Vm {
    fn drop(&mut self) {
        unsafe {
            libc::munmap(self.mem_ptr as *mut libc::c_void, self.mem_size);
        }
    }
}

impl Vm {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let kvm = Kvm::new()?;
        let vm_fd = kvm.create_vm()?;

        let mem_size = MEM_SIZE;
        let mem_ptr = unsafe {
            libc::mmap(ptr::null_mut(), mem_size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE, -1, 0) as *mut u8
        };

        if mem_ptr == ptr::null_mut() { return Err("Failed to mmap memory".into()); }

        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: 0,
            memory_size: mem_size as u64,
            userspace_addr: mem_ptr as u64,
            flags: KVM_MEM_LOG_DIRTY_PAGES,
        };
        unsafe { vm_fd.set_user_memory_region(mem_region)?; }

        vm_fd.create_irq_chip().map_err(|e| format!("Failed to create IRQ chip: {:?}", e))?;
        let pit_config = kvm_bindings::kvm_pit_config::default();
        vm_fd.create_pit2(pit_config).map_err(|e| format!("Failed to create PIT2: {:?}", e))?;

        // [CORE FIX] GSI Routing: PIT (Source 0) -> IOAPIC Pin 2
        let mut entries = Vec::new();
        for i in 0..24 {
            // IOAPIC Routing
            let mut io_entry = kvm_irq_routing_entry::default();
            io_entry.gsi = i as u32;
            io_entry.type_ = KVM_IRQ_ROUTING_IRQCHIP;
            let mut io_chip = kvm_bindings::kvm_irq_routing_irqchip::default();
            io_chip.irqchip = 2; // IOAPIC
            // 핵심: KVM GSI 0(PIT) 신호를 IOAPIC의 핀 2번으로 배달
            io_chip.pin = if i == 0 { 2 } else { i as u32 }; 
            io_entry.u.irqchip = io_chip;
            entries.push(io_entry);

            // Legacy PIC Routing (0-15)
            if i < 16 {
                let mut pic_entry = kvm_irq_routing_entry::default();
                pic_entry.gsi = i as u32;
                pic_entry.type_ = KVM_IRQ_ROUTING_IRQCHIP;
                let mut pic_chip = kvm_bindings::kvm_irq_routing_irqchip::default();
                pic_chip.irqchip = if i < 8 { 0 } else { 1 };
                pic_chip.pin = (i % 8) as u32;
                pic_entry.u.irqchip = pic_chip;
                entries.push(pic_entry);
            }
        }
        let routing = kvm_bindings::KvmIrqRouting::from_entries(&entries).map_err(|e| format!("GSI Error: {:?}", e))?;
        vm_fd.set_gsi_routing(&routing)?;

        println!("[VM] KVM GSI Routing (PIT->Pin2) initialized successfully.");

        let vcpu_fd = vm_fd.create_vcpu(0)?;
        let debug_struct = kvm_bindings::kvm_guest_debug {
            control: kvm_bindings::KVM_GUESTDBG_ENABLE | kvm_bindings::KVM_GUESTDBG_USE_SW_BP,
            ..Default::default()
        };
        vcpu_fd.set_guest_debug(&debug_struct)?;

        Ok(Vm { kvm, vm_fd, vcpu_fd, mem_ptr, mem_size })
    }

    pub fn write_memory(&mut self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.mem_size { return Err("Memory write out of bounds"); }
        unsafe { ptr::copy_nonoverlapping(data.as_ptr(), self.mem_ptr.add(offset), data.len()); }
        Ok(())
    }

    pub fn read_memory(&self, offset: usize, data: &mut [u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.mem_size { return Err("Memory read out of bounds"); }
        unsafe { ptr::copy_nonoverlapping(self.mem_ptr.add(offset), data.as_mut_ptr(), data.len()); }
        Ok(())
    }
}
