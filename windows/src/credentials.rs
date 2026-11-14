use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use windows_sys::Win32::{
    Foundation::LocalFree,
    Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    },
};

#[derive(Serialize, Deserialize)]
pub struct Identity {
    pub cert: Vec<u8>,
    pub key: Vec<u8>,
    pub fingerprint: String,
    pub session: Option<String>,
}
fn crypt(input: &[u8], decrypt: bool) -> Result<Vec<u8>> {
    let source = CRYPT_INTEGER_BLOB {
        cbData: input.len().try_into()?,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut dest: CRYPT_INTEGER_BLOB = unsafe { std::mem::zeroed() };
    let ok = unsafe {
        if decrypt {
            CryptUnprotectData(
                &source,
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut dest,
            )
        } else {
            CryptProtectData(
                &source,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut dest,
            )
        }
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let result = unsafe { std::slice::from_raw_parts(dest.pbData, dest.cbData as usize).to_vec() };
    unsafe {
        LocalFree(dest.pbData.cast());
    }
    Ok(result)
}
pub fn load(path: &Path) -> Result<Option<Identity>> {
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(serde_json::from_slice(&crypt(
        &std::fs::read(path)?,
        true,
    )?)?))
}
pub fn save(path: &Path, identity: &Identity) -> Result<()> {
    let data = crypt(&serde_json::to_vec(identity)?, false)?;
    let tmp = path.with_extension("tmp");
    let mut f = std::fs::File::create(&tmp)?;
    use std::io::Write;
    f.write_all(&data)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpapi_round_trip_is_not_plaintext() {
        let secret = b"localdrop-test-secret";
        let encrypted = crypt(secret, false).unwrap();
        assert_ne!(&encrypted, secret);
        assert_eq!(crypt(&encrypted, true).unwrap(), secret);
    }
}
