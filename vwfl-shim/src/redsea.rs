#![no_std]

#[repr(C, packed)]
struct CDATE {
    time: u32,
    date: i32
}


pub fn read_readsea() {
    let file_name: [u32; 8] = [0x0000000, 0x03FF73FB, 0xEFFFFFFF, 0x2FFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF];
    //todo: implement the reading redsea format
}

#[repr(C, packed)]
pub struct CDIREntry {
    attr: u16,
    name: [u8; 38],
    clus: i64, //same as blk
    size: i64,
    datetime: CDATE
}
#[repr(C, packed)]
pub struct CRedSeaBoot {
    jump_and_nop: [u8; 3],
    signature: u8,
    reserved: [u8; 4],
    drive_offset: i64,
    sects: i64,
    root_clus: i64,
    bitmap_sects: i64,
    unique_id: i64,
    code: [u8; 462],
    signature2: u16


}