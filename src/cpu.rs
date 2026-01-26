use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use std::io::{self, Write};

pub fn run(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    setup_long_mode(vm)?;
    println!("\n--- KVM vCPU Start ---");

    loop {
        let exit_reason = vm.vcpu_fd.run()?;
        match exit_reason {
            VcpuExit::IoOut(addr, data) => {
                if addr == 0x3F8 && !data.is_empty() {
                    print!("{}", data[0] as char);
                    io::stdout().flush()?;
                } else {
                    println!("IO Out: port=0x{:x}, data={:?}", addr, data);
                }
            }
            VcpuExit::Hlt => {
                println!("\nKVM: HLT instruction executed.");
                break;
            }
            other => {
                let reason_str = format!("{:?}", other);
                let regs = vm.vcpu_fd.get_regs()?;
                println!("KVM: VM stopped ({}) at RIP: 0x{:x}", reason_str, regs.rip);
                break;
            }
        }
    }
    Ok(())
}

fn setup_long_mode(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    vm.vcpu_fd.set_fpu(&kvm_bindings::kvm_fpu::default())?;

    let mut sregs = vm.vcpu_fd.get_sregs()?;

    // 1. Control Registers - 64비트 모드의 정석 설정
    sregs.cr3 = 0x1000;
    sregs.cr4 = 1 << 5; // PAE
    sregs.efer = (1 << 8) | (1 << 10); // LME | LMA (Long Mode Enable & Active)
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1); // PG | PE | MP

    // 2. GDT/IDT 가상 포인터
    sregs.gdt.base = 0;
    sregs.gdt.limit = 0xffff;
    sregs.idt.base = 0;
    sregs.idt.limit = 0xffff;

    // 3. 세그먼트 레지스터 설정 (가장 표준적인 64비트 값)
    fn seg_64(selector: u16, is_code: bool) -> kvm_bindings::kvm_segment {
        kvm_bindings::kvm_segment {
            base: 0,
            limit: 0xffffffff,
            selector: selector << 3,
            type_: if is_code { 11 } else { 3 },
            present: 1,
            dpl: 0,
            db: 0, // 64비트 코드 세그먼트에서는 반드시 0이어야 함
            s: 1,
            l: if is_code { 1 } else { 0 }, // Long mode bit
            g: 1,
            avl: 0,
            unusable: 0,
            padding: 0,
        }
    }

    sregs.cs = seg_64(1, true);
    let ds = seg_64(2, false);
    sregs.ds = ds;
    sregs.es = ds;
    sregs.fs = ds;
    sregs.gs = ds;
    sregs.ss = ds;

    // TR (Task Register) - 필수
    sregs.tr = kvm_bindings::kvm_segment {
        base: 0,
        limit: 0xffff,
        selector: 3 << 3,
        type_: 11, // Busy 32-bit TSS
        present: 1,
        dpl: 0,
        db: 0,
        s: 0,
        l: 0,
        g: 1,
        avl: 0,
        unusable: 0,
        padding: 0,
    };

    vm.vcpu_fd.set_sregs(&sregs)?;

    let mut regs = vm.vcpu_fd.get_regs()?;
    regs.rflags = 0x2;
    vm.vcpu_fd.set_regs(&regs)?;

    Ok(())
}