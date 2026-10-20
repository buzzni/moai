// build.rs 가 윈도 바이너리에 넣는 `.res`(moai-uwzg.kpx)의 바이트 배치를 리눅스에서 시험한다.
// 윈도 링커는 여기 없으니, 아래 숫자는 VS_VERSIONINFO 규칙으로 손으로 센 값이다.
#[path = "../build/versioninfo.rs"]
mod versioninfo;

use versioninfo::{Info, res_file, version_info};

fn w(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn d(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn sample() -> Vec<u8> {
    version_info(&Info { version: (1, 2, 3), strings: &[("A", "bc")] })
}

#[test]
fn version_info_layout_matches_hand_count() {
    let b = sample();
    // 뿌리: 6 + 키 32 → 40, 값 52 → 92, StringFileInfo(78) → 170, 패딩 → 172, VarFileInfo(68) → 240.
    assert_eq!(b.len(), 240);
    assert_eq!((w(&b, 0), w(&b, 2), w(&b, 4)), (240, 52, 0));
    assert_eq!(&b[6..38], utf16("VS_VERSION_INFO\0").as_slice());
    // VS_FIXEDFILEINFO 는 DWORD 경계(40)에서 선다.
    assert_eq!(d(&b, 40), 0xFEEF_04BD);
    assert_eq!(d(&b, 44), 0x0001_0000);
    assert_eq!(d(&b, 48), 0x0001_0002); // FileVersionMS = 1.2
    assert_eq!(d(&b, 52), 0x0003_0000); // FileVersionLS = 3.0
    assert_eq!(d(&b, 56), 0x0001_0002); // ProductVersionMS
    assert_eq!(d(&b, 60), 0x0003_0000);
    assert_eq!(d(&b, 64), 0x3F);
    assert_eq!(d(&b, 72), 0x0004_0004);
    assert_eq!(d(&b, 76), 1);
    // StringFileInfo(92): 길이 78, 값 0, 텍스트.
    assert_eq!((w(&b, 92), w(&b, 94), w(&b, 96)), (78, 0, 1));
    assert_eq!(&b[98..128], utf16("StringFileInfo\0").as_slice());
    // StringTable(128): 길이 42.
    assert_eq!((w(&b, 128), w(&b, 130), w(&b, 132)), (42, 0, 1));
    assert_eq!(&b[134..152], utf16("040904B0\0").as_slice());
    // String(152): 키 "A" 뒤에 패딩 2바이트, 값 "bc\0" 는 WORD 3개.
    assert_eq!((w(&b, 152), w(&b, 154), w(&b, 156)), (18, 3, 1));
    assert_eq!(&b[158..162], utf16("A\0").as_slice());
    assert_eq!(&b[164..170], utf16("bc\0").as_slice());
    // VarFileInfo(172) 와 Translation(204): 값 4바이트, 04B0(유니코드)/0409(영어).
    assert_eq!((w(&b, 172), w(&b, 174), w(&b, 176)), (68, 0, 1));
    assert_eq!((w(&b, 204), w(&b, 206), w(&b, 208)), (36, 4, 0));
    assert_eq!(&b[210..234], utf16("Translation\0").as_slice());
    assert_eq!(d(&b, 236), 0x04B0_0409);
}

#[test]
fn res_file_wraps_with_signature_and_rt_version_header() {
    let data = sample();
    let r = res_file(&data);
    assert_eq!(r.len(), 32 + 32 + 240);
    // 빈 서명 리소스.
    assert_eq!(d(&r, 0), 0);
    assert_eq!(d(&r, 4), 32);
    assert_eq!(&r[8..16], &[0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
    assert!(r[16..32].iter().all(|&x| x == 0));
    // RT_VERSION(16) / 이름 1 / MOVEABLE|PURE / en-US.
    assert_eq!(d(&r, 32), 240);
    assert_eq!(d(&r, 36), 32);
    assert_eq!(&r[40..48], &[0xFF, 0xFF, 0x10, 0, 0xFF, 0xFF, 0x01, 0]);
    assert_eq!(d(&r, 48), 0);
    assert_eq!((w(&r, 52), w(&r, 54)), (0x0030, 0x0409));
    assert_eq!(&r[64..], data.as_slice());
    assert_eq!(r.len() % 4, 0);
}

#[test]
fn odd_sized_values_still_keep_siblings_aligned() {
    // 값이 홀수 WORD 일 때(널 포함 3개) 다음 마디 앞에 패딩이 들어가야 한다.
    let b = version_info(&Info { version: (0, 0, 0), strings: &[("A", "bc"), ("B", "d")] });
    // 첫 String 은 152 에서 18바이트 → 둘째는 172 에서 시작해야 한다.
    assert_eq!(w(&b, 172 + 4), 1);
    assert_eq!(&b[172 + 6..172 + 10], utf16("B\0").as_slice());
    assert_eq!(b.len() % 4, 0);
}
