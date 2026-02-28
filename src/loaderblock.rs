// src/loaderblock.rs
#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
use crate::vm::Vm;
use crate::nt_types::*;

/// _TYPE_OF_MEMORY Enum (Windows Kernel Standard)
#[repr(u32)]
#[derive(Debug, Copy, Clone)]
#[allow(dead_code)]
pub enum TYPE_OF_MEMORY {
    LoaderFree = 0,
    LoaderBad = 1,
    LoaderLoadedProgram = 2,
    LoaderFirmwareTemporary = 3,
    LoaderFirmwarePermanent = 4,
    LoaderOsloaderHeap = 5,
    LoaderOsloaderStack = 6,
    LoaderSystemCode = 7,
    LoaderHalCode = 8,
    LoaderBootLdrPage = 9,
    LoaderConsoleInPage = 10,
    LoaderConsoleOutPage = 11,
    LoaderStartupPcrPage = 12,
    LoaderStartupStackPage = 13,
    LoaderStartupDataPage = 14,
    LoaderMemoryData = 15,
    LoaderMemoryTemp = 16,
    LoaderMemorySpecial = 17,
    LoaderMemoryMax = 18,
}

/// _MEMORY_ALLOCATION_DESCRIPTOR - 0x28 bytes
#[repr(C)]
pub struct MEMORY_ALLOCATION_DESCRIPTOR {
    pub ListEntry: LIST_ENTRY,
    pub MemoryType: TYPE_OF_MEMORY,
    pub BasePage: ULONGLONG,
    pub PageCount: ULONGLONG,
}
#[repr(C)]
pub struct LDR_DATA_TABLE_ENTRY {
    pub InLoadOrderLinks: LIST_ENTRY,
    pub InMemoryOrderLinks: LIST_ENTRY,
    pub InInitializationOrderLinks: LIST_ENTRY,
    pub DllBase: PVOID,
    pub EntryPoint: PVOID,
    pub SizeOfImage: ULONG,
    pub CheckSum: ULONG,
    pub FullDllName: UNICODE_STRING,
    pub BaseDllName: UNICODE_STRING,
    pub Flags: ULONG,
    pub ObsoleteLoadCount: USHORT,
    pub TlsIndex: USHORT,
    pub HashLinks: LIST_ENTRY,
    pub TimeDateStamp: ULONG,
    pub EntryPointActivationContext: PVOID,
    pub Lock: PVOID,
    pub DdagNode: PVOID,
    pub NodeModuleLink: LIST_ENTRY,
    pub LoadContext: PVOID,
    pub ParentDllBase: PVOID,
}

/// _KSPECIAL_REGISTERS - 19041 x64
#[repr(C)]
#[derive(Copy, Clone)]
pub struct KSPECIAL_REGISTERS {
    pub Cr0: ULONGLONG,
    pub Cr2: ULONGLONG,
    pub Cr3: ULONGLONG,
    pub Cr4: ULONGLONG,
    pub KernelDr0: ULONGLONG,
    pub KernelDr1: ULONGLONG,
    pub KernelDr2: ULONGLONG,
    pub KernelDr3: ULONGLONG,
    pub KernelDr6: ULONGLONG,
    pub KernelDr7: ULONGLONG,
    pub Gdtr: [u8; 16], 
    pub Idtr: [u8; 16],
    pub Tr: USHORT,
    pub Ldtr: USHORT,
    pub MxCsr: ULONG,
    pub DebugControl: ULONGLONG,
    pub LastBranchToRip: ULONGLONG,
    pub LastBranchFromRip: ULONGLONG,
    pub LastExceptionToRip: ULONGLONG,
    pub LastExceptionFromRip: ULONGLONG,
    pub Cr8: ULONGLONG,
    pub XCr0: ULONGLONG,
}

