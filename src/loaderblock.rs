// src/loaderblock.rs

use crate::vm::Vm;
use crate::nt_types::*;

/// _TYPE_OF_MEMORY Enum (Windows Kernel Standard)
#[repr(u32)]
#[derive(Debug, Copy, Clone)]
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
    pub ListEntry: LIST_ENTRY,                          // 0x0
    pub MemoryType: TYPE_OF_MEMORY,                     // 0x10
    pub BasePage: ULONGLONG,                            // 0x18
    pub PageCount: ULONGLONG,                           // 0x20
}

/// _LDR_DATA_TABLE_ENTRY - 0x120 bytes (Windows 10 x64)
#[repr(C)]
pub struct LDR_DATA_TABLE_ENTRY {
    pub InLoadOrderLinks: LIST_ENTRY,                   // 0x0
    pub InMemoryOrderLinks: LIST_ENTRY,                 // 0x10
    pub InInitializationOrderLinks: LIST_ENTRY,         // 0x20
    pub DllBase: PVOID,                                 // 0x30
    pub EntryPoint: PVOID,                              // 0x38
    pub SizeOfImage: ULONG,                             // 0x40
    pub CheckSum: ULONG,                                // 0x44
    pub FullDllName: UNICODE_STRING,                    // 0x48
    pub BaseDllName: UNICODE_STRING,                    // 0x58
    pub Flags: ULONG,                                   // 0x68
    pub ObsoleteLoadCount: USHORT,                      // 0x6c
    pub TlsIndex: USHORT,                               // 0x6e
    pub HashLinks: LIST_ENTRY,                          // 0x70
    pub TimeDateStamp: ULONG,                           // 0x80
    pub EntryPointActivationContext: PVOID,             // 0x88
    pub Lock: PVOID,                                    // 0x90
    pub DdagNode: PVOID,                                // 0x98
    pub NodeModuleLink: LIST_ENTRY,                     // 0xa0
    pub LoadContext: PVOID,                             // 0xb0
    pub ParentDllBase: PVOID,                           // 0xb8
}

/// _KPCR (Kernel Processor Control Region) - 0x178 bytes
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
    pub Unused: [ULONGLONG; 2],                         // 0x40
    pub Irql: UCHAR,                                    // 0x50
    pub Padding: [UCHAR; 183], 
    pub KdVersionBlock: PVOID,                          // 0x108
}

/// _KPRCB (Kernel Processor Control Block) - 0x700 bytes
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
    pub HalReserved: [ULONGLONG; 8],                    // 0x48
    pub MinorVersion: USHORT,                           // 0x88
    pub MajorVersion: USHORT,                           // 0x8a
    pub Reserved2: [UCHAR; 116],                        // 0x100
    pub ProcessorState: [UCHAR; 0x5c0],                 // 0x100
}

/// _LOADER_PARAMETER_BLOCK - 0x160 bytes
#[repr(C)]
pub struct LOADER_PARAMETER_BLOCK {
    pub OsMajorVersion: ULONG,                          // 0x0
    pub OsMinorVersion: ULONG,                          // 0x4
    pub Size: ULONG,                                    // 0x8
    pub OsLoaderSecurityVersion: ULONG,                 // 0xc
    pub LoadOrderListHead: LIST_ENTRY,                  // 0x10
    pub MemoryDescriptorListHead: LIST_ENTRY,           // 0x20
    pub BootDriverListHead: LIST_ENTRY,                 // 0x30
    pub EarlyLaunchListHead: LIST_ENTRY,                // 0x40
    pub CoreDriverListHead: LIST_ENTRY,                 // 0x50
    pub CoreExtensionsDriverListHead: LIST_ENTRY,       // 0x60
    pub TpmCoreDriverListHead: LIST_ENTRY,              // 0x70
    pub KernelStack: ULONGLONG,                         // 0x80
    pub Prcb: ULONGLONG,                                // 0x88
    pub Process: ULONGLONG,                             // 0x90
    pub Thread: ULONGLONG,                              // 0x98
    pub KernelStackSize: ULONG,                         // 0xa0
    pub RegistryLength: ULONG,                          // 0xa4
    pub RegistryBase: PVOID,                            // 0xa8
    pub ConfigurationRoot: PVOID,                       // 0xb0
    pub ArcBootDeviceName: PVOID,                       // 0xb8
    pub ArcHalDeviceName: PVOID,                        // 0xc0
    pub NtBootPathName: PVOID,                          // 0xc8
    pub NtHalPathName: PVOID,                           // 0xd0
    pub LoadOptions: PVOID,                             // 0xd8
    pub NlsData: PVOID,                                 // 0xe0
    pub ArcDiskInformation: PVOID,                      // 0xe8
    pub Extension: PVOID,                               // 0xf0
    pub u: [u8; 0x10],                                  // 0xf8
    pub FirmwareInformation: [u8; 0x40],                // 0x108
    pub OsBootstatPathName: PVOID,                      // 0x148
    pub ArcOSDataDeviceName: PVOID,                     // 0x150
    pub ArcWindowsSysPartName: PVOID,                   // 0x158
}

