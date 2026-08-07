use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Variant::VT_I8;

fn main() {
    let mut prop = PROPVARIANT::default();
    unsafe {
        prop.Anonymous.Anonymous.vt = VT_I8;
        prop.Anonymous.Anonymous.Anonymous.hVal = 10000;
    }
}
