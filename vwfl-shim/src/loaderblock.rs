#![allow(non_snake_case)]
#![allow(non_camel_case_types)]
#![no_std]
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

/// _KLDR_DATA_TABLE_ENTRY - Vergilious Windows 10 명세 반영 (0x120 bytes)
#[repr(C)]
pub struct LDR_DATA_TABLE_ENTRY {
    pub InLoadOrderLinks: LIST_ENTRY,                    // 0x0
    pub InMemoryOrderLinks: LIST_ENTRY,                  // 0x10
    pub InInitializationOrderLinks: LIST_ENTRY,          // 0x20
    pub DllBase: PVOID,                                  // 0x30
    pub EntryPoint: PVOID,                               // 0x38
    pub SizeOfImage: ULONG,                              // 0x40
    pub Padding_After_Size: ULONG,                       // 0x44
    pub FullDllName: UNICODE_STRING,                     // 0x48 (0x44 패딩 포함 자동 정렬)
    pub BaseDllName: UNICODE_STRING,                     // 0x58
    pub Flags: ULONG,                                    // 0x68
    pub ObsoleteLoadCount: USHORT,                       // 0x6c
    pub TlsIndex: USHORT,                                // 0x6e
    pub HashLinks: LIST_ENTRY,                           // 0x70
    pub TimeDateStamp: ULONG,                            // 0x80
    pub EntryPointActivationContext: PVOID,              // 0x88 (0x84 패딩 포함)
    pub Lock: PVOID,                                     // 0x90
    pub DdagNode: PVOID,                                 // 0x98
    pub NodeModuleLink: LIST_ENTRY,                      // 0xa0
    pub LoadContext: PVOID,                              // 0xb0
    pub ParentDllBase: PVOID,                            // 0xb8
    pub SwitchBackContext: PVOID,                        // 0xc0
    pub BaseAddressIndexNode: [UCHAR; 24],               // 0xc8
    pub MappingInfoIndexNode: [UCHAR; 24],               // 0xe0
    pub OriginalBase: ULONGLONG,                         // 0xf8
    pub LoadTime: ULONGLONG,                             // 0x100
    pub BaseNameHashValue: ULONG,                        // 0x108
    pub LoadReason: ULONG,                               // 0x10c
    pub ImplicitPathOptions: ULONG,                      // 0x110
    pub ReferenceCount: ULONG,                           // 0x114
    pub DependentLoadFlags: ULONG,                       // 0x118
    pub SigningLevel: UCHAR,                             // 0x11c
    pub Padding_Final: [UCHAR; 3],                       // 0x11d -> 0x120
}

/// _KSPECIAL_REGISTERS - 베르길리우스 명세 반영 (0xf0 bytes)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct KSPECIAL_REGISTERS {
    pub Cr0: ULONGLONG,                                 // 0x0
    pub Cr2: ULONGLONG,                                 // 0x8
    pub Cr3: ULONGLONG,                                 // 0x10
    pub Cr4: ULONGLONG,                                 // 0x18
    pub KernelDr0: ULONGLONG,                           // 0x20
    pub KernelDr1: ULONGLONG,                           // 0x28
    pub KernelDr2: ULONGLONG,                           // 0x30
    pub KernelDr3: ULONGLONG,                           // 0x38
    pub KernelDr6: ULONGLONG,                           // 0x40
    pub KernelDr7: ULONGLONG,                           // 0x48
    pub Gdtr: [UCHAR; 16],                              // 0x50 (KDESCRIPTOR)
    pub Idtr: [UCHAR; 16],                              // 0x60 (KDESCRIPTOR)
    pub Tr: USHORT,                                     // 0x70
    pub Ldtr: USHORT,                                   // 0x72
    pub MxCsr: ULONG,                                   // 0x74
    pub DebugControl: ULONGLONG,                        // 0x78
    pub LastBranchToRip: ULONGLONG,                     // 0x80
    pub LastBranchFromRip: ULONGLONG,                   // 0x88
    pub LastExceptionToRip: ULONGLONG,                  // 0x90
    pub LastExceptionFromRip: ULONGLONG,                // 0x98
    pub Cr8: ULONGLONG,                                 // 0xa0
    pub MsrGsBase: ULONGLONG,                           // 0xa8
    pub MsrGsSwap: ULONGLONG,                           // 0xb0
    pub MsrStar: ULONGLONG,                             // 0xb8
    pub MsrLStar: ULONGLONG,                            // 0xc0
    pub MsrCStar: ULONGLONG,                            // 0xc8
    pub MsrSyscallMask: ULONGLONG,                      // 0xd0
    pub Xcr0: ULONGLONG,                                // 0xd8
    pub MsrFsBase: ULONGLONG,                           // 0xe0
    pub SpecialPadding0: ULONGLONG,                     // 0xe8 -> 0xf0
}