/// _KPROCESSOR_STATE
#[repr(C)]
#[derive(Copy, Clone)]
pub struct KPROCESSOR_STATE {
    pub SpecialRegisters: KSPECIAL_REGISTERS,
    pub ContextFrame: [UCHAR; 0x4d0],
}

/// _KPRCB (Kernel Processor Control Block) - 최신 Windows 10 명세 반영 (sizeof 0x700)
#[repr(C, align(64))]
pub struct KPRCB {
    pub MxCsr: ULONG,                                   // 0x0
    pub LegacyNumber: UCHAR,                            // 0x4
    pub ReservedMustBeZero: UCHAR,                      // 0x5
    pub InterruptRequest: UCHAR,                        // 0x6
    pub IdleHalt: UCHAR,                                // 0x7
    pub CurrentThread: PVOID,                           // 0x8
    pub NextThread: PVOID,                              // 0x10
    pub IdleThread: PVOID,                              // 0x18
    pub NestingLevel: UCHAR,                            // 0x20
    pub ClockOwner: UCHAR,                              // 0x21
    pub PendingTickFlags: UCHAR,                        // 0x22
    pub IdleState: UCHAR,                               // 0x23
    pub Number: ULONG,                                  // 0x24
    pub RspBase: ULONGLONG,                             // 0x28
    pub PrcbLock: ULONGLONG,                            // 0x30
    pub PriorityState: PVOID,                           // 0x38
    
    pub CpuType: UCHAR,                                 // 0x40
    pub CpuID: UCHAR,                                   // 0x41
    pub CpuStep: USHORT,                                // 0x42
    pub MHz: ULONG,                                     // 0x44
    pub HalReserved: [ULONGLONG; 8],                    // 0x48 -> 0x88
    
    pub MinorVersion: USHORT,                           // 0x88
    pub MajorVersion: USHORT,                           // 0x8a
    pub BuildType: UCHAR,                               // 0x8c
    pub CpuVendor: UCHAR,                               // 0x8d
    pub LegacyCoresPerPhysicalProcessor: UCHAR,         // 0x8e
    pub LegacyLogicalProcessorsPerCore: UCHAR,          // 0x8f
    pub TscFrequency: ULONGLONG,                        // 0x90
    
    pub CoresPerPhysicalProcessor: ULONG,               // 0x98
    pub LogicalProcessorsPerCore: ULONG,                // 0x9c
    pub PrcbPad04: [ULONGLONG; 4],                      // 0xa0
    pub ParentNode: PVOID,                              // 0xc0
    pub GroupSetMember: ULONGLONG,                      // 0xc8
    pub Group: UCHAR,                                   // 0xd0
    pub GroupIndex: UCHAR,                              // 0xd1
    pub PrcbPad05: [UCHAR; 2],                          // 0xd2
    pub InitialApicId: ULONG,                           // 0xd4
    pub ScbOffset: ULONG,                               // 0xd8
    pub ApicMask: ULONG,                                // 0xdc
    pub AcpiReserved: PVOID,                            // 0xe0
    pub CFlushSize: ULONG,                              // 0xe8
    pub PrcbPad11: [ULONGLONG; 2],                      // 0xf0
    
    pub ProcessorState: KPROCESSOR_STATE,               // 0x100
}

/// _KPCR (Kernel Processor Control Region) - 0x180 bytes
#[repr(C, align(16))]
pub struct KPCR {
    pub GdtBase: PVOID,                                 // 0x0
    pub TssBase: PVOID,                                 // 0x8
    pub UserRsp: ULONGLONG,                             // 0x10
    pub SelfPcr: PVOID,                                 // 0x18
    pub CurrentPrcb: PVOID,                             // 0x20
    pub LockArray: PVOID,                               // 0x28
    pub Used_Self: PVOID,                               // 0x30
    pub IdtBase: PVOID,                                 // 0x38
    pub Unused: [ULONGLONG; 2],                         // 0x40 -> 0x50
    pub Irql: UCHAR,                                    // 0x50
    pub Unused2: [UCHAR; 19],                           // 0x51 -> 0x64
    pub StallScaleFactor: ULONG,                        // 0x64 (CRITICAL for timing)
    pub Padding: [UCHAR; 160],                          // 0x68 -> 0x108
    pub KdVersionBlock: PVOID,                          // 0x108
    pub Reserved: [ULONGLONG; 14],                      // 0x110 -> 0x180
}

