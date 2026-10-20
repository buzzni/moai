//! 윈도 `.res`(VS_VERSIONINFO 하나)를 바이트로 쓴다.
//!
//! `build.rs` 와 `tests/versioninfo.rs` 가 같은 파일을 `#[path]` 로 든다 — build.rs 는
//! 시험을 돌릴 수 없어서, 바이트 배치를 리눅스에서 손으로 센 값과 견주려면 이렇게 떼어 둬야 한다.
//! 의존성(`winresource`·`embed-resource`)을 안 쓰는 까닭은 build.rs 의 머리말에 있다.

/// 버전 정보에 들어가는 글. 키는 StringFileInfo 의 표준 이름이다.
pub struct Info<'a> {
    pub version: (u16, u16, u16),
    pub strings: &'a [(&'a str, &'a str)],
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(u16::to_le_bytes).collect()
}

// `is_multiple_of` 는 1.90 부터라 rust-version(1.88)에서는 비트로 쓴다.
fn pad4(v: &mut Vec<u8>) {
    while v.len() & 3 != 0 {
        v.push(0);
    }
}

/// VS_VERSIONINFO 계열의 마디 하나: wLength·wValueLength·wType·키·(패딩)·값·자식.
/// 정렬은 이 마디가 DWORD 경계에서 시작한다고 보고 센다 — 자식은 붙이기 전에 패딩으로
/// 경계를 맞춘다. wLength 에 뒤따르는 패딩은 안 든다(rc.exe 와 같다).
fn node(value_len: u16, w_type: u16, key: &str, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
    let mut v = vec![0, 0];
    v.extend(value_len.to_le_bytes());
    v.extend(w_type.to_le_bytes());
    v.extend(utf16z(key));
    pad4(&mut v);
    v.extend(value);
    for c in children {
        pad4(&mut v);
        v.extend(c);
    }
    let len = u16::try_from(v.len()).expect("버전 정보가 64KB 를 넘는다");
    v[0..2].copy_from_slice(&len.to_le_bytes());
    v
}

fn fixed_file_info(version: (u16, u16, u16)) -> Vec<u8> {
    let ms = (u32::from(version.0) << 16) | u32::from(version.1);
    let ls = u32::from(version.2) << 16;
    [
        0xFEEF_04BD, // dwSignature
        0x0001_0000, // dwStrucVersion
        ms,          // dwFileVersionMS
        ls,          // dwFileVersionLS
        ms,          // dwProductVersionMS
        ls,          // dwProductVersionLS
        0x3F,        // dwFileFlagsMask
        0,           // dwFileFlags
        0x0004_0004, // dwFileOS = VOS_NT_WINDOWS32
        1,           // dwFileType = VFT_APP
        0,           // dwFileSubtype
        0,           // dwFileDateMS
        0,           // dwFileDateLS
    ]
    .into_iter()
    .flat_map(u32::to_le_bytes)
    .collect()
}

/// RT_VERSION 리소스의 데이터(헤더 없는 VS_VERSIONINFO 구조체).
pub fn version_info(info: &Info) -> Vec<u8> {
    let strings: Vec<Vec<u8>> = info
        .strings
        .iter()
        .map(|(k, val)| {
            let value = utf16z(val);
            // 문자열의 wValueLength 는 바이트가 아니라 널을 포함한 WORD 수다.
            node((value.len() / 2) as u16, 1, k, &value, &[])
        })
        .collect();
    // 040904B0 = 영어(미국) + 유니코드 코드페이지(1200).
    let table = node(0, 1, "040904B0", &[], &strings);
    let string_file_info = node(0, 1, "StringFileInfo", &[], &[table]);
    let var = node(4, 0, "Translation", &0x04B0_0409u32.to_le_bytes(), &[]);
    let var_file_info = node(0, 1, "VarFileInfo", &[], &[var]);
    node(52, 0, "VS_VERSION_INFO", &fixed_file_info(info.version), &[string_file_info, var_file_info])
}

/// 리소스 파일 한 개를 감싼다: 빈 32바이트 머리(.res 의 서명) + RT_VERSION/1 리소스.
pub fn res_file(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    // 서명 리소스: DataSize 0, HeaderSize 32, TYPE·NAME 모두 서수 0.
    out.extend(0u32.to_le_bytes());
    out.extend(32u32.to_le_bytes());
    out.extend([0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
    out.extend([0u8; 16]);
    // 실제 리소스. TYPE = 서수 16(RT_VERSION), NAME = 서수 1.
    out.extend((data.len() as u32).to_le_bytes());
    out.extend(32u32.to_le_bytes());
    out.extend([0xFF, 0xFF, 0x10, 0x00, 0xFF, 0xFF, 0x01, 0x00]);
    out.extend(0u32.to_le_bytes()); // DataVersion
    out.extend(0x0030u16.to_le_bytes()); // MemoryFlags = MOVEABLE | PURE
    out.extend(0x0409u16.to_le_bytes()); // LanguageId
    out.extend(0u32.to_le_bytes()); // Version
    out.extend(0u32.to_le_bytes()); // Characteristics
    out.extend(data);
    pad4(&mut out);
    out
}
