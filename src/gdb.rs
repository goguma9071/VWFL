use gdbstub::target::ext::base::BaseOps;
use gdbstub::target::ext::base::single_register::SingleRegisterOps;
use gdbstub::target::{Target, TargetResult};
use gdbstub::target::ext::breakpoints::{Breakpoints, SwBreakpoint, SwBreakpointOps};
use gdbstub_arch::x86::reg::X86_64CoreRegs;
use crate::vm::Vm;

pub struct VwflTarget<'a> {
    pub vm: &'a mut Vm,
}

impl<'a> Target for VwflTarget<'a> {
    type Arch = gdbstub_arch::x86::X86_64_SSE;
    type Error = &'static str;

    fn base_ops(&mut self) -> BaseOps<'_, Self::Arch, Self::Error> {
        BaseOps::MultiThread(self)
    }

    fn support_breakpoints(&mut self) -> Option<Breakpoints<'_, Self>> {
        Some(Breakpoints::new(self))
    }
}

impl<'a> gdbstub::target::ext::base::multithread::MultiThreadBaseOps for VwflTarget<'a> {
    fn read_registers(&mut self, _tid: gdbstub::common::Tid, regs: &mut X86_64CoreRegs) -> TargetResult<(), Self> {
        let kvm_regs = self.vm.vcpu_fd.get_regs().map_err(|_| ())?;
        let sregs = self.vm.vcpu_fd.get_sregs().map_err(|_| ())?;
        
        regs.rax = kvm_regs.rax;
        regs.rbx = kvm_regs.rbx;
        regs.rcx = kvm_regs.rcx;
        regs.rdx = kvm_regs.rdx;
        regs.rsi = kvm_regs.rsi;
        regs.rdi = kvm_regs.rdi;
        regs.rbp = kvm_regs.rbp;
        regs.rsp = kvm_regs.rsp;
        regs.r8 = kvm_regs.r8;
        regs.r9 = kvm_regs.r9;
        regs.r10 = kvm_regs.r10;
        regs.r11 = kvm_regs.r11;
        regs.r12 = kvm_regs.r12;
        regs.r13 = kvm_regs.r13;
        regs.r14 = kvm_regs.r14;
        regs.r15 = kvm_regs.r15;
        regs.rip = kvm_regs.rip;
        regs.eflags = kvm_regs.rflags as u32;
        
        // Segments and other registers would go here
        Ok(())
    }

    fn write_registers(&mut self, _tid: gdbstub::common::Tid, regs: &X86_64CoreRegs) -> TargetResult<(), Self> {
        let mut kvm_regs = self.vm.vcpu_fd.get_regs().map_err(|_| ())?;
        kvm_regs.rax = regs.rax;
        kvm_regs.rbx = regs.rbx;
        kvm_regs.rcx = regs.rcx;
        kvm_regs.rdx = regs.rdx;
        kvm_regs.rsi = regs.rsi;
        kvm_regs.rdi = regs.rdi;
        kvm_regs.rbp = regs.rbp;
        kvm_regs.rsp = regs.rsp;
        kvm_regs.r8 = regs.r8;
        kvm_regs.r9 = regs.r9;
        kvm_regs.r10 = regs.r10;
        kvm_regs.r11 = regs.r11;
        kvm_regs.r12 = regs.r12;
        kvm_regs.r13 = regs.r13;
        kvm_regs.r14 = regs.r14;
        kvm_regs.r15 = regs.r15;
        kvm_regs.rip = regs.rip;
        kvm_regs.rflags = regs.eflags as u64;
        self.vm.vcpu_fd.set_regs(&kvm_regs).map_err(|_| ())?;
        Ok(())
    }

    fn read_addrs(&mut self, _tid: gdbstub::common::Tid, start_addr: u64, data: &mut [u8]) -> TargetResult<usize, Self> {
        // Simple linear mapping for now, should use virt_to_phys in real usage
        if let Err(_) = self.vm.read_memory(start_addr as usize, data) {
            return Ok(0);
        }
        Ok(data.len())
    }

    fn write_addrs(&mut self, _tid: gdbstub::common::Tid, start_addr: u64, data: &[u8]) -> TargetResult<(), Self> {
        self.vm.write_memory(start_addr as usize, data).map_err(|_| ())?;
        Ok(())
    }

    fn list_active_threads(&mut self, mut cb: gdbstub::target::ext::base::multithread::ThreadEnumerator<'_, Self>) -> TargetResult<(), Self> {
        cb(gdbstub::common::Tid::new(1).unwrap());
        Ok(())
    }

    fn support_single_register_access(&mut self) -> Option<SingleRegisterOps<'_, gdbstub::common::Tid, Self::Arch, Self::Error>> {
        None
    }
}

impl<'a> SwBreakpoint for VwflTarget<'a> {
    fn add_sw_breakpoint(&mut self, _tid: gdbstub::common::Tid, addr: u64, _kind: usize) -> TargetResult<bool, Self> {
        // Implementation for software breakpoints (e.g., using KVM_SET_GUEST_DEBUG)
        Ok(true)
    }

    fn remove_sw_breakpoint(&mut self, _tid: gdbstub::common::Tid, addr: u64, _kind: usize) -> TargetResult<bool, Self> {
        Ok(true)
    }
}