/// _KPROCESSOR_STATE (0x5c0 bytes)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct KPROCESSOR_STATE {
    pub SpecialRegisters: KSPECIAL_REGISTERS,           // 0x0 (0xf0 bytes)
    pub ContextFrame: [UCHAR; 0x4d0],                   // 0xf0 (CONTEXT)
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
    pub ProcessorState: KPROCESSOR_STATE,               // 0x100 (0x5c0 bytes)
    pub ExtendedSupervisorState: PVOID,                 // 0x6c0
    pub ProcessorSignature: ULONG,                      // 0x6c8
    pub ProcessorFlags: ULONG,                          // 0x6cc
    pub PrcbPad12a: ULONGLONG,                          // 0x6d0
    pub PrcbPad12: [ULONGLONG; 3],                      // 0x6d8 -> 0x700
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
    pub SecondLevelCacheAssociativity: UCHAR,           // 0x51
    pub ObsoleteNumber: UCHAR,                          // 0x52
    pub Fill0: UCHAR,                                   // 0x53
    pub Unused0: [ULONG; 3],                            // 0x54
    pub MajorVersion: USHORT,                           // 0x60
    pub MinorVersion: USHORT,                           // 0x62
    pub StallScaleFactor: ULONG,                        // 0x64
    pub Unused1: [PVOID; 3],                            // 0x68
    pub KernelReserved: [ULONG; 15],                    // 0x80
    pub SecondLevelCacheSize: ULONG,                    // 0xbc
    pub HalReserved: [ULONG; 16],                       // 0xc0
    pub Unused2: ULONG,                                 // 0x100
    pub Padding_Align: ULONG,                           // 0x104
    pub KdVersionBlock: PVOID,                          // 0x108
    pub Unused3: PVOID,                                 // 0x110
    pub PcrAlign1: [ULONG; 24],                         // 0x118
}

struct Kpcr;
impl Kpcr {
    pub unsafe fn setup(gdt_v: PVOID) {

        let kpcr = 0x40000 as *mut KPCR;

        (*kpcr).MajorVersion = 10;
        (*kpcr).MinorVersion = 19041;
        (*kpcr).GdtBase = gdt_v; //and more!!! todo: i need to research about kernel structure structure's data & memory offset

    }
}

#[repr(C)]
pub struct LOADER_PARAMETER_BLOCK {
    pub OsMajorVersion: ULONG,                           // 0x0 // to 10
    pub OsMinorVersion: ULONG,                           // 0x4 // 19041?
    pub Size: ULONG,                                     // 0x8 // core::mem::size_of::<LOADER_PARAMETER_BLOCK>() as u32;
    pub OsLoaderSecurityVersion: ULONG,                  // 0xc
    pub LoadOrderListHead: LIST_ENTRY,                   // 0x10
    pub MemoryDescriptorListHead: LIST_ENTRY,            // 0x20 // i think I must gonna fill this field. right?
    pub BootDriverListHead: LIST_ENTRY,                  // 0x30 // either too
    pub EarlyLaunchListHead: LIST_ENTRY,                 // 0x40 // either too
    pub CoreDriverListHead: LIST_ENTRY,                  // 0x50 // either too
    pub CoreExtensionsDriverListHead: LIST_ENTRY,        // 0x60
    pub TpmCoreDriverListHead: LIST_ENTRY,               // 0x70
    pub KernelStack: ULONGLONG,                          // 0x80 // kernel_stack_v
    pub Prcb: ULONGLONG,                                 // 0x88
    pub Process: ULONGLONG,                              // 0x90
    pub Thread: ULONGLONG,                               // 0x98
    pub KernelStackSize: ULONG,                          // 0xa0
    pub RegistryLength: ULONG,                           // 0xa4
    pub RegistryBase: PVOID,                             // 0xa8
    pub ConfigurationRoot: PVOID,                        // 0xb0
    pub ArcBootDeviceName: PVOID,                        // 0xb8
    pub ArcHalDeviceName: PVOID,                         // 0xc0
    pub NtBootPathName: PVOID,                           // 0xc8
    pub NtHalPathName: PVOID,                            // 0xd0
    pub LoadOptions: PVOID,                              // 0xd8
    pub NlsData: PVOID,                                  // 0xe0
    pub ArcDiskInformation: PVOID,                       // 0xe8
    pub Extension: PVOID,                                // 0xf0
    pub u: [UCHAR; 16],                                  // 0xf8
    pub FirmwareInformation: [UCHAR; 64],                // 0x108
    pub OsBootstatPathName: PVOID,                       // 0x148
    pub ArcOSDataDeviceName: PVOID,                      // 0x150
    pub ArcWindowsSysPartName: PVOID,                    // 0x158
}

