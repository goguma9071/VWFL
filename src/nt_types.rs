// src/nt_types.rs

#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use core::ffi::c_void;

// Windows 커널 기본 타입 정의
pub type UCHAR = u8;
pub type USHORT = u16;
pub type ULONG = u32;
pub type LONG = i32;
pub type ULONGLONG = u64;
pub type LONGLONG = i64;
pub type PVOID = u64; // 하이퍼바이저 환경에서는 게스트 주소를 u64로 관리하는 것이 안전합니다.

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LIST_ENTRY {
    pub Flink: u64,
    pub Blink: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct UNICODE_STRING {
    pub Length: USHORT,
    pub MaximumLength: USHORT,
    // x64 alignment padding (4 bytes) will be inserted here by repr(C)
    pub Buffer: u64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct GUID {
    pub Data1: u32,
    pub Data2: u16,
    pub Data3: u16,
    pub Data4: [u8; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union LARGE_INTEGER {
    pub QuadPart: LONGLONG,
    pub u: LARGE_INTEGER_u,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LARGE_INTEGER_u {
    pub LowPart: ULONG,
    pub HighPart: LONG,
}
