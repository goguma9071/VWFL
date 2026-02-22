use kvm_ioctls::{Kvm, VmFd, VcpuFd};
use kvm_bindings::{kvm_userspace_memory_region, KVM_MEM_LOG_DIRTY_PAGES};
use std::ptr;

pub const MEM_SIZE: usize = 8 * 1024 * 1024 * 1024; // 8GB Memory
pub const SERIAL_PORT_ADDRESS: u64 = 0x10000000; // 가상 시리얼 포트 주소 (MMIO)

pub struct Vm {
    #[allow(dead_code)]
    pub kvm: Kvm,
    #[allow(dead_code)]
    pub vm_fd: VmFd,
    pub vcpu_fd: VcpuFd,
    pub mem_ptr: *mut u8, // 할당된 실제 메모리의 포인터
    pub mem_size: usize,
}

// 메모리 자동 해제를 위한 Drop 구현
impl Drop for Vm {
    fn drop(&mut self) {
        unsafe {
            // mmap으로 할당했던 메모리를 해제
            libc::munmap(self.mem_ptr as *mut libc::c_void, self.mem_size);
        }
    }
}

impl Vm {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 1. KVM 열기
        let kvm = Kvm::new()?;

        // 2. VM 생성
        let vm_fd = kvm.create_vm()?;

        // 3. 메모리 할당 (mmap 사용)
        let mem_size = MEM_SIZE;
        let mem_ptr = unsafe {
            libc::mmap(
                ptr::null_mut(),
                mem_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_ANONYMOUS | libc::MAP_SHARED | libc::MAP_NORESERVE,
                -1,
                0,
            ) as *mut u8
        };

        if mem_ptr == ptr::null_mut() {
            return Err("Failed to mmap memory".into());
        }

        // 4. KVM에 메모리 등록 (Slot 0에 매핑)
        let mem_region = kvm_userspace_memory_region {
            slot: 0,
            guest_phys_addr: 0,
            memory_size: mem_size as u64,
            userspace_addr: mem_ptr as u64,
            flags: KVM_MEM_LOG_DIRTY_PAGES, // (선택) 메모리 변경 추적
        };

        unsafe {
            vm_fd.set_user_memory_region(mem_region)?;
        }

        // 5. 가상 인터럽트 컨트롤러(IRQCHIP) 및 타이머(PIT) 생성 (윈도우 필수)
        vm_fd.create_irq_chip().map_err(|e| format!("Failed to create IRQ chip: {:?}", e))?;
        let pit_config = kvm_bindings::kvm_pit_config::default();
        vm_fd.create_pit2(pit_config).map_err(|e| format!("Failed to create PIT2: {:?}", e))?;

        println!("[VM] KVM IRQ Chip and PIT2 initialized successfully.");

        // 6. vCPU 생성 (ID: 0)
        let vcpu_fd = vm_fd.create_vcpu(0)?;

        // [FIX] Enable Guest Debug to intercept INT 3 before Guest IDT
        let debug_struct = kvm_bindings::kvm_guest_debug {
            control: kvm_bindings::KVM_GUESTDBG_ENABLE | kvm_bindings::KVM_GUESTDBG_USE_SW_BP,
            ..Default::default()
        };
        vcpu_fd.set_guest_debug(&debug_struct)?;

        Ok(Vm {
            kvm,
            vm_fd,
            vcpu_fd,
            mem_ptr,
            mem_size,
        })
    }

    /// 가상 메모리에 데이터 쓰기
    pub fn write_memory(&mut self, offset: usize, data: &[u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.mem_size {
            return Err("Memory write out of bounds");
        }
        unsafe {
            let dest = self.mem_ptr.add(offset);
            ptr::copy_nonoverlapping(data.as_ptr(), dest, data.len());
        }
        Ok(())
    }

    /// 가상 메모리에서 데이터 읽기
    pub fn read_memory(&self, offset: usize, data: &mut [u8]) -> Result<(), &'static str> {
        if offset + data.len() > self.mem_size {
            return Err("Memory read out of bounds");
        }
        unsafe {
            let src = self.mem_ptr.add(offset);
            ptr::copy_nonoverlapping(src, data.as_mut_ptr(), data.len());
        }
        Ok(())
    }
}
