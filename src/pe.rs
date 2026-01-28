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
    pub pdb_name: String, // 추가: PDB 이름을 통해 버전 유추 가능
}

/// 외부에서 호출할 수 있는 파싱 함수
pub fn parse(bytes: &[u8]) -> Result<PeFile, &'static str> {
    PeFile::from_bytes(bytes)
}

impl PeFile {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        let file = File::parse(bytes).map_err(|_| "Failed to parse file: not a valid object file")?;

        // PDB 이름 추출 시도
        let pdb_name = match file.pdb_info() {
            Ok(Some(info)) => String::from_utf8_lossy(info.path()).to_string(),
            _ => "Unknown".to_string(),
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
            pdb_name,
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