/// _LOADER_PARAMETER_EXTENSION - 0xe38 bytes ( 정밀 오프셋 보정 버전 )
#[repr(C)]
pub struct LOADER_PARAMETER_EXTENSION {
    pub Size: ULONG,                                    // 0x0
    pub Profile: [UCHAR; 0x14],                         // 0x4
    pub EmInfFileImage: PVOID,                          // 0x18
    pub EmInfFileSize: ULONG,                           // 0x20
    pub Padding1: [UCHAR; 0x54],                        // 0x24 -> 0x78 (AcpiTable 위치 확보)
    pub AcpiTable: PVOID,                               // 0x78
    pub AcpiTableSize: ULONG,                           // 0x80
    pub Bitfields: ULONG,                               // 0x84
    pub LoaderPerformanceData: [UCHAR; 0x60],           // 0x88 -> 0xE8
    pub Padding2: [UCHAR; 0xAA0],                       // 0xE8 -> 0xB88 (MajorRelease 위치 확보)
    pub MajorRelease: ULONG,                            // 0xb88
    pub MinorRelease: ULONG,                            // 0xb8c
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

        prcb.MxCsr = 0x1F80;
        prcb.CurrentThread = dummy_thread_v;
        prcb.NextThread = dummy_thread_v;
        prcb.IdleThread = dummy_thread_v;
        prcb.RspBase = stack_v;
        prcb.MinorVersion = 1;
        prcb.MajorVersion = 1;

        let kpcr_bytes = unsafe { std::slice::from_raw_parts(&kpcr as *const _ as *const u8, std::mem::size_of::<KPCR>()) };
        vm.write_memory(paddr as usize, kpcr_bytes)?;

        let prcb_bytes = unsafe { std::slice::from_raw_parts(&prcb as *const _ as *const u8, std::mem::size_of::<KPRCB>()) };
        vm.write_memory((paddr + 0x180) as usize, prcb_bytes)?;

        let valid_cr3 = 0x8102000u64; 
        vm.write_memory((paddr + 0x6000 + 0x28) as usize, &valid_cr3.to_le_bytes())?;
        vm.write_memory((paddr + 0x5000 + 0xA8) as usize, &dummy_process_v.to_le_bytes())?;

        Ok(())
    }
}

pub struct LoaderParameterBlock;
impl LoaderParameterBlock {
    pub fn setup(vm: &mut Vm, lpb_v: u64, lpb_p: u64, prcb_v: u64, stack_v: u64, registry_v: u64, registry_size: u32) -> Result<(), &'static str> {
        let mut lpb = unsafe { std::mem::zeroed::<LOADER_PARAMETER_BLOCK>() };
        lpb.OsMajorVersion = 10;
        lpb.Size = 0x160;
        lpb.KernelStack = stack_v;
        lpb.Prcb = prcb_v;
        lpb.Process = prcb_v + 0x5e80;
        lpb.Thread = prcb_v + 0x4e80;
        lpb.RegistryLength = registry_size;
        lpb.RegistryBase = registry_v;
        lpb.ConfigurationRoot = lpb_v + 0x2000;
        lpb.LoadOptions = lpb_v + 0xC000;
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

pub struct LoaderParameterExtension;
impl LoaderParameterExtension {
    pub const OFFSET_IN_LPB: u64 = 0x8000;
    pub fn setup(vm: &mut Vm, ext_p: u64) -> Result<(), &'static str> {
        let mut ext = unsafe { std::mem::zeroed::<LOADER_PARAMETER_EXTENSION>() };
        ext.Size = 0xE38;
        ext.MajorRelease = 10;
        ext.MinorRelease = 0;
        let bytes = unsafe { std::slice::from_raw_parts(&ext as *const _ as *const u8, 0xE38) };
        vm.write_memory(ext_p as usize, bytes)?;
        Ok(())
    }
    pub fn set_acpi(vm: &mut Vm, ext_p: u64, rsdp_v: u64) -> Result<(), &'static str> {
        // 구조체 필드를 통해 직접 주소 쓰기 (오프셋 0x78 보장)
        vm.write_memory((ext_p + 0x78) as usize, &rsdp_v.to_le_bytes())?;
        vm.write_memory((ext_p + 0x80) as usize, &36u32.to_le_bytes())?;
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
