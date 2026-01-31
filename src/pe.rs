// src/pe.rs

use object::{File, Object, ObjectSection, Architecture, LittleEndian};
use std::fmt;

/// PE 파일의 한 섹션에 대한 정보를 담는 구조체
#[derive(Debug)]
pub struct Section {
    pub name: String,
    pub virtual_address: u64,
    pub virtual_size: u64,
    pub raw_data: Vec<u8>,
}

/// 파싱된 PE 파일 전체를 나타내는 구조체
#[derive(Debug)]
pub struct PeFile {
    pub entry_point: u64,
    pub architecture: Architecture,
    pub image_base: u64,
    pub sections: Vec<Section>,
    pub header_data: Vec<u8>, // 추가: PE 헤더 데이터
    pub pdb_name: String,
    pub pdb_guid_age: String, 
}

/// 외부에서 호출할 수 있는 파싱 함수
pub fn parse(bytes: &[u8]) -> Result<PeFile, &'static str> {
    let mut pe = PeFile::from_bytes(bytes)?;
    
    // 헤더 데이터 추출 (첫 섹션의 RawData 시작점 이전까지)
    let header_size = if !pe.sections.is_empty() {
        // 섹션들의 raw_data 시작점 중 최소값을 찾음 (보통 0x400 ~ 0x1000)
        // 여기서는 간단히 4096바이트 또는 첫 섹션 이전까지로 제한
        4096.min(bytes.len())
    } else {
        bytes.len().min(4096)
    };
    pe.header_data = bytes[..header_size].to_vec();
    
    Ok(pe)
}

impl PeFile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        let file = File::parse(bytes).map_err(|_| "Failed to parse file: not a valid object file")?;

        // PDB 정보(GUID, Age) 추출
        let (pdb_name, pdb_guid_age) = match file.pdb_info() {
            Ok(Some(info)) => {
                let name = String::from_utf8_lossy(info.path()).to_string();
                let guid = info.guid();
                let guid_str = format!(
                    "{:08X}{:04X}{:04X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:X}",
                    guid[0], guid[1], guid[2], guid[3], guid[4], guid[5], guid[6], guid[7], guid[8], guid[9], guid[10],
                    info.age()
                );
                (name, guid_str)
            }
            _ => ("Unknown".to_string(), "Unknown".to_string()),
        };

        let (entry_point, architecture, image_base, sections) = match file {
            File::Pe32(pe) => {
                let sections = Self::extract_sections(&pe)?;
                let image_base = pe.nt_headers().optional_header.image_base.get(LittleEndian) as u64;
                (pe.entry(), pe.architecture(), image_base, sections)
            }
            File::Pe64(pe) => {
                let sections = Self::extract_sections(&pe)?;
                let image_base = pe.nt_headers().optional_header.image_base.get(LittleEndian);
                (pe.entry(), pe.architecture(), image_base, sections)
            }
            _ => return Err("The file is not a valid PE file."),
        };

        Ok(PeFile {
            entry_point,
            architecture,
            image_base,
            sections,
            header_data: Vec::new(),
            pdb_name,
            pdb_guid_age,
        })
    }

    /// `object`의 섹션 정보로부터 필요한 정보만 뽑아서 `Vec<Section>` 생성
    fn extract_sections<'data: 'file, 'file, O: Object<'data,>>(
        object_file: &'file O,
    ) -> Result<Vec<Section>, &'static str> {
        let mut sections = Vec::new();
        for section in object_file.sections() {
            if let Ok(name) = section.name() {
                if section.size() > 0 {
                    let raw_data = match section.data() {
                        Ok(data) => data.to_vec(),
                        Err(_) => return Err("Failed to read raw data from a section"),
                    };

                    sections.push(Section {
                        name: name.to_string(),
                        virtual_address: section.address(),
                        virtual_size: section.size(),
                        raw_data,
                    });
                }
            }
        }
        Ok(sections)
    }
}

// 디버깅 출력을 보기 좋게 하기 위한 Display 트레잇 구현
impl fmt::Display for PeFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "PE File Info:")?;
        writeln!(f, "  Architecture: {:?}", self.architecture)?;
        writeln!(f, "  Image Base:   0x{:x}", self.image_base)?;
        writeln!(f, "  Entry Point:  0x{:x}", self.entry_point)?;
        writeln!(f, "  Sections:")?;
        for section in &self.sections {
            writeln!(
                f,
                "    - Name: {:<8} Addr: 0x{:08x}, Size: {}",
                section.name, section.virtual_address, section.virtual_size
            )?;
        }
        Ok(())
    }
}
