use crate::{Error, Result};
use zeroize::Zeroizing;

#[cfg(windows)]
fn crypt(input: &[u8], decrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
    use windows_sys::Win32::{
        Foundation::LocalFree,
        Security::Cryptography::{
            CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
        },
    };
    if input.len() > u32::MAX as usize {
        return Err(Error::new("VaultError", "dpapi", "Secret too large"));
    }
    let blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };
    let entropy = b"NetPulse2/local-profile/v1";
    let salt = CRYPT_INTEGER_BLOB {
        cbData: entropy.len() as u32,
        pbData: entropy.as_ptr() as *mut u8,
    };
    // DPAPI current-user scope. UI disabled, no machine-wide or plaintext fallback.
    let ok = unsafe {
        if decrypt {
            CryptUnprotectData(
                &blob,
                std::ptr::null_mut(),
                &salt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &blob,
                std::ptr::null(),
                &salt,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if ok == 0 {
        return Err(Error::io("dpapi", std::io::Error::last_os_error()));
    }
    let result = Zeroizing::new(
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec(),
    );
    unsafe {
        std::ptr::write_bytes(output.pbData, 0, output.cbData as usize);
        LocalFree(output.pbData as *mut _);
    }
    Ok(result)
}
#[cfg(not(windows))]
fn crypt(_input: &[u8], _decrypt: bool) -> Result<Zeroizing<Vec<u8>>> {
    Err(Error::new(
        "PlatformUnsupported",
        "dpapi",
        "Windows DPAPI is required",
    ))
}
pub fn seal(input: &[u8]) -> Result<Vec<u8>> {
    Ok(crypt(input, false)?.to_vec())
}
pub fn open(input: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    crypt(input, true)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_corruption() {
        let secret = b"PRIVATE-CREDENTIAL";
        let sealed = seal(secret).unwrap();
        assert!(!sealed.windows(secret.len()).any(|v| v == secret));
        assert_eq!(&*open(&sealed).unwrap(), secret);
        let mut corrupt = sealed;
        let n = corrupt.len();
        corrupt[n - 1] ^= 0xff;
        assert!(open(&corrupt).is_err());
    }
}