pub struct Kpcr;
impl Kpcr {
    pub fn setup(vm: &mut Vm, vaddr: u64, paddr: u64, gdt_v: u64, idt_v: u64, tss_v: u64, stack_v: u64) -> Result<(), &'static str> {
        let mut kpcr = unsafe { std::mem::zeroed::<KPCR>() };
        let mut prcb = unsafe { std::mem::zeroed::<KPRCB>() };
        
        let prcb_v = vaddr + 0x180;
        let dummy_thread_v = vaddr + 0x5000;
        let dummy_process_v = vaddr + 0x6000;

        kpcr.GdtBase = gdt_v;
        kpcr.TssBase = tss_v;
        kpcr.SelfPcr = vaddr;
        kpcr.CurrentPrcb = prcb_v;
        kpcr.Used_Self = vaddr;
        kpcr.IdtBase = idt_v;
        kpcr.StallScaleFactor = 0x00000024; // 3.6GHz (3600MHz / 100) Scale Factor

        prcb.MxCsr = 0x1F80;
        prcb.CurrentThread = dummy_thread_v;
        prcb.NextThread = dummy_thread_v;
        prcb.IdleThread = dummy_thread_v;
        prcb.RspBase = stack_v;
        prcb.MinorVersion = 19045;
        prcb.MajorVersion = 10;
        prcb.TscFrequency = 3600000000; 
        prcb.MHz = 3600;

        let cr3 = 0x8102000; 
        prcb.ProcessorState.SpecialRegisters.Cr3 = cr3;
        prcb.ProcessorState.SpecialRegisters.Cr0 = 0x80050033;
        prcb.ProcessorState.SpecialRegisters.Cr4 = 0x6f8;
        prcb.ProcessorState.SpecialRegisters.MxCsr = 0x1F80;
        prcb.ProcessorState.SpecialRegisters.Tr = 0x40;

        let gdt_limit: u16 = (32 * 8 - 1) as u16;
        prcb.ProcessorState.SpecialRegisters.Gdtr[2..4].copy_from_slice(&gdt_limit.to_le_bytes());
        prcb.ProcessorState.SpecialRegisters.Gdtr[4..12].copy_from_slice(&gdt_v.to_le_bytes());
        
        let idt_limit: u16 = 0x0FFF;
        prcb.ProcessorState.SpecialRegisters.Idtr[2..4].copy_from_slice(&idt_limit.to_le_bytes());
        prcb.ProcessorState.SpecialRegisters.Idtr[4..12].copy_from_slice(&idt_v.to_le_bytes());

        let kpcr_bytes = unsafe { std::slice::from_raw_parts(&kpcr as *const _ as *const u8, std::mem::size_of::<KPCR>()) };
        vm.write_memory(paddr as usize, kpcr_bytes)?;

        // PRCB 정밀 수동 기록
        let prcb_p = paddr + 0x180;
        let thread_p = paddr + 0x5000;

        vm.write_memory((prcb_p + 0x24) as usize, &0u32.to_le_bytes()).ok(); // Number = 0
        vm.write_memory((prcb_p + 0x28) as usize, &stack_v.to_le_bytes()).ok(); // RspBase
        vm.write_memory((prcb_p + 0x8) as usize, &dummy_thread_v.to_le_bytes()).ok(); // CurrentThread
        
        // 1. KPRCB.GroupSetMember (Offset 0x2D8) <- 1 (BSP 활성화)
        vm.write_memory((prcb_p + 0x2D8) as usize, &1u64.to_le_bytes()).ok();