// 0x400000 대역에 Loaderblock을 배치해야 한다.

pub struct LoaderParameterBlock;

impl LoaderParameterBlock {

    pub unsafe fn setup(stack_v: u64, stack_size: ULONG, prcb_v: ULONGLONG) {

        let lpb = 0x40000 as *mut LOADER_PARAMETER_BLOCK;

            (*lpb).OsMajorVersion = 10;
            (*lpb).OsMinorVersion = 19041;
            (*lpb).Size = size_of::<LOADER_PARAMETER_BLOCK>() as u32;
            (*lpb).KernelStack = stack_v;
            (*lpb).KernelStackSize = stack_size;
            (*lpb).Prcb = prcb_v;
            (*lpb).LoadOptions = stack_v + 0xC000u64;
            (*lpb).Extension = stack_v + 0xe38;


    }
}


/// _LOADER_PARAMETER_EXTENSION - 0xe38 bytes
#[repr(C)]
pub struct LOADER_PARAMETER_EXTENSION {
    pub Size: ULONG,                                    // 0x0
    pub Profile: [UCHAR; 0x14],                         // 0x4
    pub EmInfFileImage: PVOID,                          // 0x18
    pub EmInfFileSize: ULONG,                           // 0x20
    pub TriageDumpBlock: PVOID,                         // 0x28
    pub HeadlessLoaderBlock: PVOID,                     // 0x30
    pub SMBiosEPSHeader: PVOID,                         // 0x38
    pub DrvDBImage: PVOID,                              // 0x40
    pub DrvDBSize: ULONG,                               // 0x48
    pub DrvDBPatchImage: PVOID,                         // 0x50
    pub DrvDBPatchSize: ULONG,                          // 0x58
    pub NetworkLoaderBlock: PVOID,                      // 0x60
    pub FirmwareDescriptorListHead: LIST_ENTRY,         // 0x68
    pub AcpiTable: PVOID,                               // 0x78
    pub AcpiTableSize: ULONG,                           // 0x80
    pub Bitfields: ULONG,                               // 0x84
    pub LoaderPerformanceData: [UCHAR; 0x60],           // 0x88
    pub BootApplicationPersistentData: LIST_ENTRY,      // 0xe8
    pub Padding1: [UCHAR; 0x8D0],                       // 0xF8 -> 0x9C8
    pub ProcessorCounterFrequency: ULONGLONG,           // 0x9c0
    pub HypervisorExtension: [UCHAR; 0x40],             // 0x9c8
    pub HardwareConfigurationId: [UCHAR; 16],           // 0xa08
    pub HalExtensionModuleList: LIST_ENTRY,             // 0xa18
    pub SystemTime: ULONGLONG,                          // 0xa28
    pub TimeStampAtSystemTimeRead: ULONGLONG,           // 0xa30
    pub BootFlags: ULONGLONG,                           // 0xa38
    pub InternalBootFlags: ULONGLONG,                   // 0xa40
    pub WfsFPData: PVOID,                               // 0xa48
    pub WfsFPDataSize: ULONG,                           // 0xa50
    pub BugcheckParameters: [UCHAR; 0x28],              // 0xa58
    pub ApiSetSchema: PVOID,                            // 0xa80
    pub ApiSetSchemaSize: ULONG,                        // 0xa88
    pub ApiSetSchemaExtensions: LIST_ENTRY,             // 0xa90
    pub PaddingFinal: [UCHAR; 0x390],                   // 0xaa0 -> 0xe30
    pub MajorRelease: ULONG,                            // 0xb88
    pub MinorRelease: ULONG,                            // 0xb8c
}