        // 2. KPRCB APIC ID 삼위일체
        vm.write_memory((prcb_p + 0xD4) as usize, &[0u8]).ok(); // InitialApicId = 0
        vm.write_memory((prcb_p + 0x2D0) as usize, &[0u8]).ok(); // ApicId = 0

        // 3. KPROCESS.DirectoryTableBase (Offset 0x28) <- CR3
        vm.write_memory((paddr + 0x6000 + 0x28) as usize, &cr3.to_le_bytes()).ok();
        
        // 4. KTHREAD 초기화 (베르길리우스 22H2 표준)
        vm.write_memory((thread_p + 0x28) as usize, &stack_v.to_le_bytes()).ok(); // InitialStack
        vm.write_memory((thread_p + 0x30) as usize, &(stack_v - 0x10000).to_le_bytes()).ok(); // StackLimit
        vm.write_memory((thread_p + 0x38) as usize, &stack_v.to_le_bytes()).ok(); // StackBase
        
        vm.write_memory((thread_p + 0x71) as usize, &[1u8]).ok(); // Running = 1
        
        vm.write_memory((thread_p + 0x98) as usize, &dummy_process_v.to_le_bytes()).ok(); // ApcState.Process
        vm.write_memory((thread_p + 0x220) as usize, &dummy_process_v.to_le_bytes()).ok(); // Process Direct Pointer
        
        vm.write_memory((thread_p + 0xC8) as usize, &0u64.to_le_bytes()).ok(); // WaitStatus
        vm.write_memory((thread_p + 0x184) as usize, &[2u8]).ok(); // State = Running
        vm.write_memory((thread_p + 0x24a) as usize, &[0u8]).ok(); // [NEW] ApcStateIndex = 0
        vm.write_memory((thread_p + 0x283) as usize, &[0u8]).ok(); // WaitReason = Executive
        vm.write_memory((thread_p + 0x1D0) as usize, &[1u8]).ok(); // Affinity (CombinedApicMask)
        
        // [CORE FIX] ReadySummary (Offset 0x520) 주입
        // 스케줄러에게 "8번 우선순위(0x100)에 준비된 스레드가 있다"고 알려 유휴 상태 탈출을 유도합니다.
        vm.write_memory((prcb_p + 0x520) as usize, &0x100u32.to_le_bytes()).ok();

        // [CORE FIX] 9. KPRCB DispatcherReadyListHead (Offset 0x530) 초기화
        // 32개의 우선순위 리스트 헤드를 순환 구조(자기 참조)로 만듭니다.
        for i in 0..32 {
            let entry_v = prcb_v + 0x530 + (i * 16);
            let entry_p = prcb_p + 0x530 + (i * 16);
            vm.write_memory(entry_p as usize, &entry_v.to_le_bytes()).ok(); // Flink
            vm.write_memory((entry_p + 8) as usize, &entry_v.to_le_bytes()).ok(); // Blink
        }

        Ok(())
    }
}

/// _LOADER_PARAMETER_BLOCK
#[repr(C)]
pub struct LOADER_PARAMETER_BLOCK {
    pub OsMajorVersion: ULONG,
    pub OsMinorVersion: ULONG,
    pub Size: ULONG,
    pub OsLoaderSecurityVersion: ULONG,
    pub LoadOrderListHead: LIST_ENTRY,
    pub MemoryDescriptorListHead: LIST_ENTRY,
    pub BootDriverListHead: LIST_ENTRY,
    pub EarlyLaunchListHead: LIST_ENTRY,
    pub CoreDriverListHead: LIST_ENTRY,
    pub CoreExtensionsDriverListHead: LIST_ENTRY,
    pub TpmCoreDriverListHead: LIST_ENTRY,
    pub KernelStack: ULONGLONG,
    pub Prcb: ULONGLONG,
    pub Process: ULONGLONG,
    pub Thread: ULONGLONG,
    pub KernelStackSize: ULONG,
    pub RegistryLength: ULONG,
    pub RegistryBase: PVOID,
    pub ConfigurationRoot: PVOID,
    pub ArcBootDeviceName: PVOID,
    pub ArcHalDeviceName: PVOID,
    pub NtBootPathName: PVOID,
    pub NtHalPathName: PVOID,
    pub LoadOptions: PVOID,
    pub NlsData: PVOID,
    pub ArcDiskInformation: PVOID,
    pub Extension: PVOID,
    pub u: [u8; 0x10],
    pub FirmwareInformation: [u8; 0x40],
    pub OsBootstatPathName: PVOID,
    pub ArcOSDataDeviceName: PVOID,
    pub ArcWindowsSysPartName: PVOID,
}

pub struct LoaderParameterBlock;
impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64, stack_size: u32, registry_v: u64, registry_size: u32, nls_v: u64) -> Result<(), &'static str> {
        let mut lpb = unsafe { std::mem::zeroed::<LOADER_PARAMETER_BLOCK>() };
        lpb.OsMajorVersion = 10;
        lpb.Size = 0x160;
        lpb.KernelStack = stack_v;
        lpb.Prcb = prcb_v;
        lpb.Process = prcb_v + 0x5e80;
        lpb.Thread = prcb_v + 0x4e80;
        lpb.KernelStackSize = stack_size; // [FIX] 커널 스택 크기 명시
        lpb.RegistryLength = registry_size;
        lpb.RegistryBase = registry_v;
        lpb.ConfigurationRoot = lpb_v + 0x2000;
        
        // [CORE FIX] LoadOptions는 반드시 ANSI(ASCII)여야 커널이 인식함
        let options_str = "/DEBUG /DEBUGPORT=COM1 /BAUDRATE=115200 /EMS"; 
        let mut options_bytes = options_str.as_bytes().to_vec();
        options_bytes.push(0); // Null terminator
        vm.write_memory((lpb_p + 0xC000) as usize, &options_bytes).ok();
        lpb.LoadOptions = lpb_v + 0xC000;

        lpb.NlsData = nls_v;
        lpb.Extension = lpb_v + 0x8000;
        let bytes = unsafe { std::slice::from_raw_parts(&lpb as *const _ as *const u8, 0x160) };
        vm.write_memory(lpb_p as usize, bytes)?;
        Ok(())
    }
    pub fn add_memory(vm: &mut Vm, _lpb_v: u64, _lpb_p: u64, _entry_v: u64, entry_p: u64, base_addr: u64, size: u64, mem_type: u32) -> Result<(), &'static str> {
        let mut desc = unsafe { std::mem::zeroed::<MEMORY_ALLOCATION_DESCRIPTOR>() };
        desc.MemoryType = unsafe { std::mem::transmute(mem_type) };
        desc.BasePage = base_addr >> 12;
        desc.PageCount = size >> 12;
        let bytes = unsafe { std::slice::from_raw_parts(&desc as *const _ as *const u8, 0x28) };
        vm.write_memory(entry_p as usize, bytes)?;
        Ok(())
    }
}

/// _LOADER_PARAMETER_EXTENSION
#[repr(C)]
pub struct LOADER_PARAMETER_EXTENSION {
    pub Size: ULONG,                                    // 0x0
    pub Profile: [UCHAR; 0x14],                         // 0x4
    pub EmInfFileImage: PVOID,                          // 0x18
    pub EmInfFileSize: ULONG,                           // 0x20
    pub Padding1: [UCHAR; 0x54],                        // 0x24 -> 0x78
    pub AcpiTable: PVOID,                               // 0x78
    pub AcpiTableSize: ULONG,                           // 0x80
    pub Bitfields: ULONG,                               // 0x84
    pub LoaderPerformanceData: [UCHAR; 0x60],           // 0x88 -> 0xE8
    pub Padding2: [UCHAR; 0x998],                       // 0xE8 -> 0xA80
    pub ApiSetSchema: PVOID,                            // 0xA80
    pub ApiSetSchemaSize: ULONG,                        // 0xA88
    pub Padding3: [UCHAR; 0xFC],                        // 0xA8C -> 0xB88
    pub MajorRelease: ULONG,                            // 0xB88
    pub MinorRelease: ULONG,                            // 0xB8C
}

pub struct LoaderParameterExtension;
impl LoaderParameterExtension {
    pub const OFFSET_IN_LPB: u64 = 0x8000;
    pub fn setup(vm: &mut Vm, ext_p: u64, ext_v: u64) -> Result<(), &'static str> {
        let mut ext = unsafe { std::mem::zeroed::<LOADER_PARAMETER_EXTENSION>() };
        ext.Size = 0xE38;
        ext.Bitfields = 0x4000; // LastBootSucceeded
        ext.MajorRelease = 10;
        ext.MinorRelease = 0;
        let bytes = unsafe { std::slice::from_raw_parts(&ext as *const _ as *const u8, 0xE38) };
        vm.write_memory(ext_p as usize, bytes)?;

        // [CORE FIX] HalExtensionModuleList (Offset 0xA18) 순환 리스트 초기화
        let list_head_v = ext_v + 0xA18;
        let list_head_p = ext_p + 0xA18;
        vm.write_memory(list_head_p as usize, &list_head_v.to_le_bytes()).ok(); // Flink
        vm.write_memory((list_head_p + 8) as usize, &list_head_v.to_le_bytes()).ok(); // Blink

        Ok(())
    }
    pub fn set_acpi(vm: &mut Vm, ext_p: u64, rsdp_v: u64) -> Result<(), &'static str> {
        vm.write_memory((ext_p + 0x78) as usize, &rsdp_v.to_le_bytes())?;
        vm.write_memory((ext_p + 0x80) as usize, &36u32.to_le_bytes())?;
        Ok(())
    }
    pub fn set_apiset(vm: &mut Vm, ext_p: u64, apiset_v: u64, size: u32) -> Result<(), &'static str> {
        vm.write_memory((ext_p + 0xA80) as usize, &apiset_v.to_le_bytes())?;
        vm.write_memory((ext_p + 0xA88) as usize, &size.to_le_bytes())?;
        Ok(())
    }
}

pub struct LdrDataTableEntry;
impl LdrDataTableEntry {
    pub fn add_module(vm: &mut Vm, _entry_v: u64, entry_p: u64, img_base: u64, entry_point: u64, size: u32, name: &str) -> Result<(), &'static str> {
        let mut ldr = unsafe { std::mem::zeroed::<LDR_DATA_TABLE_ENTRY>() };
        ldr.DllBase = img_base;
        ldr.EntryPoint = entry_point;
        ldr.SizeOfImage = size;
        ldr.Flags = 0x40004; 
        ldr.ObsoleteLoadCount = 1;
        let name_v = _entry_v + 0x150;
        let mut name_u16: Vec<u8> = Vec::new();
        for c in name.encode_utf16() { name_u16.extend_from_slice(&c.to_le_bytes()); }
        let len = name_u16.len() as u16;
        ldr.FullDllName.Length = len;
        ldr.FullDllName.MaximumLength = len + 2;
        ldr.FullDllName.Buffer = name_v;
        ldr.BaseDllName.Length = len;
        ldr.BaseDllName.MaximumLength = len + 2;
        ldr.BaseDllName.Buffer = name_v;
        let bytes = unsafe { std::slice::from_raw_parts(&ldr as *const _ as *const u8, 0x120) };
        vm.write_memory(entry_p as usize, bytes)?;
        vm.write_memory((entry_p + 0x150) as usize, &name_u16)?;
        Ok(())
    }
}
